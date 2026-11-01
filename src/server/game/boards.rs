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

        let week = week_key(now());
        let since = week_start(&week).unwrap_or(now());
        let mut noted: Vec<String> = Vec::new();
        for (player, sid, at) in self.world.db.discoveries_since(since)? {
            if noted.contains(&sid) {
                continue;
            }
            if let Some(s) = crate::server::nature::by_id(&sid) {
                self.world.db.add_note(id, &player, &discovery_text(s), at, &week, Some(s.id))?;
                noted.push(sid);
            }
        }
        println!("📖 put up the Island Journal ({} discoveries so far this week)", noted.len());
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
            .b(Col::Yellow, player)
            .c(Col::Yellow, format!(" added {} to the Island Journal!", crate::server::nature::a(s.name)));
        let note = Note { id: note_id, author: player.into(), text, when: when(at) };
        for p in self.players.values() {
            let _ = p.tx.send(ToClient::Lines { lines: vec![line.clone()] });
            let _ = p.tx.send(self.board_update(board, note.clone()));
        }
        Ok(())
    }

    // ---- finding boards --------------------------------------------------

    fn board_here(&self, x: i64, y: i64) -> Option<(i64, bool)> {
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
        let seen = self.world.db.membership(&p.name, board).map_or(0, |m| m.1);
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
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
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
            self.world.db.set_seen(&name, board, self.world.db.last_note(board)?)?;
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
            .map(|n| Note { id: n.id, author: n.author, text: n.text, when: when(n.at) })
            .collect();
        let msg = ToClient::Board { board, title: self.board_title(board), mode, notes, more, page, weeks };
        if let Some(p) = self.players.get(&id) {
            let _ = p.tx.send(msg);
        }
        Ok(())
    }

    pub(super) fn cmd_post(&mut self, id: u64, text: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
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
        let was_following = self.world.db.membership(&name, board).is_some_and(|m| m.0);
        self.world.db.set_following(&name, board, true)?;
        self.world.db.set_seen(&name, board, note_id)?;

        let what = if journal { "the Island Journal" } else { "the noticeboard" };
        let place = self.world.place_title(x, y);
        let note = Note { id: note_id, author: name.clone(), text: text.clone(), when: when(at) };
        let update = self.board_update(board, note);
        let followers = self.world.db.followers(board)?;
        for (&pid, p) in &self.players {
            if pid != id && !p.hears(&name) {
                continue;
            }
            let here = (p.x, p.y) == (x, y);
            let line = if pid == id {
                Line::new().c(Col::Yellow, format!("📌 You post on {what}: ")).c(Col::White, text.clone())
            } else if here {
                Line::new().c(Col::Yellow, "📌 ").b(Col::Yellow, &name).c(Col::Yellow, format!(" posts on {what}: ")).c(Col::White, text.clone())
            } else if followers.contains(&p.name) {
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

    pub(super) fn cmd_follow(&mut self, id: u64, follow: bool) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let Some((board, _)) = self.board_here(x, y) else {
            self.say1(id, Line::plain("Stand at a noticeboard to follow or unfollow it."));
            return Ok(());
        };
        self.world.db.set_following(&name, board, follow)?;
        let msg = if follow {
            "📌 Following. You'll hear about new notes here wherever you are."
        } else {
            "📌 Unfollowed. It'll keep quiet now."
        };
        self.say1(id, Line::col(Col::Yellow, msg));
        Ok(())
    }

    pub(super) fn cmd_boards(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((_, _, name)) = self.here(id) else { return Ok(()) };
        let followed = self.world.db.followed(&name)?;
        let mut lines = vec![Line::new().b(Col::Magenta, "📌 Boards you follow")];
        if followed.is_empty() {
            lines.push(Line::col(Col::Dim, "  None yet. Post on a board to follow it."));
        }
        for (board, seen) in followed {
            let Some((bx, by)) = self.board_place(board) else { continue };
            let new = self.world.db.notes_after(board, seen)?;
            let news = if new > 0 { format!("  {new} new") } else { String::new() };
            lines.push(
                Line::new()
                    .t(format!("  {}", self.board_title(board)))
                    .c(Col::Dim, format!("  ({})", self.world.place_title(bx, by)))
                    .b(Col::Yellow, news),
            );
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

fn discovery_text(s: &Species) -> String {
    format!("found {} {}", crate::server::nature::a(s.name), s.kind.icon())
}

pub fn journal_kind() -> &'static str {
    JOURNAL_KIND
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
    fn spots_boards_by_name() {
        assert!(is_board_name("a little noticeboard"));
        assert!(is_board_name("a Notice Board by the pond"));
        assert!(!is_board_name("a skateboard"));
    }
}
