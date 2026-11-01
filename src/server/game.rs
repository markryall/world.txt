//! The one task that owns the world. Connections send it events; it
//! answers with lines of text and status updates.

mod boards;
mod craft;
mod gather;
mod homes;

use super::nature::Find;
use super::rng::Rng;
use super::terrain::Terrain;
use super::tutorial::{self, Act};
use super::weather::{Clock, Phase, Sky};
use super::world::{DIRS, SIZE, Trigger, World, capitalise, in_bounds, strip_article};
use super::{ClientTx, Event};
use crate::protocol::{Col, Line, ToClient};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

/// Starting a line with any of these says it out loud.
const SAY_QUOTES: [char; 6] = ['\'', '"', '‘', '’', '“', '”'];
const TICK: Duration = Duration::from_secs(3);
const SHOUT_RANGE: i64 = 10;

struct Player {
    name: String,
    x: i64,
    y: i64,
    tx: ClientTx,
    sky: Sky,
    bot: bool,
    /// Who last told us something, for `reply`.
    reply_to: Option<String>,
    /// First-steps progress: an index into tutorial::STEPS, or DONE.
    tutorial: u8,
    /// Ticks until something bites, while a line is in the water.
    fishing: Option<u8>,
    /// Which board they're reading, for `read more`.
    reading: Option<boards::Reading>,
    /// Lowercased names this player doesn't want to hear from.
    muted: std::collections::HashSet<String>,
    /// Hash of the device key they joined with, so a kick covers the device too.
    key_hash: Option<String>,
}

impl Player {
    fn hears(&self, from: &str) -> bool {
        !self.muted.contains(&from.to_lowercase())
    }
}

struct Game {
    world: World,
    players: HashMap<u64, Player>,
    phase: Phase,
    rng: Rng,
    /// Flowers, butterflies, shells... waiting to be gathered.
    finds: Vec<Find>,
    /// The Island Journal's thing id.
    journal_board: Option<i64>,
    /// Names that may tidy up anyone's things.
    keepers: Vec<String>,
}

pub async fn run(world: World, mut events: UnboundedReceiver<Event>, keepers: Vec<String>) {
    let seed = world.seed;
    let mut game = Game { world, players: HashMap::new(), phase: Clock::phase(), rng: Rng::new(seed), finds: Vec::new(), journal_board: None, keepers };
    if let Err(e) = game.ensure_journal() {
        eprintln!("couldn't put up the Island Journal: {e}");
    }
    let mut tick = tokio::time::interval(TICK);
    loop {
        tokio::select! {
            ev = events.recv() => match ev {
                Some(ev) => game.handle(ev),
                None => break,
            },
            _ = tick.tick() => game.tick(),
        }
    }
}

impl Game {
    fn handle(&mut self, ev: Event) {
        match ev {
            Event::Join { id, name, bot, key, tx } => self.join(id, name, bot, key, tx),
            Event::Leave { id } => self.leave(id),
            Event::Cmd { id, text } => {
                let before = self.snapshot(id);
                if let Err(e) = self.command(id, text.trim()) {
                    eprintln!("command error: {e}");
                    self.send(id, vec![Line::col(Col::Red, "Something went wobbly. Try again?")]);
                }
                if let Some(act) = self.what_happened(id, text.trim(), before) {
                    self.progress(id, act);
                }
            }
        }
    }

    // ---- sending -------------------------------------------------------

    fn send(&self, id: u64, lines: Vec<Line>) {
        if let Some(p) = self.players.get(&id) {
            let _ = p.tx.send(ToClient::Lines { lines });
        }
    }

    fn say1(&self, id: u64, line: Line) {
        self.send(id, vec![line]);
    }

    /// Something `from` said or did, to everyone at (x, y) who hasn't muted them.
    fn tell_here_from(&self, x: i64, y: i64, except: u64, from: &str, line: Line) {
        for (&pid, p) in &self.players {
            if pid != except && p.x == x && p.y == y && p.hears(from) {
                let _ = p.tx.send(ToClient::Lines { lines: vec![line.clone()] });
            }
        }
    }

    /// Everyone standing at (x, y) except `except`.
    fn tell_here(&self, x: i64, y: i64, except: u64, line: Line) {
        for (&pid, p) in &self.players {
            if pid != except && p.x == x && p.y == y {
                let _ = p.tx.send(ToClient::Lines { lines: vec![line.clone()] });
            }
        }
    }

    fn others_here(&self, id: u64) -> Vec<String> {
        let Some(me) = self.players.get(&id) else { return vec![] };
        self.players
            .iter()
            .filter(|(pid, p)| **pid != id && p.x == me.x && p.y == me.y)
            .map(|(_, p)| p.name.clone())
            .collect()
    }

    fn status(&self, id: u64) {
        let Some(p) = self.players.get(&id) else { return };
        let sky = self.world.weather.sky_at(p.x, p.y);
        let people: Vec<(i64, i64)> =
            self.players.iter().filter(|(pid, _)| **pid != id).map(|(_, o)| (o.x, o.y)).collect();
        let msg = ToClient::Status {
            place: self.world.place_title(p.x, p.y),
            weather: format!(
                "{} {} · wind from the {}",
                sky.icon(Clock::phase()),
                sky.label(),
                self.world.weather.wind_label()
            ),
            clock: Clock::label(),
            map: self.world.minimap(p.x, p.y, &people, &self.find_positions()),
            here: self.others_here(id),
            humans: self.players.values().filter(|p| !p.bot).count() as u32,
        };
        let _ = p.tx.send(msg);
    }

    fn status_all(&self) {
        for &id in self.players.keys() {
            self.status(id);
        }
    }

    fn look(&self, id: u64) {
        let Some(p) = self.players.get(&id) else { return };
        let mut lines = self.world.describe(p.x, p.y, &self.others_here(id));
        let exits = lines.pop();
        lines.extend(self.board_lines_here(id));
        lines.extend(self.find_lines(p.x, p.y));
        lines.extend(exits);
        self.send(id, lines);
    }

    // ---- arriving and leaving -----------------------------------------

    fn join(&mut self, id: u64, raw: String, bot: bool, key: Option<String>, tx: ClientTx) {
        let name = clean_name(&raw);
        if name.is_empty() {
            let _ = tx.send(ToClient::Bye { reason: "Names need a letter or two, darling.".into() });
            return;
        }
        if self.players.values().any(|p| p.name.eq_ignore_ascii_case(&name)) {
            let _ = tx.send(ToClient::Bye { reason: format!("Someone called {name} is already here.") });
            return;
        }
        let claim = match super::names::check(self.world.db.name_key(&name).as_deref(), key.as_deref()) {
            super::names::NameCheck::Taken(tried) => {
                let reason = match tried {
                    Some(hash) => {
                        let fresh = super::names::knock_code(self.rng.next());
                        match self.world.db.add_knock(&name, &hash, &fresh) {
                            Ok(code) => {
                                self.tell_keepers_about_knocks();
                                println!("🔐 {name} knocked from a new device (ask them for their code, then: welcome {name} <code>)");
                                format!(
                                    "🔐 {name} is linked to another device. To use this one, ask an island keeper \
                                     to let you in and tell them this code: {code} \
                                     Once they say yes, just join again. (Or pick a different name.)"
                                )
                            }
                            Err(e) => {
                                eprintln!("remembering a knock for {name}: {e}");
                                format!("🔐 The name {name} is linked to another device. Try a different name!")
                            }
                        }
                    }
                    None => format!("🔐 The name {name} is linked to another device. Try a different name!"),
                };
                let _ = tx.send(ToClient::Bye { reason });
                return;
            }
            super::names::NameCheck::Claim(hash) => Some(hash),
            super::names::NameCheck::Welcome => None,
        };
        let key_hash = key.as_deref().map(str::trim).filter(|k| !k.is_empty()).map(super::names::hash);
        let now = boards::now();
        if !self.is_keeper(&name) {
            if let Some(until) = self.world.db.banned_until(&name, key_hash.as_deref(), now) {
                let mins = ((until - now) / 60).max(1);
                let _ = tx.send(ToClient::Bye {
                    reason: format!("🌙 A keeper has asked you to take a break. You can come back in about {mins} minutes."),
                });
                return;
            }
        }
        let saved = self.world.db.player(&name);
        let first_time = saved.is_none() && !bot;
        let tutorial = if bot { tutorial::DONE } else { saved.map_or(0, |s| s.2) };
        let (x, y) = saved
            .map(|s| (s.0, s.1))
            .filter(|&(x, y)| self.world.terrain(x, y).walkable())
            .unwrap_or_else(|| self.world.spawn_point());
        let sky = self.world.weather.sky_at(x, y);
        let muted = self.world.db.mutes(&name).unwrap_or_default().into_iter().map(|m| m.to_lowercase()).collect();
        if let Some(hash) = claim {
            if let Err(e) = self.world.db.claim_name(&name, &hash, x, y) {
                eprintln!("claiming {name}: {e}");
            }
        }
        self.players.insert(id, Player { name: name.clone(), x, y, tx, sky, bot, reply_to: None, tutorial, fishing: None, reading: None, muted, key_hash });

        self.send(
            id,
            vec![
                Line::new().b(Col::Magenta, "✨ world.txt ✨"),
                Line::col(Col::Dim, "A small island with no battles, just vibes. Type 'help' if you're lost."),
                Line::new(),
            ],
        );
        self.look(id);
        self.status(id);
        if first_time {
            self.say1(id, Line::new().b(Col::Yellow, format!("🐣 Welcome to the island, {name}! Here's how to get started.")));
        }
        self.show_hint(id);
        if self.is_keeper(&name) {
            self.tell_keepers_about_knocks();
        }
        for (&pid, p) in &self.players {
            if pid != id {
                let line = if p.x == x && p.y == y {
                    Line::new().b(Col::Cyan, &name).c(Col::Cyan, " appears, blinking, as if from a dream.")
                } else if first_time {
                    Line::col(Col::Dim, format!("🐣 {name} has arrived on the island for the first time!"))
                } else {
                    Line::col(Col::Dim, format!("✧ {name} has wandered into the world."))
                };
                let _ = p.tx.send(ToClient::Lines { lines: vec![line] });
            }
        }
        self.status_all();
    }

    fn leave(&mut self, id: u64) {
        let Some(p) = self.players.remove(&id) else { return };
        if p.bot {
            // Residents keep no position or progress.
        } else if let Err(e) = self.world.db.save_player(&p.name, p.x, p.y, p.tutorial) {
            eprintln!("saving {}: {e}", p.name);
        }
        for o in self.players.values() {
            let line = if o.x == p.x && o.y == p.y {
                Line::col(Col::Cyan, format!("{} curls up and fades softly away.", p.name))
            } else {
                Line::col(Col::Dim, format!("✧ {} has drifted off to sleep.", p.name))
            };
            let _ = o.tx.send(ToClient::Lines { lines: vec![line] });
        }
        self.status_all();
    }

    // ---- commands -----------------------------------------------------

    fn command(&mut self, id: u64, input: &str) -> anyhow::Result<()> {
        if input.is_empty() {
            return Ok(());
        }
        // Shortcuts: 'hello  and  :waves
        // Phones like to "smarten" quotes, so curly ones count too.
        if let Some(rest) = input.strip_prefix(SAY_QUOTES) {
            return self.cmd_say(id, rest.trim());
        }
        if let Some(rest) = input.strip_prefix(':') {
            return self.cmd_emote(id, rest.trim());
        }
        // @Poppy hi  → tell Poppy hi;  @all hi → chat hi
        if let Some(rest) = input.strip_prefix('@') {
            let (to, msg) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            if to.eq_ignore_ascii_case("all") || to.eq_ignore_ascii_case("everyone") {
                return self.cmd_chat(id, msg.trim());
            }
            return self.cmd_tell(id, to, msg.trim());
        }
        let (verb, rest) = match input.split_once(char::is_whitespace) {
            Some((v, r)) => (v.to_lowercase(), r.trim()),
            None => (input.to_lowercase(), ""),
        };

        if let Some(&(_, _, dx, dy)) = DIRS.iter().find(|d| d.0 == verb || d.1 == verb) {
            return self.walk(id, dx, dy);
        }
        if homes::BUILDING.contains(&verb.as_str()) && !self.may_build_here(id) {
            return Ok(());
        }

        match verb.as_str() {
            "help" | "?" => self.send(id, help()),
            "look" | "l" | "examine" | "x" if rest.is_empty() => self.look(id),
            "look" | "l" | "examine" | "x" => self.cmd_examine(id, rest),
            "go" | "walk" => match DIRS.iter().find(|d| d.0 == rest || d.1 == rest) {
                Some(&(_, _, dx, dy)) => return self.walk(id, dx, dy),
                None => self.say1(id, Line::plain("Go where? Try north, south, east, west, or ne/nw/se/sw.")),
            },
            "say" => return self.cmd_say(id, rest),
            "emote" | "me" => return self.cmd_emote(id, rest),
            "shout" | "yell" | "call" => return self.cmd_shout(id, rest),
            "hum" | "sing" | "whistle" => return self.cmd_hum(id, &verb, rest),
            "who" => self.cmd_who(id),
            "map" | "m" => {
                if let Some(p) = self.players.get(&id) {
                    let people: Vec<(i64, i64)> =
                        self.players.iter().filter(|(pid, _)| **pid != id).map(|(_, o)| (o.x, o.y)).collect();
                    let mut lines = vec![Line::new().b(Col::Magenta, "🗺  The island")];
                    lines.extend(self.world.island_map((p.x, p.y), &people));
                    self.send(id, lines);
                }
            }
            "claim" => return self.cmd_claim(id),
            "home" => return self.cmd_home(id),
            "invite" => return self.cmd_invite(id, rest, true),
            "uninvite" => return self.cmd_invite(id, rest, false),
            "guests" => return self.cmd_guests(id),
            "read" => return self.cmd_read(id, rest),
            "post" | "note" => return self.cmd_post(id, rest),
            "follow" => return self.cmd_follow(id, true),
            "unfollow" => return self.cmd_follow(id, false),
            "boards" => return self.cmd_boards(id),
            "archive" => return self.cmd_archive(id),
            "gather" | "pick" | "catch" | "collect" | "forage" | "g" => return self.cmd_gather(id, rest),
            "fish" => return self.cmd_fish(id),
            "satchel" | "bag" | "inventory" | "inv" | "i" => return self.cmd_satchel(id),
            "journal" | "j" => return self.cmd_journal(id),
            "give" => return self.cmd_give(id, rest),
            "place" | "drop" | "put" => return self.cmd_place(id, rest),
            "release" | "free" => return self.cmd_release(id, rest),
            "skip" => {
                if let Some(p) = self.players.get_mut(&id) {
                    p.tutorial = tutorial::DONE;
                }
                self.show_hint(id);
                self.save_progress(id);
                self.say1(id, Line::col(Col::Dim, "Okay! Type tutorial any time to see the first steps again."));
            }
            "tutorial" | "start" => {
                if let Some(p) = self.players.get_mut(&id) {
                    p.tutorial = 0;
                }
                self.show_hint(id);
            }
            "tell" | "whisper" | "msg" | "t" => {
                let (to, msg) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
                return self.cmd_tell(id, to, msg.trim());
            }
            "reply" | "r" => {
                let to = self.players.get(&id).and_then(|p| p.reply_to.clone());
                match to {
                    Some(to) => return self.cmd_tell(id, &to, rest),
                    None => self.say1(id, Line::plain("Nobody's told you anything yet. Try '@Name hello'.")),
                }
            }
            "chat" | "all" => return self.cmd_chat(id, rest),
            "find" | "where" => self.cmd_find(id, rest.trim_start_matches("is ").trim()),
            "sit" => return self.cmd_emote(id, "sits down for a while."),
            "lie" | "rest" => return self.cmd_emote(id, "lies back and gazes at the sky."),
            "dance" => return self.cmd_emote(id, "dances, a little twirl and a sway. 💃"),
            "wave" => return self.cmd_emote(id, "waves. 👋"),
            "hug" if !rest.is_empty() => return self.cmd_emote(id, &format!("hugs {rest}. 🫂")),
            "dig" => return self.cmd_dig(id),
            "fill" => return self.cmd_fill(id),
            "plant" => return self.cmd_plant(id, rest),
            "raise" | "pile" => return self.cmd_raise(id),
            "path" | "pave" => return self.cmd_path(id),
            "name" | "call-this" => return self.cmd_name(id, rest),
            "make" | "build" | "create" => return self.cmd_make(id, rest),
            "craft" => return self.cmd_craft(id, rest),
            "recipes" => return self.cmd_recipes(id),
            "unmake" | "destroy" | "remove" => return self.cmd_unmake(id, rest),
            "undo" => return self.cmd_undo(id),
            "welcome" | "letin" => return self.cmd_welcome(id, rest),
            "unlink" => return self.cmd_unlink(id, rest),
            "mute" | "block" => return self.cmd_mute(id, rest, true),
            "unmute" | "unblock" => return self.cmd_mute(id, rest, false),
            "mutes" => return self.cmd_mutes(id),
            "kick" => return self.cmd_kick(id, rest),
            "unkick" => return self.cmd_unkick(id, rest),
            "push" | "carry" | "move" => return self.cmd_push(id, rest),
            "quit" | "exit" | "bye" => {
                if let Some(p) = self.players.get(&id) {
                    let _ = p.tx.send(ToClient::Bye { reason: "Sweet dreams. ✨".into() });
                }
            }
            _ => self.say1(id, Line::col(Col::Dim, format!("Hmm, '{verb}' isn't something you know how to do. Try 'help'."))),
        }
        Ok(())
    }

    fn walk(&mut self, id: u64, dx: i64, dy: i64) -> anyhow::Result<()> {
        let Some(p) = self.players.get(&id) else { return Ok(()) };
        let (ox, oy, name) = (p.x, p.y, p.name.clone());
        let (nx, ny) = (ox + dx, oy + dy);
        let to = self.world.terrain(nx, ny);
        if !in_bounds(nx, ny) || !to.walkable() {
            self.say1(id, Line::col(Col::Blue, "The water gets too deep to wade any further. You turn back."));
            return Ok(());
        }
        let dir = DIRS.iter().find(|d| d.2 == dx && d.3 == dy).map_or("away", |d| d.1);
        let back = DIRS.iter().find(|d| d.2 == -dx && d.3 == -dy).map_or("somewhere", |d| d.1);
        self.tell_here(ox, oy, id, Line::col(Col::Cyan, format!("{name} wanders off to the {dir}.")));
        if let Some(p) = self.players.get_mut(&id) {
            p.x = nx;
            p.y = ny;
            p.sky = self.world.weather.sky_at(nx, ny);
            p.fishing = None; // walking off reels the line in
        }
        self.tell_here(nx, ny, id, Line::col(Col::Cyan, format!("{name} arrives from the {back}.")));
        if to.is_wet() && !self.world.terrain(ox, oy).is_wet() {
            self.say1(id, Line::col(Col::Blue, "You wade in. The water is cool around your legs."));
        }
        self.look(id);
        // Things that respond to arrivals.
        let reactions: Vec<String> = self
            .world
            .things_at(nx, ny)
            .filter(|t| t.trigger == Trigger::Arrive)
            .filter_map(|t| t.action.as_ref().map(|a| format!("{} {a}", the(&t.name))))
            .collect();
        for r in reactions {
            let line = Line::col(Col::Yellow, sentence(&r));
            self.say1(id, line.clone());
            self.tell_here(nx, ny, id, line);
        }
        // Save where they are with every step, so a crash or power cut loses nothing.
        self.save_progress(id);
        self.status_all();
        Ok(())
    }

    fn cmd_say(&mut self, id: u64, msg: &str) -> anyhow::Result<()> {
        if msg.is_empty() {
            self.say1(id, Line::plain("Say what, babe?"));
            return Ok(());
        }
        let Some(p) = self.players.get(&id) else { return Ok(()) };
        let (x, y, name) = (p.x, p.y, p.name.clone());
        self.say1(id, Line::new().c(Col::Dim, "You say, ").c(Col::White, format!("\"{msg}\"")));
        self.tell_here_from(x, y, id, &name, Line::new().b(Col::Cyan, &name).c(Col::Dim, " says, ").c(Col::White, format!("\"{msg}\"")));
        self.react_to_words(x, y);
        Ok(())
    }

    fn cmd_emote(&mut self, id: u64, msg: &str) -> anyhow::Result<()> {
        if msg.is_empty() {
            self.say1(id, Line::plain("Emote what? e.g. ':stretches and yawns'"));
            return Ok(());
        }
        let Some(p) = self.players.get(&id) else { return Ok(()) };
        let (x, y, name) = (p.x, p.y, p.name.clone());
        let line = Line::new().b(Col::Cyan, &name).c(Col::Cyan, format!(" {msg}"));
        self.say1(id, line.clone());
        self.tell_here_from(x, y, id, &name, line);
        Ok(())
    }

    fn cmd_shout(&mut self, id: u64, msg: &str) -> anyhow::Result<()> {
        if msg.is_empty() {
            self.say1(id, Line::plain("Shout what?"));
            return Ok(());
        }
        let Some(me) = self.players.get(&id) else { return Ok(()) };
        self.say1(id, Line::new().c(Col::Dim, "You shout, ").b(Col::White, format!("\"{msg}!\"")));
        for (&pid, p) in &self.players {
            if pid == id || !p.hears(&me.name) {
                continue;
            }
            let (dx, dy) = (me.x - p.x, me.y - p.y);
            let d = dx.abs().max(dy.abs());
            if d > SHOUT_RANGE {
                continue;
            }
            let line = if d == 0 {
                Line::new().b(Col::Cyan, &me.name).c(Col::Dim, " shouts, ").b(Col::White, format!("\"{msg}!\""))
            } else {
                let dir = DIRS
                    .iter()
                    .find(|q| q.2 == dx.signum() && q.3 == dy.signum())
                    .map_or("somewhere", |q| q.1);
                if d <= 4 {
                    Line::new()
                        .c(Col::Dim, format!("From the {dir}, "))
                        .b(Col::Cyan, &me.name)
                        .c(Col::Dim, " shouts, ")
                        .c(Col::White, format!("\"{msg}!\""))
                } else {
                    Line::col(Col::Dim, format!("Faintly, from far to the {dir}, someone calls: \"{msg}...\""))
                }
            };
            let _ = p.tx.send(ToClient::Lines { lines: vec![line] });
        }
        Ok(())
    }

    fn cmd_hum(&mut self, id: u64, verb: &str, tune: &str) -> anyhow::Result<()> {
        let Some(p) = self.players.get(&id) else { return Ok(()) };
        let (x, y, name) = (p.x, p.y, p.name.clone());
        let tune = if tune.is_empty() { "a little tune".to_string() } else { format!("♪ {tune} ♪") };
        let verbs = match verb {
            "sing" => "sings",
            "whistle" => "whistles",
            _ => "hums",
        };
        self.say1(id, Line::new().c(Col::Dim, format!("You {verb} ")).c(Col::Pink, &tune));
        self.tell_here_from(x, y, id, &name, Line::new().b(Col::Cyan, &name).c(Col::Dim, format!(" {verbs} ")).c(Col::Pink, &tune));
        for (&pid, o) in &self.players {
            let d = (o.x - x).abs().max((o.y - y).abs());
            if pid != id && (1..=2).contains(&d) && o.hears(&name) {
                let _ = o.tx.send(ToClient::Lines {
                    lines: vec![Line::col(Col::Pink, format!("♪ You hear someone {verbs} nearby."))],
                });
            }
        }
        Ok(())
    }

    // ---- first steps --------------------------------------------------

    fn snapshot(&self, id: u64) -> Option<(i64, i64, Terrain, usize)> {
        let p = self.players.get(&id)?;
        Some((p.x, p.y, self.world.terrain(p.x, p.y), self.world.things.len()))
    }

    /// Work out what a command actually did, by comparing before and after.
    fn what_happened(&self, id: u64, input: &str, before: Option<(i64, i64, Terrain, usize)>) -> Option<Act> {
        let (bx, by, bt, bn) = before?;
        let (ax, ay, at, an) = self.snapshot(id)?;
        let verb = input.split_whitespace().next().unwrap_or("").to_lowercase();
        if (ax, ay) != (bx, by) {
            Some(Act::Walk)
        } else if an > bn {
            Some(Act::Make)
        } else if verb == "plant" && (at != bt || matches!(at, Terrain::Meadow | Terrain::Forest | Terrain::Grass)) {
            Some(Act::Plant)
        } else if verb == "who" {
            Some(Act::Who)
        } else if said_something(input, &verb) {
            Some(Act::Say)
        } else {
            None
        }
    }

    /// Save a player's position and first-steps progress straight away, so
    /// nothing is lost even if the world stops before they leave.
    fn save_progress(&self, id: u64) {
        let Some(p) = self.players.get(&id) else { return };
        if !p.bot {
            if let Err(e) = self.world.db.save_player(&p.name, p.x, p.y, p.tutorial) {
                eprintln!("saving {}: {e}", p.name);
            }
        }
    }

    fn progress(&mut self, id: u64, act: Act) {
        let Some(p) = self.players.get_mut(&id) else { return };
        let Some(step) = tutorial::step(p.tutorial) else { return };
        if step.act != act {
            return;
        }
        p.tutorial = if (p.tutorial as usize) + 1 >= tutorial::STEPS.len() { tutorial::DONE } else { p.tutorial + 1 };
        self.say1(id, Line::new().b(Col::Yellow, step.cheer));
        self.show_hint(id);
        self.save_progress(id);
    }

    fn show_hint(&self, id: u64) {
        let Some(p) = self.players.get(&id) else { return };
        let msg = match tutorial::step(p.tutorial) {
            Some(step) => {
                self.say1(id, Line::new().c(Col::Yellow, "→ ").b(Col::Yellow, step.hint));
                ToClient::Hint { text: Some(step.hint.into()), cmd: Some(step.cmd.into()) }
            }
            None => ToClient::Hint { text: None, cmd: None },
        };
        let _ = p.tx.send(msg);
    }

    fn find_player(&self, name: &str) -> Option<u64> {
        let n = name.trim().trim_end_matches([',', ':']).to_lowercase();
        if n.is_empty() {
            return None;
        }
        let exact = self.players.iter().find(|(_, p)| p.name.to_lowercase() == n);
        let prefix = || self.players.iter().find(|(_, p)| p.name.to_lowercase().starts_with(&n));
        exact.or_else(prefix).map(|(&pid, _)| pid)
    }

    fn cmd_tell(&mut self, id: u64, to: &str, msg: &str) -> anyhow::Result<()> {
        if to.is_empty() || msg.is_empty() {
            self.say1(id, Line::plain("Tell who what? e.g. '@Poppy want to build a boat?'"));
            return Ok(());
        }
        let Some(tid) = self.find_player(to) else {
            self.say1(id, Line::plain(format!("Nobody called {to} is in the world right now. Try 'who'.")));
            return Ok(());
        };
        if tid == id {
            self.say1(id, Line::plain("You mutter to yourself. Very wise."));
            return Ok(());
        }
        let from = self.players[&id].name.clone();
        let target = self.players[&tid].name.clone();
        self.say1(id, Line::new().c(Col::Pink, format!("✉ You say to {target}: ")).c(Col::White, msg));
        if !self.players[&tid].hears(&from) {
            return Ok(()); // muted: quietly not delivered
        }
        self.say1(tid, Line::new().b(Col::Pink, format!("✉ {from}")).c(Col::Pink, " says to you: ").c(Col::White, msg));
        if let Some(t) = self.players.get_mut(&tid) {
            t.reply_to = Some(from);
        }
        if let Some(me) = self.players.get_mut(&id) {
            me.reply_to.get_or_insert(target);
        }
        Ok(())
    }

    fn cmd_chat(&mut self, id: u64, msg: &str) -> anyhow::Result<()> {
        if msg.is_empty() {
            self.say1(id, Line::plain("Chat what? Everyone in the world will hear it."));
            return Ok(());
        }
        let from = self.players[&id].name.clone();
        let line = Line::new().c(Col::Green, "💬 ").b(Col::Green, &from).c(Col::Green, ": ").c(Col::White, msg);
        for p in self.players.values().filter(|p| p.hears(&from)) {
            let _ = p.tx.send(ToClient::Lines { lines: vec![line.clone()] });
        }
        Ok(())
    }

    fn cmd_find(&self, id: u64, name: &str) {
        let Some(tid) = self.find_player(name) else {
            self.say1(id, Line::plain(format!("You can't find anyone called '{name}'. Try 'who'.")));
            return;
        };
        let (me, them) = (&self.players[&id], &self.players[&tid]);
        let place = self.world.place_title(them.x, them.y);
        let (dx, dy) = (them.x - me.x, them.y - me.y);
        let d = dx.abs().max(dy.abs());
        let line = if tid == id {
            "You're right here, silly.".to_string()
        } else if d == 0 {
            format!("{} is right here with you!", them.name)
        } else {
            let dir = DIRS.iter().find(|q| q.2 == dx.signum() && q.3 == dy.signum()).map_or("away", |q| q.1);
            let steps = if d == 1 { "1 step".to_string() } else { format!("{d} steps") };
            format!("{} is at {place}, about {steps} to the {dir}.", them.name)
        };
        self.say1(id, Line::col(Col::Cyan, format!("🧭 {line}")));
    }

    fn cmd_who(&self, id: u64) {
        let mut lines = vec![Line::new().b(Col::Magenta, "Out in the world:")];
        for p in self.players.values() {
            lines.push(
                Line::new()
                    .c(Col::Cyan, if p.bot { "  🐾 " } else { "  ☺ " })
                    .b(Col::Cyan, &p.name)
                    .c(Col::Dim, format!("  ({})", self.world.place_title(p.x, p.y)))
                    .c(Col::Dim, if p.bot { "  island resident" } else { "" }),
            );
        }
        lines.push(Line::col(Col::Dim, "Talk to anyone, anywhere: @Name hello"));
        self.send(id, lines);
    }

    fn cmd_examine(&self, id: u64, what: &str) {
        let Some(p) = self.players.get(&id) else { return };
        match self.world.find_thing(p.x, p.y, what) {
            Some(i) => {
                let t = &self.world.things[i];
                let mut lines = vec![Line::new().b(Col::Yellow, capitalise(&t.name))];
                let when = match t.trigger {
                    Trigger::Always => "now and then",
                    Trigger::Rain => "when it rains",
                    Trigger::Sun => "when the sun is out",
                    Trigger::Night => "at night",
                    Trigger::Day => "during the day",
                    Trigger::Arrive => "when someone arrives",
                };
                if let Some(a) = &t.action {
                    lines.push(Line::plain(format!("It {a} {when}.")));
                }
                lines.push(Line::col(Col::Dim, format!("Made by {}.", t.maker)));
                self.send(id, lines);
            }
            None => self.say1(id, Line::plain(format!("You don't see any {what} here."))),
        }
    }

    // ---- shaping the land --------------------------------------------

    fn here(&self, id: u64) -> Option<(i64, i64, String)> {
        self.players.get(&id).map(|p| (p.x, p.y, p.name.clone()))
    }

    fn reshape(&mut self, id: u64, to: Terrain, mine: &str, theirs: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        self.world.set_terrain(x, y, to)?;
        self.say1(id, Line::col(Col::Green, mine));
        self.tell_here(x, y, id, Line::col(Col::Green, format!("{name} {theirs}")));
        self.status_all();
        Ok(())
    }

    fn near_water(&self, x: i64, y: i64) -> bool {
        DIRS.iter().any(|d| self.world.terrain(x + d.2, y + d.3).is_wet())
    }

    fn cmd_dig(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        let raining = self.world.weather.sky_at(x, y).wet();
        use Terrain::*;
        match self.world.terrain(x, y) {
            Dirt | Sand if raining || self.near_water(x, y) => self.reshape(
                id,
                Pond,
                "You dig deeper. Water seeps in and slowly fills the hollow. A pond! 💧",
                "digs, and water wells up into a little pond.",
            ),
            Dirt => self.reshape(
                id,
                Pond,
                "You dig deeper. Cool water trickles up from below and fills the hollow. A pond! 💧",
                "digs out a little pond.",
            ),
            Grass | Meadow | Path | Forest => self.reshape(
                id,
                Dirt,
                "You dig, turning over the ground into a patch of dark, crumbly earth.",
                "digs up a patch of earth.",
            ),
            Sand => {
                self.say1(id, Line::plain("You scoop at the sand, but it just slides back in. Too far from water to make a pool."));
                Ok(())
            }
            Hill => self.reshape(id, Grass, "You dig away at the hill until it's level ground.", "levels the hill with a lot of digging."),
            Stone => {
                self.say1(id, Line::plain("The rock here is far too hard to dig."));
                Ok(())
            }
            Water | Pond | DeepWater => {
                self.say1(id, Line::col(Col::Blue, "You splash about. It's very satisfying, but you're not digging anything."));
                Ok(())
            }
        }
    }

    fn cmd_fill(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        match self.world.terrain(x, y) {
            Terrain::Pond => self.reshape(id, Terrain::Dirt, "You fill the pond back in with earth.", "fills in the pond."),
            Terrain::Water => self.reshape(id, Terrain::Sand, "You heap up sand until it rises above the water.", "builds up a little sandbank."),
            Terrain::Dirt => self.reshape(id, Terrain::Grass, "You pat the earth flat. Grass will soon cover it.", "pats the earth flat."),
            _ => {
                self.say1(id, Line::plain("There's nothing here to fill."));
                Ok(())
            }
        }
    }

    fn cmd_plant(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        let here = self.world.terrain(x, y);
        use Terrain::*;
        if matches!(here, Water | Pond | DeepWater | Stone) {
            self.say1(id, Line::plain("Nothing would grow here."));
            return Ok(());
        }
        let w = strip_article(&what.to_lowercase()).to_string();
        if w.contains("tree") || w.contains("forest") || w.contains("wood") || w.contains("seed") {
            self.reshape(id, Forest, "You plant a sapling. It grows, impossibly fast, into a tall tree. 🌳", "plants a tree, which shoots up overnight-fast.")
        } else if w.contains("flower") || w.contains("bulb") || w.contains("meadow") {
            self.reshape(id, Meadow, "You scatter seeds and flowers bloom around your feet. 🌼", "scatters seeds; wildflowers bloom everywhere.")
        } else if w.contains("grass") || w.is_empty() {
            self.reshape(id, Grass, "You sow grass seed. A soft green carpet spreads out. 🌱", "sows grass, and green spreads around.")
        } else {
            self.say1(id, Line::plain("You can plant a tree, flowers or grass."));
            Ok(())
        }
    }

    fn cmd_raise(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        match self.world.terrain(x, y) {
            Terrain::Hill => self.reshape(id, Terrain::Stone, "You heap up stones until it's a rocky height. ⛰", "piles up a rocky height."),
            Terrain::Stone => {
                self.say1(id, Line::plain("It's as high as it's going to get."));
                Ok(())
            }
            t if t.is_wet() => {
                self.say1(id, Line::plain("You'd need to fill the water in first."));
                Ok(())
            }
            _ => self.reshape(id, Terrain::Hill, "You heap the earth up into a gentle hill.", "heaps the ground up into a hill."),
        }
    }

    fn cmd_path(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        if self.world.terrain(x, y).is_wet() {
            self.say1(id, Line::plain("Paths don't float, sweetie."));
            return Ok(());
        }
        self.reshape(id, Terrain::Path, "You lay down pebbles and stamp out a little path.", "lays down a little path.")
    }

    fn cmd_name(&mut self, id: u64, name: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let name = name.strip_prefix("here ").unwrap_or(name).trim();
        if name.is_empty() {
            self.say1(id, Line::plain("Name this place what? e.g. 'name The Willow Bank'"));
            return Ok(());
        }
        let name: String = name.chars().take(40).collect();
        self.world.set_name(x, y, Some(name.clone()))?;
        self.say1(id, Line::new().c(Col::Green, "This place will be known as ").b(Col::Magenta, &name).c(Col::Green, "."));
        self.tell_here(x, y, id, Line::col(Col::Green, format!("{who} names this place {name}.")));
        self.status_all();
        Ok(())
    }

    fn cmd_make(&mut self, id: u64, text: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        if text.is_empty() {
            self.send(
                id,
                vec![
                    Line::plain("Make what? Some ideas:"),
                    Line::col(Col::Yellow, "  make a little blue bench"),
                    Line::col(Col::Yellow, "  make a frog that croaks when it rains"),
                    Line::col(Col::Yellow, "  make a lantern that glows softly at night"),
                    Line::col(Col::Yellow, "  make a wind chime that tinkles"),
                    Line::col(Col::Yellow, "  make a doorbell that rings when someone arrives"),
                ],
            );
            return Ok(());
        }
        if self.world.things_at(x, y).count() >= 12 {
            self.say1(id, Line::plain("It's getting crowded here. Make something somewhere else?"));
            return Ok(());
        }
        let (name, action, trigger) = parse_make(text);
        if boards::is_board_name(&name) {
            if self.world.things_at(x, y).any(|t| t.is_board()) {
                self.say1(id, Line::plain("There's already a board here. Try 'post hello!'"));
                return Ok(());
            }
            self.world.add_thing(x, y, name.clone(), None, Trigger::Always, &who, Some(boards::board_kind()))?;
            self.say1(id, Line::new().c(Col::Yellow, "📌 You put up ").b(Col::Yellow, &name).c(Col::Yellow, ". Anyone passing can read it and post notes."));
            self.say1(id, Line::col(Col::Dim, "Try: post hello everyone! · read · follow"));
            self.tell_here(x, y, id, Line::col(Col::Yellow, format!("📌 {who} puts up {name}.")));
            self.status_all();
            return Ok(());
        }
        self.world.add_thing(x, y, name.clone(), action, trigger, &who, None)?;
        self.say1(id, Line::new().c(Col::Yellow, "✨ You make ").b(Col::Yellow, &name).c(Col::Yellow, "."));
        self.tell_here(x, y, id, Line::col(Col::Yellow, format!("✨ {who} makes {name}.")));
        self.status_all();
        Ok(())
    }

    fn is_keeper(&self, name: &str) -> bool {
        self.keepers.iter().any(|k| k.trim().eq_ignore_ascii_case(name))
    }

    /// Only whoever made something (or an island keeper) may unmake or push it.
    fn may_change(&self, who: &str, i: usize) -> bool {
        self.world.things[i].maker.eq_ignore_ascii_case(who) || self.is_keeper(who)
    }

    fn not_yours(&self, i: usize) -> Line {
        let t = &self.world.things[i];
        let what = crate::server::world::strip_article(&t.name).to_string();
        if self.players.values().any(|p| p.name == t.maker) || self.world.db.player(&t.maker).is_some() {
            Line::plain(format!("That's {}'s {what}. Only {} can unmake or move it. (Ask them: @{} ...)", t.maker, t.maker, t.maker))
        } else {
            Line::plain(format!("That {what} belongs to {}. It stays as it is.", t.maker))
        }
    }

    /// Remind online keepers about devices waiting to be let in.
    fn tell_keepers_about_knocks(&self) {
        let Ok(knocks) = self.world.db.knocks() else { return };
        if knocks.is_empty() {
            return;
        }
        let mut lines = Vec::new();
        for (name, devices, ago) in knocks {
            let when = if ago < 60 { "just now".to_string() } else if ago < 3600 { format!("{}m ago", ago / 60) } else { format!("{}h ago", ago / 3600) };
            let who = if devices == 1 { "Someone wants".to_string() } else { format!("{devices} devices want") };
            lines.push(
                Line::new()
                    .c(Col::Yellow, format!("🔐 {who} to come in as {name} ({when}). Ask {name} for the code on their screen, then type: "))
                    .b(Col::Yellow, format!("welcome {name} <code>")),
            );
        }
        for p in self.players.values().filter(|p| self.is_keeper(&p.name)) {
            let _ = p.tx.send(ToClient::Lines { lines: lines.clone() });
        }
    }

    fn cmd_welcome(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((_, _, who)) = self.here(id) else { return Ok(()) };
        if !self.is_keeper(&who) {
            self.say1(id, Line::plain("Only island keepers can let a new device in."));
            return Ok(());
        }
        let mut words = rest.split_whitespace();
        let (Some(name), Some(code)) = (words.next(), words.next()) else {
            self.say1(id, Line::plain("Ask them for the code on their screen, then: welcome Poppy FROG-42"));
            return Ok(());
        };
        match self.world.db.welcome(name, code)? {
            Some(n) => self.say1(id, Line::col(Col::Green, format!("✅ {n}'s new device can come in now. (Any other device for {n} won't work any more.)"))),
            None => self.say1(id, Line::plain(format!("That code doesn't match anyone knocking as {name}. Check the code on their screen?"))),
        }
        Ok(())
    }

    fn cmd_mute(&mut self, id: u64, name: &str, on: bool) -> anyhow::Result<()> {
        let Some((_, _, who)) = self.here(id) else { return Ok(()) };
        let name = name.trim();
        if name.is_empty() {
            self.say1(id, Line::plain(if on { "Mute who? e.g. 'mute Sam'" } else { "Unmute who?" }));
            return Ok(());
        }
        // Use the name as the world knows it, online or not.
        let known = self
            .find_player(name)
            .map(|pid| self.players[&pid].name.clone())
            .or_else(|| self.world.db.player(&clean_name(name)).map(|_| clean_name(name)));
        let Some(target) = known else {
            self.say1(id, Line::plain(format!("There's nobody called {name} on the island.")));
            return Ok(());
        };
        if target.eq_ignore_ascii_case(&who) {
            self.say1(id, Line::plain("You can't mute yourself, silly."));
            return Ok(());
        }
        self.world.db.set_mute(&who, &target, on)?;
        if let Some(p) = self.players.get_mut(&id) {
            if on {
                p.muted.insert(target.to_lowercase());
            } else {
                p.muted.remove(&target.to_lowercase());
            }
        }
        let msg = if on {
            format!("🔇 You won't hear from {target} any more. They aren't told. (unmute {target} to undo)")
        } else {
            format!("🔈 You'll hear from {target} again.")
        };
        self.say1(id, Line::col(Col::Dim, msg));
        Ok(())
    }

    fn cmd_mutes(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((_, _, who)) = self.here(id) else { return Ok(()) };
        let mutes = self.world.db.mutes(&who)?;
        let msg = if mutes.is_empty() {
            "You haven't muted anyone.".to_string()
        } else {
            format!("🔇 Muted: {}", mutes.join(", "))
        };
        self.say1(id, Line::col(Col::Dim, msg));
        Ok(())
    }

    fn cmd_kick(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((_, _, who)) = self.here(id) else { return Ok(()) };
        if !self.is_keeper(&who) {
            self.say1(id, Line::plain("Only island keepers can ask someone to take a break. You can 'mute' them though."));
            return Ok(());
        }
        let mut words = rest.split_whitespace();
        let Some(name) = words.next() else {
            self.say1(id, Line::plain("Kick who? e.g. 'kick Sam' or 'kick Sam 60' (minutes)"));
            return Ok(());
        };
        let minutes: i64 = words.next().and_then(|m| m.parse().ok()).unwrap_or(30).clamp(1, 7 * 24 * 60);
        let target = self.find_player(name);
        let (tname, thash) = match target {
            Some(pid) => (self.players[&pid].name.clone(), self.players[&pid].key_hash.clone()),
            None => (clean_name(name), None),
        };
        if self.is_keeper(&tname) {
            self.say1(id, Line::plain("Keepers can't kick other keepers."));
            return Ok(());
        }
        self.world.db.add_ban(&tname, thash.as_deref(), boards::now() + minutes * 60)?;
        if let Some(pid) = target {
            if let Some(p) = self.players.get(&pid) {
                let _ = p.tx.send(ToClient::Bye {
                    reason: format!("🌙 A keeper has asked you to take a break. You can come back in about {minutes} minutes."),
                });
            }
        }
        let gone = if target.is_some() { "has been sent off" } else { "isn't here, but" };
        self.say1(id, Line::col(Col::Yellow, format!("👢 {tname} {gone} for {minutes} minutes (name and device). 'unkick {tname}' to let them back early.")));
        println!("👢 {who} kicked {tname} for {minutes} minutes");
        Ok(())
    }

    fn cmd_unkick(&mut self, id: u64, name: &str) -> anyhow::Result<()> {
        let Some((_, _, who)) = self.here(id) else { return Ok(()) };
        if !self.is_keeper(&who) {
            self.say1(id, Line::plain("Only island keepers can do that."));
            return Ok(());
        }
        let msg = if self.world.db.lift_ban(name.trim())? {
            format!("🌤 {} can come back whenever they like.", name.trim())
        } else {
            format!("{} wasn't on a break.", name.trim())
        };
        self.say1(id, Line::col(Col::Green, msg));
        Ok(())
    }

    fn cmd_unlink(&mut self, id: u64, name: &str) -> anyhow::Result<()> {
        let Some((_, _, who)) = self.here(id) else { return Ok(()) };
        if !self.is_keeper(&who) {
            self.say1(id, Line::plain("Only island keepers can release a name."));
            return Ok(());
        }
        let msg = if self.world.db.release_name(name.trim())? {
            format!("🔓 {} is free: the next device to use that name will claim it.", name.trim())
        } else {
            format!("Nobody had claimed '{}', so there was nothing to release.", name.trim())
        };
        self.say1(id, Line::col(Col::Green, msg));
        Ok(())
    }

    fn cmd_undo(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((_, _, who)) = self.here(id) else { return Ok(()) };
        let restored = self.world.undo_remove(&who)?.map(|t| (t.x, t.y, t.name.clone()));
        match restored {
            Some((x, y, name)) => {
                let place = self.world.place_title(x, y);
                self.say1(id, Line::col(Col::Yellow, format!("✨ {} is back at {place}.", capitalise(&name))));
                self.tell_here(x, y, id, Line::col(Col::Yellow, format!("✨ {} reappears, just where it was.", capitalise(&name))));
                self.status_all();
            }
            None => self.say1(id, Line::plain("There's nothing of yours to bring back from the last day.")),
        }
        Ok(())
    }

    fn cmd_unmake(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        match self.world.find_thing(x, y, what) {
            Some(i) if self.world.things[i].kind.as_deref() == Some(boards::journal_kind()) => {
                self.say1(id, Line::plain("The Island Journal belongs to everyone. It stays right where it is."));
            }
            Some(i) if !what.is_empty() && !self.may_change(&who, i) => {
                self.say1(id, self.not_yours(i));
            }
            Some(i) if !what.is_empty() => {
                let t = self.world.remove_thing(i, &who)?;
                self.say1(id, Line::col(Col::Dim, format!("{} fades gently away.", capitalise(&the(&t.name)))));
                self.say1(id, Line::col(Col::Dim, "(changed your mind? type undo)"));
                self.tell_here(x, y, id, Line::col(Col::Dim, format!("{who} unmakes {}.", t.name)));
                self.status_all();
            }
            _ => self.say1(id, Line::plain("Unmake what? Name something that's here.")),
        }
        Ok(())
    }

    fn cmd_push(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let mut words: Vec<&str> = rest.split_whitespace().collect();
        let dir = words.pop().and_then(|w| DIRS.iter().find(|d| d.0 == w || d.1 == w));
        let what = words.join(" ");
        let (Some(&(_, long, dx, dy)), Some(i)) = (dir, self.world.find_thing(x, y, &what)) else {
            self.say1(id, Line::plain("Try something like 'push bench north'."));
            return Ok(());
        };
        if !self.world.terrain(x + dx, y + dy).walkable() {
            self.say1(id, Line::plain("It would sink without a trace."));
            return Ok(());
        }
        if self.world.things[i].kind.as_deref() == Some(boards::journal_kind()) {
            self.say1(id, Line::plain("The Island Journal won't budge. It belongs right here."));
            return Ok(());
        }
        if !self.may_change(&who, i) {
            self.say1(id, self.not_yours(i));
            return Ok(());
        }
        let name = self.world.things[i].name.clone();
        self.world.move_thing(i, x + dx, y + dy)?;
        self.say1(id, Line::plain(format!("You push {} off to the {long}.", the(&name))));
        self.tell_here(x, y, id, Line::plain(format!("{who} pushes {} off to the {long}.", the(&name))));
        self.tell_here(x + dx, y + dy, id, Line::plain(format!("{} arrives, pushed by {who}.", capitalise(&the(&name)))));
        self.status_all();
        Ok(())
    }

    /// Talking near something alive sometimes gets a response. Mostly it's
    /// a hook for later; for now, birds.
    fn react_to_words(&mut self, x: i64, y: i64) {
        if self.world.terrain(x, y) == Terrain::Forest && self.rng.chance(0.15) {
            self.tell_here(x, y, 0, Line::col(Col::Dim, "A bird in the branches answers with a trill."));
        }
    }

    // ---- the world turning -------------------------------------------

    fn tick(&mut self) {
        self.world.weather.step();

        let phase = Clock::phase();
        if phase != self.phase {
            self.phase = phase;
            let msg = match phase {
                Phase::Dawn => "🌅 The sky begins to lighten in the east. Dawn is coming.",
                Phase::Day => "☀ The sun climbs up over the sea. Good morning, world.",
                Phase::Dusk => "🌇 The sun is sinking low. Shadows stretch long across the island.",
                Phase::Night => "🌙 Night falls. One by one, the stars come out.",
            };
            for p in self.players.values() {
                let _ = p.tx.send(ToClient::Lines { lines: vec![Line::col(Col::Magenta, msg)] });
            }
        }

        let ids: Vec<u64> = self.players.keys().copied().collect();
        for id in ids {
            let Some(p) = self.players.get(&id) else { continue };
            let (x, y) = (p.x, p.y);
            let sky = self.world.weather.sky_at(x, y);
            if sky != p.sky {
                if let Some(msg) = sky_change(p.sky, sky) {
                    self.say1(id, Line::col(Col::Blue, msg));
                }
                if let Some(p) = self.players.get_mut(&id) {
                    p.sky = sky;
                }
            } else if self.rng.chance(1.0 / 14.0) {
                let terrain = self.world.terrain(x, y);
                if let Some(line) = ambient(&mut self.rng, terrain, sky, phase) {
                    self.say1(id, Line::col(Col::Dim, line));
                }
            }
        }

        self.nature_tick();

        // Things doing their things, but only when someone's there to see.
        let mut acts: Vec<(i64, i64, String)> = Vec::new();
        for t in &self.world.things {
            let Some(action) = &t.action else { continue };
            if !self.players.values().any(|p| p.x == t.x && p.y == t.y) {
                continue;
            }
            let sky = self.world.weather.sky_at(t.x, t.y);
            let active = match t.trigger {
                Trigger::Always => true,
                Trigger::Rain => sky.wet(),
                Trigger::Sun => sky == Sky::Clear && phase != Phase::Night,
                Trigger::Night => phase == Phase::Night,
                Trigger::Day => phase == Phase::Day,
                Trigger::Arrive => false,
            };
            if active && self.rng.chance(0.07) {
                acts.push((t.x, t.y, sentence(&format!("{} {action}", the(&t.name)))));
            }
        }
        for (x, y, text) in acts {
            self.tell_here(x, y, 0, Line::col(Col::Yellow, text));
        }

        self.status_all();
    }
}

/// `say hi`, `'hi` or `"hi`, with something actually said.
fn said_something(input: &str, verb: &str) -> bool {
    let text = if verb == "say" {
        input.split_once(char::is_whitespace).map_or("", |(_, r)| r)
    } else if let Some(r) = input.strip_prefix(SAY_QUOTES) {
        r
    } else {
        ""
    };
    !text.trim().is_empty()
}

fn sky_change(from: Sky, to: Sky) -> Option<&'static str> {
    Some(match (from, to) {
        (_, Sky::Rain) if from != Sky::Drizzle => "Fat drops start to fall, and then it's properly raining. ☔",
        (Sky::Drizzle, Sky::Rain) => "The drizzle thickens into real rain. ☔",
        (_, Sky::Drizzle) if from == Sky::Rain => "The rain eases to a soft drizzle.",
        (_, Sky::Drizzle) => "A fine drizzle begins to fall. ☂",
        (Sky::Rain | Sky::Drizzle, Sky::Cloudy) => "The rain stops. Everything drips and glistens.",
        (Sky::Clear, Sky::Cloudy) => "Clouds drift over and the light goes soft.",
        (_, Sky::Clear) => "The clouds part and the sky opens up, clear and wide.",
        _ => return None,
    })
}

fn ambient(rng: &mut Rng, terrain: Terrain, sky: Sky, phase: Phase) -> Option<&'static str> {
    use Terrain::*;
    let night = phase == Phase::Night;
    let options: &[&str] = match (terrain, sky.wet(), night) {
        (_, true, _) => &[
            "Rain ticks softly all around you.",
            "Water drips from somewhere nearby.",
            "The rain comes in a gust, then softens again.",
        ],
        (Forest, _, false) => &[
            "A woodpecker knocks somewhere above.",
            "Leaves rustle as something small scurries away.",
            "A shaft of light breaks through the canopy.",
        ],
        (Forest, _, true) => &["An owl calls softly: hoo, hoo.", "Something rustles in the dark undergrowth."],
        (Meadow, _, false) => &["A bee bumbles past your ear.", "A butterfly settles on a poppy, then lifts away."],
        (Meadow | Grass, _, true) => &["Crickets sing in the grass.", "A firefly blinks on, then off."],
        (Grass, _, false) => &["The grass ripples like water in the wind.", "A skylark sings high above."],
        (Sand | Water | DeepWater, _, _) => &[
            "A wave rolls in and hushes back out.",
            "A gull cries, far out over the water.",
            "The sea sighs against the shore.",
        ],
        (Pond, _, _) => &["A frog plops into the water.", "A dragonfly hovers over the pond."],
        (Hill | Stone, _, false) => &["The wind hums over the rocks.", "A hawk circles lazily high above."],
        (Hill | Stone, _, true) => &["The whole sky is wheeling with stars up here."],
        _ => return None,
    };
    Some(*rng.pick(options))
}

/// "make a frog that croaks when it rains" → ("a frog", "croaks", Rain)
fn parse_make(text: &str) -> (String, Option<String>, Trigger) {
    let mut s = text.trim().trim_end_matches(['.', '!']).to_string();
    let triggers: &[(&str, Trigger)] = &[
        (" when it rains", Trigger::Rain),
        (" when it's raining", Trigger::Rain),
        (" in the rain", Trigger::Rain),
        (" when it's sunny", Trigger::Sun),
        (" when the sun shines", Trigger::Sun),
        (" in the sun", Trigger::Sun),
        (" at night", Trigger::Night),
        (" in the dark", Trigger::Night),
        (" during the day", Trigger::Day),
        (" by day", Trigger::Day),
        (" when someone arrives", Trigger::Arrive),
        (" when someone comes", Trigger::Arrive),
        (" when people arrive", Trigger::Arrive),
    ];
    let mut trigger = Trigger::Always;
    let lower = s.to_lowercase();
    for (suffix, t) in triggers {
        if lower.ends_with(suffix) {
            s.truncate(s.len() - suffix.len());
            trigger = *t;
            break;
        }
    }
    let lower = s.to_lowercase();
    let split = [" that ", " which ", " who "].iter().filter_map(|w| lower.find(w).map(|i| (i, w.len()))).min();
    let (name, action) = match split {
        Some((i, len)) => (s[..i].trim().to_string(), Some(s[i + len..].trim().to_string())),
        None => (s.trim().to_string(), None),
    };
    let name: String = name.chars().take(60).collect();
    let lname = name.to_lowercase();
    let has_article = ["a ", "an ", "the ", "some ", "my ", "two ", "three "]
        .iter()
        .any(|a| lname.starts_with(a))
        || name.chars().next().is_some_and(|c| c.is_uppercase());
    let name = if has_article {
        name
    } else if lname.starts_with(['a', 'e', 'i', 'o', 'u']) {
        format!("an {name}")
    } else {
        format!("a {name}")
    };
    let action = action.filter(|a| !a.is_empty()).map(|a| a.chars().take(80).collect());
    (name, action, trigger)
}

/// "a frog" → "the frog"
fn the(name: &str) -> String {
    for a in ["a ", "an ", "some "] {
        if let Some(rest) = name.strip_prefix(a) {
            return format!("the {rest}");
        }
    }
    name.to_string()
}

fn sentence(s: &str) -> String {
    let mut s = capitalise(s);
    if !s.ends_with(['.', '!', '?']) {
        s.push('.');
    }
    s
}

fn clean_name(raw: &str) -> String {
    let name: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(16)
        .collect();
    capitalise(&name)
}

fn help() -> Vec<Line> {
    // No column padding, so it reads fine on a narrow phone screen too.
    let h = |cmd: &str, what: &str| {
        let line = Line::new().c(Col::Yellow, format!("  {cmd}"));
        if what.is_empty() { line } else { line.c(Col::Dim, format!("  {what}")) }
    };
    vec![
        Line::new().b(Col::Magenta, "✨ Things you can do ✨"),
        Line::col(Col::Pink, "wander"),
        h("n s e w ne nw se sw", "walk about"),
        h("look / look <thing>", "take it all in"),
        h("tutorial / skip", "first steps again, or hide them"),
        h("who", "who's out in the world"),
        h("map", "the whole island, with friends and named places"),
        Line::col(Col::Pink, "chat"),
        h("say hi  (or 'hi)", "talk to people here"),
        h("emote waves  (or :waves)", "do a little something"),
        h("@Poppy hi  (or tell)", "talk to someone anywhere"),
        h("reply thanks!  (or r)", "answer whoever told you"),
        h("chat hi everyone  (@all)", "everyone in the world hears"),
        h("find Poppy", "which way to walk to them"),
        h("shout hello", "carries a long way"),
        h("hum C E G", "a little tune"),
        h("sit / rest / dance / wave", "vibes"),
        h("mute Sam / unmute Sam / mutes", "stop hearing from someone (they aren't told)"),
        Line::col(Col::Pink, "shape the land"),
        h("dig / fill", "ponds, earth, sandbanks"),
        h("plant tree|flowers|grass", "make things grow"),
        h("raise / path", "hills, rocks, paths"),
        h("name The Willow Bank", "name this place"),
        Line::col(Col::Pink, "gather"),
        h("gather  (or pick, catch)", "take what's here: look for pink • on the map"),
        h("fish", "by the water, then wait for a bite"),
        h("satchel / journal", "what you carry / everything you've found"),
        h("give daisy to Poppy", "share with someone here"),
        h("place daisy / release moth", "decorate, or set something free"),
        Line::col(Col::Pink, "crafting"),
        h("recipes", "what you can make from what you've gathered"),
        h("craft wind chime", "make it (then place it, or give it away)"),
        Line::col(Col::Pink, "home"),
        h("claim", "make the patch around you your home (one each)"),
        h("home", "go straight home from anywhere"),
        h("invite Lu / uninvite Lu / guests", "who else may build at your home"),
        Line::col(Col::Pink, "noticeboards"),
        h("make a noticeboard", "put one up for everyone"),
        h("read / read more", "the notes here, then older ones"),
        h("post see you at sunset!", "leave a note (and follow the board)"),
        h("boards / follow / unfollow", "boards you hear about from anywhere"),
        h("archive / read week 2026-11-02", "past pages of the Island Journal"),
        Line::col(Col::Pink, "make things"),
        h("make a frog that croaks when it rains", ""),
        Line::col(Col::Dim, "    also: at night, during the day, when it's sunny, when someone arrives"),
        h("push bench north", "move something"),
        h("unmake bench / undo", "take away something you made, or bring it back"),
        Line::col(Col::Pink, "island keepers"),
        h("welcome Poppy FROG-42", "let Poppy in from a new device (the code is on her screen)"),
        h("unlink Poppy", "free a name for whoever uses it next"),
        h("kick Sam [minutes] / unkick Sam", "send someone off for a break (30 min)"),
        Line::new(),
        Line::col(Col::Dim, format!("The island is {SIZE}×{SIZE}. A whole day passes in 24 minutes.")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phone_quotes_still_say_things() {
        for input in ["'hi", "’hi", "‘hi", "\"hi", "“hi", "say hi"] {
            let verb = input.split_whitespace().next().unwrap().to_lowercase();
            assert!(said_something(input, &verb), "{input}");
        }
        assert!(!said_something("’", "’"));
    }

    #[test]
    fn makes_a_rainy_frog() {
        let (n, a, t) = parse_make("frog that croaks when it rains");
        assert_eq!(n, "a frog");
        assert_eq!(a.as_deref(), Some("croaks"));
        assert_eq!(t, Trigger::Rain);
    }

    #[test]
    fn makes_a_plain_bench() {
        let (n, a, t) = parse_make("a little blue bench");
        assert_eq!(n, "a little blue bench");
        assert_eq!(a, None);
        assert_eq!(t, Trigger::Always);
    }

    #[test]
    fn makes_an_owl() {
        let (n, a, t) = parse_make("owl who hoots at night.");
        assert_eq!(n, "an owl");
        assert_eq!(a.as_deref(), Some("hoots"));
        assert_eq!(t, Trigger::Night);
        assert_eq!(sentence(&format!("{} hoots", the(&n))), "The owl hoots.");
    }
}
