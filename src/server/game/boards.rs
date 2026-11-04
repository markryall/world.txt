//! Noticeboards: places people build where anyone passing can leave a note,
//! like a group chat that belongs to a spot. Posting follows the board, so
//! you hear about new notes wherever you are.
//!
//! The Island Journal is a special board at the spawn point. The island posts
//! each week's first find of every species to it, and it starts a fresh page
//! every Monday, with old pages kept in its archive.

use super::Game;
use crate::protocol::{BoardMode, Col, Line, Note, ToClient, Week};
use crate::server::nature::Species;
use crate::server::world::Trigger;
use chrono::{Datelike, Local, NaiveDate, TimeZone};

const PAGE: i64 = 10;
const MAX_NOTE: usize = 280;
const JOURNAL_KIND: &str = "journal";
const BOARD_KIND: &str = "board";

/// Where a player is reading, so `read more` knows what comes next.
pub struct Reading {
    board: i64,
    week: Option<String>,
    oldest: i64,
}

pub fn is_board_name(name: &str) -> bool {
    let n = name.to_lowercase();
    ["noticeboard", "notice board", "message board", "bulletin board", "notice-board"]
        .iter()
        .any(|w| n.contains(w))
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn local(ts: i64) -> chrono::DateTime<Local> {
    Local.timestamp_opt(ts, 0).single().unwrap_or_else(Local::now)
}

/// The Monday a moment's week starts on, in local time: "2026-11-02".
pub fn week_key(ts: i64) -> String {
    let d = local(ts).date_naive();
    let monday = d - chrono::Duration::days(d.weekday().num_days_from_monday() as i64);
    monday.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
fn week_start(key: &str) -> Option<i64> {
    let d = NaiveDate::parse_from_str(key, "%Y-%m-%d").ok()?;
    let dt = d.and_hms_opt(0, 0, 0)?;
    Local.from_local_datetime(&dt).earliest().map(|t| t.timestamp())
}

fn week_label(key: &str) -> String {
    match NaiveDate::parse_from_str(key, "%Y-%m-%d") {
        Ok(d) => format!("Week of {}", d.format("%a %-d %b")),
        Err(_) => key.to_string(),
    }
}

fn when(ts: i64) -> String {
    let ago = now() - ts;
    match ago {
        ..60 => "just now".into(),
        60..3600 => format!("{}m ago", ago / 60),
        3600..86_400 => format!("{}h ago", ago / 3600),
        86_400..172_800 => "yesterday".into(),
        172_800..518_400 => local(ts).format("%a").to_string(),
        _ => local(ts).format("%-d %b").to_string(),
    }
}

impl Game {
    // ---- the Island Journal --------------------------------------------

    /// Make sure the Island Journal stands at the spawn point. When it's
    /// first put up, this week's discoveries so far are written into it.
    pub(super) fn ensure_journal(&mut self) -> anyhow::Result<()> {
        let existing = self.world.things.iter().find(|t| t.kind.as_deref() == Some(JOURNAL_KIND)).map(|t| t.id);
        if let Some(id) = existing {
            self.journal_board = Some(id);
            return Ok(());
        }
        let (x, y) = self.world.spawn_point();
        let id = self.world.add_thing(x, y, "the Island Journal".into(), None, Trigger::Always, "the island", Some(JOURNAL_KIND))?;
        self.journal_board = Some(id);
        let added = backfill_journal(&self.world.db, id)?;
        println!("📖 put up the Island Journal ({added} past discoveries written in)");
        Ok(())
    }

    /// Called whenever someone gathers or catches something: the first of
    /// each species each week goes in the Island Journal.
    pub(super) fn note_discovery(&mut self, player: &str, s: &'static Species) -> anyhow::Result<()> {
        let Some(board) = self.journal_board else { return Ok(()) };
        let week = week_key(now());
        if self.world.db.species_noted(board, s.id, &week)? {
            return Ok(());
        }
        let text = discovery_text(s);
        let at = now();
        let note_id = self.world.db.add_note(board, player, &text, at, &week, Some(s.id))?;
        let line = Line::new()
            .c(Col::Yellow, "📖 ")
            .b(Col::Yellow, self.world.nick(player))
            .c(Col::Yellow, format!(" added {} to the Island Journal!", crate::server::nature::a(s.name)));
        let note = Note { id: note_id, author: self.world.nick(player), text, when: when(at) };
        for p in self.players.values() {
            let _ = p.tx.send(ToClient::Lines { lines: vec![line.clone()] });
            let _ = p.tx.send(self.board_update(board, note.clone()));
        }
        Ok(())
    }

    // ---- finding boards --------------------------------------------------

    pub(super) fn board_here(&self, x: i64, y: i64) -> Option<(i64, bool)> {
        self.world
            .things_at(x, y)
            .find(|t| t.is_board())
            .map(|t| (t.id, t.kind.as_deref() == Some(JOURNAL_KIND)))
    }

    fn board_title(&self, board: i64) -> String {
        match self.world.things.iter().find(|t| t.id == board) {
            Some(t) if t.kind.as_deref() == Some(JOURNAL_KIND) => "📖 The Island Journal".into(),
            Some(t) => format!("📌 Noticeboard at {}", self.world.place_title(t.x, t.y)),
            None => "📌 A noticeboard".into(),
        }
    }

    fn board_place(&self, board: i64) -> Option<(i64, i64)> {
        self.world.things.iter().find(|t| t.id == board).map(|t| (t.x, t.y))
    }

    fn board_update(&self, board: i64, note: Note) -> ToClient {
        ToClient::Board {
            board,
            title: self.board_title(board),
            mode: BoardMode::New,
            notes: vec![note],
            more: false,
            page: None,
            weeks: vec![],
        }
    }

    /// For `look`: how many notes are waiting on boards here.
    pub(super) fn board_lines_here(&self, id: u64) -> Vec<Line> {
        let Some(p) = self.players.get(&id) else { return vec![] };
        let Some((board, journal)) = self.board_here(p.x, p.y) else { return vec![] };
        let seen = self.world.db.membership(&p.user, board).map_or(0, |m| m.1);
        let new = self.world.db.notes_after(board, seen).unwrap_or(0);
        let what = if journal { "📖 The Island Journal" } else { "📌 The noticeboard" };
        let news = match new {
            0 => "has nothing new for you".to_string(),
            1 => "has 1 new note".to_string(),
            n => format!("has {n} new notes"),
        };
        vec![Line::new().c(Col::Yellow, format!("{what} {news}.")).c(Col::Dim, " (type read)")]
    }

    // ---- commands ----------------------------------------------------------

    pub(super) fn cmd_read(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, _name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let rest = rest.trim().to_lowercase();
        let Some((board, journal)) = self.board_here(x, y) else {
            self.say1(id, Line::plain("There's no noticeboard here. Type boards to see the ones you follow."));
            return Ok(());
        };
        let current = journal.then(|| week_key(now()));

        let (week, before, mode) = if rest == "more" || rest == "older" {
            match self.players.get(&id).and_then(|p| p.reading.as_ref()).filter(|r| r.board == board) {
                Some(r) => (r.week.clone(), Some(r.oldest), BoardMode::Older),
                None => (current, None, BoardMode::Open),
            }
        } else if let Some(key) = rest.strip_prefix("week ") {
            if !journal {
                self.say1(id, Line::plain("Only the Island Journal has weekly pages."));
                return Ok(());
            }
            (Some(key.trim().to_string()), None, BoardMode::Open)
        } else {
            (current, None, BoardMode::Open)
        };

        let (rows, more) = self.world.db.notes(board, week.as_deref(), before, PAGE)?;
        if mode == BoardMode::Older && rows.is_empty() {
            self.say1(id, Line::col(Col::Dim, "That's the very first note. Nothing older."));
            return Ok(());
        }
        if mode == BoardMode::Open {
            self.world.db.set_seen(&user, board, self.world.db.last_note(board)?)?;
        }
        let oldest = rows.first().map(|n| n.id).unwrap_or(i64::MAX);
        if let Some(p) = self.players.get_mut(&id) {
            p.reading = Some(Reading { board, week: week.clone(), oldest });
        }

        let this_week = week_key(now());
        let page = week.as_ref().map(|w| {
            if *w == this_week {
                format!("This week (since {})", week_label(w).trim_start_matches("Week of "))
            } else {
                week_label(w)
            }
        });
        let weeks = if journal {
            self.world
                .db
                .weeks(board)?
                .into_iter()
                .filter(|(w, _)| *w != this_week)
                .map(|(w, n)| Week { label: format!("{} ({n})", week_label(&w)), key: w })
                .collect()
        } else {
            vec![]
        };
        let muted = self.players.get(&id).map(|p| p.muted.clone()).unwrap_or_default();
        let notes = rows
            .into_iter()
            .filter(|n| !muted.contains(&n.author.to_lowercase()))
            .map(|n| Note { id: n.id, author: self.world.nick(&n.author), text: n.text, when: when(n.at) })
            .collect();
        let msg = ToClient::Board { board, title: self.board_title(board), mode, notes, more, page, weeks };
        if let Some(p) = self.players.get(&id) {
            let _ = p.tx.send(msg);
        }
        Ok(())
    }

    pub(super) fn cmd_post(&mut self, id: u64, text: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let Some((board, journal)) = self.board_here(x, y) else {
            self.say1(id, Line::plain("You need to be at a noticeboard to post. Make one with 'make a noticeboard'!"));
            return Ok(());
        };
        let text: String = text.trim().chars().take(MAX_NOTE).collect();
        if text.is_empty() {
            self.say1(id, Line::plain("Post what? e.g. 'post party here at sunset! 🪩'"));
            return Ok(());
        }
        let at = now();
        let note_id = self.world.db.add_note(board, &name, &text, at, &week_key(at), None)?;
        let was_following = self.world.db.membership(&user, board).is_some_and(|m| m.0);
        self.world.db.set_following(&user, board, true)?;
        self.world.db.set_seen(&user, board, note_id)?;

        let what = if journal { "the Island Journal" } else { "the noticeboard" };
        let place = self.world.place_title(x, y);
        let note = Note { id: note_id, author: name.clone(), text: text.clone(), when: when(at) };
        let update = self.board_update(board, note);
        let followers = self.world.db.followers(board)?;
        for (&pid, p) in &self.players {
            if pid != id && !p.hears(&user) {
                continue;
            }
            let here = (p.x, p.y) == (x, y);
            let line = if pid == id {
                Line::new().c(Col::Yellow, format!("📌 You post on {what}: ")).c(Col::White, text.clone())
            } else if here {
                Line::new().c(Col::Yellow, "📌 ").b(Col::Yellow, &name).c(Col::Yellow, format!(" posts on {what}: ")).c(Col::White, text.clone())
            } else if followers.contains(&p.user) {
                Line::new().c(Col::Yellow, "📌 ").b(Col::Yellow, &name).c(Col::Yellow, format!(" posted at {place}: ")).c(Col::White, text.clone())
            } else {
                continue;
            };
            let _ = p.tx.send(ToClient::Lines { lines: vec![line] });
            let _ = p.tx.send(update.clone());
        }
        if !was_following {
            self.say1(id, Line::col(Col::Dim, "You're following this board now: you'll hear about new notes wherever you are. (unfollow to stop)"));
        }
        Ok(())
    }

    // ---- hooks: outside tools posting here ---------------------------------

    /// The board here that `id` may put hooks on: its maker, or a keeper.
    /// Not the Island Journal, which is the island's own.
    fn hookable_board(&self, id: u64) -> Option<i64> {
        let (x, y, _) = self.here(id)?;
        let user = self.user(id);
        let Some((board, journal)) = self.board_here(x, y) else {
            self.say1(id, Line::plain("Hooks belong to noticeboards. Stand at one (or make a noticeboard)."));
            return None;
        };
        if journal {
            self.say1(id, Line::plain("The Island Journal is the island's own: put hooks on a noticeboard instead."));
            return None;
        }
        let maker = self.world.things.iter().find(|t| t.id == board).is_some_and(|t| t.maker == user);
        if !maker && !self.is_keeper(&user) {
            self.say1(id, Line::plain("Only whoever put this board up (or an island keeper) can change its hooks."));
            return None;
        }
        Some(board)
    }

    /// `hook CI`: a secret address outside tools can post notes to.
    pub(super) fn cmd_hook(&mut self, id: u64, name: &str) -> anyhow::Result<()> {
        let Some(board) = self.hookable_board(id) else { return Ok(()) };
        let name = clean_hook_name(name);
        if name.is_empty() {
            self.say1(id, Line::plain("Name it after whatever will post, e.g. 'hook CI' or 'hook Deploys'."));
            return Ok(());
        }
        if self.world.db.hooks(board)?.iter().any(|h| h.0.eq_ignore_ascii_case(&name)) {
            self.say1(id, Line::plain(format!("This board already has a hook called {name}. For a fresh address: unhook {name}, then hook {name}.")));
            return Ok(());
        }
        let secret = crate::server::accounts::new_secret()?;
        self.world.db.add_hook(board, &name, &crate::server::names::hash(&secret), &self.user(id), now())?;
        let address = hook_url(self.url.as_deref(), &secret);
        self.send(
            id,
            vec![
                Line::new().b(Col::Yellow, format!("🔔 {name} can post here now. Its secret address:")),
                Line::new().t("   ").b(Col::Yellow, address.clone()),
                Line::col(Col::Dim, r#"   POST plain text, or JSON with "text" (Slack style) or "content" (Discord style). For example:"#),
                Line::col(Col::Dim, format!("   curl -d 'the build passed 🎉' {address}")),
                Line::col(Col::Dim, "   It's only shown this once, and anyone with it can post here: keep it like a password. (unhook to stop it)"),
            ],
        );
        Ok(())
    }

    pub(super) fn cmd_unhook(&mut self, id: u64, name: &str) -> anyhow::Result<()> {
        let Some(board) = self.hookable_board(id) else { return Ok(()) };
        let name = clean_hook_name(name);
        if name.is_empty() {
            self.say1(id, Line::plain("Stop which one? 'hooks' lists them."));
        } else if self.world.db.remove_hook(board, &name)? {
            self.say1(id, Line::col(Col::Yellow, format!("🔕 {name} can't post here any more.")));
        } else {
            self.say1(id, Line::plain(format!("This board has no hook called {name}. ('hooks' lists them)")));
        }
        Ok(())
    }

    pub(super) fn cmd_hooks(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        let Some((board, _)) = self.board_here(x, y) else {
            self.say1(id, Line::plain("Stand at a noticeboard to see its hooks."));
            return Ok(());
        };
        let hooks = self.world.db.hooks(board)?;
        let mut lines = vec![Line::new().b(Col::Magenta, "🔔 Hooks on this board")];
        if hooks.is_empty() {
            lines.push(Line::col(Col::Dim, "  None yet. Its maker or a keeper can add one: hook CI"));
        }
        for (name, by, _, last) in hooks {
            let last = last.map_or("hasn't posted yet".to_string(), |t| format!("last posted {}", when(t)));
            lines.push(Line::new().t("  ").b(Col::Yellow, name).c(Col::Dim, format!("  made by {} · {last}", self.world.nick(&by))));
        }
        self.send(id, lines);
        Ok(())
    }

    /// An outside tool posting through its hook's secret address.
    pub(super) fn hook_post(&mut self, secret: &str, text: &str) -> crate::server::HookReply {
        use crate::server::HookReply;
        let Some((hook, board, name)) = self.world.db.hook_by_secret(&crate::server::names::hash(secret)) else {
            return HookReply::Unknown;
        };
        // A board that's been unmade keeps its hooks, quietly, in case of undo.
        let Some((x, y)) = self.board_place(board) else { return HookReply::Unknown };
        let soon = self.hook_last.get(&hook).is_some_and(|t| t.elapsed() < HOOK_GAP);
        if soon {
            return HookReply::TooFast;
        }
        self.hook_last.insert(hook, std::time::Instant::now());
        let text: String = if text.chars().count() > MAX_NOTE {
            text.chars().take(MAX_NOTE - 1).chain(['…']).collect()
        } else {
            text.to_string()
        };
        let author = format!("🔔 {name}");
        let at = now();
        let Ok(note_id) = self.world.db.add_note(board, &author, &text, at, &week_key(at), None) else {
            return HookReply::Unknown;
        };
        let _ = self.world.db.touch_hook(hook, at);
        let place = self.world.place_title(x, y);
        let update = self.board_update(board, Note { id: note_id, author: author.clone(), text: text.clone(), when: when(at) });
        let followers = self.world.db.followers(board).unwrap_or_default();
        for p in self.players.values() {
            let line = if (p.x, p.y) == (x, y) {
                Line::new().b(Col::Yellow, &author).c(Col::Yellow, " posts on the noticeboard: ").c(Col::White, text.clone())
            } else if followers.contains(&p.user) {
                Line::new().b(Col::Yellow, &author).c(Col::Yellow, format!(" posted at {place}: ")).c(Col::White, text.clone())
            } else {
                continue;
            };
            let _ = p.tx.send(ToClient::Lines { lines: vec![line] });
            let _ = p.tx.send(update.clone());
        }
        HookReply::Posted
    }

    /// The boards someone follows that are still standing, in `boards` order.
    fn followed_boards(&self, user: &str) -> Vec<(i64, i64)> {
        self.world.db.followed(user).unwrap_or_default().into_iter().filter(|(b, _)| self.board_place(*b).is_some()).collect()
    }

    /// `follow` / `unfollow` here, or `unfollow 2` from anywhere (the
    /// number from `boards`).
    pub(super) fn cmd_follow(&mut self, id: u64, follow: bool, which: &str) -> anyhow::Result<()> {
        let Some((x, y, _name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let which = which.trim();
        let board = if let Ok(n) = which.parse::<usize>() {
            match self.followed_boards(&user).get(n.wrapping_sub(1)) {
                Some(&(b, _)) if !follow => b,
                Some(_) => {
                    self.say1(id, Line::plain("You already follow that one."));
                    return Ok(());
                }
                None => {
                    self.say1(id, Line::plain("Which one? 'boards' lists the boards you follow."));
                    return Ok(());
                }
            }
        } else {
            match self.board_here(x, y) {
                Some((b, _)) => b,
                None => {
                    let msg = if follow { "Stand at a noticeboard to follow it." } else { "Stand at a noticeboard to unfollow it, or 'unfollow 2' (the number from 'boards') from anywhere." };
                    self.say1(id, Line::plain(msg));
                    return Ok(());
                }
            }
        };
        self.world.db.set_following(&user, board, follow)?;
        let msg = if follow {
            "📌 Following. You'll hear about new notes here wherever you are.".to_string()
        } else {
            format!("📌 Unfollowed {}. It'll keep quiet now.", self.board_title(board).trim_start_matches("📌 "))
        };
        self.say1(id, Line::col(Col::Yellow, msg));
        Ok(())
    }

    /// Whether `add` here means adding people to a board.
    pub(super) fn adding_to_board(&self, id: u64, rest: &str) -> bool {
        let r = rest.trim().to_lowercase();
        !r.is_empty() && !r.starts_with("song") && self.players.get(&id).is_some_and(|p| self.board_here(p.x, p.y).is_some_and(|b| !b.1))
    }

    /// `add Poppy, Sam to board`: they follow this board now, like being
    /// added to a group chat. Anyone who follows it (or made it, or a keeper)
    /// can add people, and anyone added can leave with unfollow.
    pub(super) fn cmd_add_to_board(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let Some((board, false)) = self.board_here(x, y) else {
            self.say1(id, Line::plain("Stand at a noticeboard to add people to it. (Not the Island Journal: that's everyone's.)"));
            return Ok(());
        };
        let maker = self.world.things.iter().find(|t| t.id == board).is_some_and(|t| t.maker == user);
        let member = self.world.db.membership(&user, board).is_some_and(|m| m.0);
        if !member && !maker && !self.is_keeper(&user) {
            self.say1(id, Line::plain("Follow this board first, then you can add people to it."));
            return Ok(());
        }
        let mut who = rest.trim();
        for tail in [" to the noticeboard", " to noticeboard", " to the board", " to board", " to this board"] {
            if who.to_lowercase().ends_with(tail) {
                who = &who[..who.len() - tail.len()];
            }
        }
        let names: Vec<&str> =
            who.split([',', '&']).flat_map(|p| p.split(" and ")).map(|n| n.trim().trim_start_matches('@').trim()).filter(|n| !n.is_empty()).collect();
        if names.is_empty() {
            self.say1(id, Line::plain("Add who? e.g. 'add Poppy, Sam to board'"));
            return Ok(());
        }
        let place = self.world.place_title(x, y);
        let mut added = Vec::new();
        let mut already = Vec::new();
        for n in names {
            let found = self.world.db.matching_users(n)?;
            let [them] = found.as_slice() else {
                let why = if found.is_empty() { "there's nobody called" } else { "more than one person could be" };
                self.say1(id, Line::plain(format!("Skipping {n}: {why} {n}. (Usernames from 'users' always work.)")));
                continue;
            };
            let nick = self.world.nick(them);
            if self.world.db.membership(them, board).is_some_and(|m| m.0) {
                already.push(nick);
                continue;
            }
            self.world.db.set_following(them, board, true)?;
            let n_boards = self.followed_boards(them).iter().position(|(b, _)| *b == board).map_or(1, |i| i + 1);
            let note = format!("📌 added you to the noticeboard at {place}: you'll hear its notes wherever you are. (boards to see it, unfollow {n_boards} to leave)");
            match self.players.iter().find(|(_, p)| p.user == *them) {
                Some((&pid, p)) if p.hears(&user) => self.say1(pid, Line::new().b(Col::Yellow, format!("📌 {name}")).c(Col::Yellow, note.trim_start_matches("📌"))),
                Some(_) => {}
                None => self.world.db.send_mail(them, &user, &note, now())?,
            }
            added.push(nick);
        }
        if !added.is_empty() {
            let verb = if added.len() == 1 { "follows" } else { "follow" };
            self.say1(id, Line::col(Col::Yellow, format!("📌 {} {verb} this board now. They'll hear its notes (and place events here) wherever they are.", join(&added))));
        }
        if !already.is_empty() {
            self.say1(id, Line::col(Col::Dim, format!("   {} already follow{} it.", join(&already), if already.len() == 1 { "s" } else { "" })));
        }
        Ok(())
    }

    pub(super) fn cmd_boards(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((_, _, _name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let followed = self.followed_boards(&user);
        let mut lines = vec![Line::new().b(Col::Magenta, "📌 Boards you follow")];
        if followed.is_empty() {
            lines.push(Line::col(Col::Dim, "  None yet. Post on a board to follow it."));
        }
        for (i, (board, seen)) in followed.iter().enumerate() {
            let Some((bx, by)) = self.board_place(*board) else { continue };
            let new = self.world.db.notes_after(*board, *seen)?;
            let news = if new > 0 { format!("  {new} new") } else { String::new() };
            lines.push(
                Line::new()
                    .c(Col::Yellow, format!("  {}. ", i + 1))
                    .t(self.board_title(*board))
                    .c(Col::Dim, format!("  ({})", self.world.place_title(bx, by)))
                    .b(Col::Yellow, news),
            );
        }
        if !followed.is_empty() {
            lines.push(Line::col(Col::Dim, "  unfollow 2 leaves one from anywhere"));
        }
        if let Some(j) = self.journal_board {
            if let Some((jx, jy)) = self.board_place(j) {
                lines.push(Line::col(Col::Dim, format!("  The Island Journal stands at {}.", self.world.place_title(jx, jy))));
            }
        }
        self.send(id, lines);
        Ok(())
    }

    pub(super) fn cmd_archive(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        let Some((board, true)) = self.board_here(x, y) else {
            self.say1(id, Line::plain("The archive lives with the Island Journal. Go and find it!"));
            return Ok(());
        };
        let this_week = week_key(now());
        let weeks: Vec<(String, i64)> = self.world.db.weeks(board)?.into_iter().filter(|(w, _)| *w != this_week).collect();
        let mut lines = vec![Line::new().b(Col::Magenta, "📖 The Island Journal: past weeks")];
        if weeks.is_empty() {
            lines.push(Line::col(Col::Dim, "  This is the very first week. Come back next Monday!"));
        }
        for (w, n) in weeks {
            let notes = if n == 1 { "1 note".to_string() } else { format!("{n} notes") };
            lines.push(Line::new().t(format!("  {} · {notes}", week_label(&w))).c(Col::Dim, format!("   read week {w}")));
        }
        self.send(id, lines);
        Ok(())
    }
}

/// Write every week's first find of each species into the journal, from
/// everyone's personal journals, skipping any already there. Earlier weeks
/// land in the archive. Returns how many it added.
pub fn backfill_journal(db: &crate::server::db::Db, board: i64) -> anyhow::Result<usize> {
    let mut added = 0;
    for (player, sid, at) in db.discoveries_since(0)? {
        let Some(s) = crate::server::nature::by_id(&sid) else { continue };
        let week = week_key(at);
        if db.species_noted(board, s.id, &week)? {
            continue;
        }
        db.add_note(board, &player, &discovery_text(s), at, &week, Some(s.id))?;
        added += 1;
    }
    Ok(added)
}

fn discovery_text(s: &Species) -> String {
    format!("found {} {}", crate::server::nature::a(s.name), s.kind.icon())
}

fn join(names: &[String]) -> String {
    crate::server::world::join_and(names)
}

/// The least time between one hook's posts.
const HOOK_GAP: std::time::Duration = std::time::Duration::from_secs(2);

fn clean_hook_name(raw: &str) -> String {
    let n: String = raw.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.')).collect();
    n.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(24).collect()
}

/// Where a hook posts to. Without a known address, a placeholder.
fn hook_url(url: Option<&str>, secret: &str) -> String {
    match url {
        Some(u) => format!("{}/hook/{secret}", u.trim_end_matches('/')),
        None => format!("<world address>/hook/{secret}"),
    }
}

pub fn board_kind() -> &'static str {
    BOARD_KIND
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weeks_start_on_monday() {
        let k = week_key(now());
        let d = NaiveDate::parse_from_str(&k, "%Y-%m-%d").unwrap();
        assert_eq!(d.weekday(), chrono::Weekday::Mon);
        assert!(week_start(&k).unwrap() <= now());
    }

    #[test]
    fn hook_names_are_tidy() {
        assert_eq!(clean_hook_name("  CI   builds <b> "), "CI builds b");
        assert_eq!(clean_hook_name("a-very-long-name-for-a-hook-indeed"), "a-very-long-name-for-a-h");
        assert_eq!(hook_url(Some("https://w.example/"), "abc"), "https://w.example/hook/abc");
    }

    #[test]
    fn spots_boards_by_name() {
        assert!(is_board_name("a little noticeboard"));
        assert!(is_board_name("a Notice Board by the pond"));
        assert!(!is_board_name("a skateboard"));
    }
}
