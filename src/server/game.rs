//! The one task that owns the world. Connections send it events; it
//! answers with lines of text and status updates.

mod boards;
pub(super) use boards::backfill_journal;
mod companions;
mod cook;
mod craft;
mod events;
mod games;
mod hills;
mod gather;
mod green;
mod gramophone;
mod growing;
mod homes;
mod inside;
mod jukebox;
mod lighthouse;
mod meetings;
mod music;
mod play;
mod progress;
mod lights;
mod builds;
mod cases;
mod radio;
pub(crate) use radio::clean_station_name;
mod sailing;
mod together;
mod travel;
mod visitors;

use super::nature::Find;
use super::rng::Rng;
use super::terrain::Terrain;
use super::tutorial::{self, Act};
use super::weather::{Clock, Phase, Season, Sky};
use super::world::{DIRS, Trigger, World, capitalise, in_bounds, strip_article};
use super::{ClientTx, Event};
use crate::protocol::{Col, Line, ToClient};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedReceiver;

/// Starting a line with any of these says it out loud.
const SAY_QUOTES: [char; 6] = ['\'', '"', '‘', '’', '“', '”'];
const TICK: Duration = Duration::from_secs(3);
const SHOUT_RANGE: i64 = 10;
/// How long a place's description may be.
const DESCRIBE_MAX: usize = 280;

struct Player {
    /// Nickname: what everyone sees, and it can change.
    name: String,
    /// Username: fixed forever, and what everything of theirs is stored under.
    user: String,
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
    /// Who they're walking to, with `join`.
    travel: Option<String>,
    /// Where they're walking to, with `go event`: (x, y, place name).
    walk_to: Option<(i64, i64, String)>,
    /// Where they're headed in the end, when it's over the water: walk to the
    /// dock, row across, then walk on.
    heading: Option<(i64, i64, String)>,
    /// Who they're off to find once they've rowed across (`join`), by username.
    join_after: Option<String>,
    /// Out on the water in a rowing boat.
    voyage: Option<sailing::Voyage>,
    /// Their timezone, so times read as theirs.
    tz: chrono_tz::Tz,
    /// Whether that's really theirs (their device said, now or before), not
    /// just the world's.
    tz_known: bool,
    /// Their animal friend: (animal id, nickname).
    companion: Option<(String, Option<String>)>,
    /// The last little bit of scenery they were told, so it isn't said twice running.
    last_ambient: Option<&'static str>,
    /// Scenery lines since they last did anything: someone who's stepped away
    /// shouldn't come back to a screenful of birdsong.
    ambient_quiet: u8,
    /// When each thing last did its thing for them, so a frog doesn't croak on and on.
    seen_acts: HashMap<i64, std::time::Instant>,
    /// Kites, seagulls, silly hats.
    fun: play::Fun,
    /// Their side panel is out of date, and goes with the next stride.
    status_due: std::cell::Cell<bool>,
    /// The radio this browser has been told it can hear: (how loud, stream).
    radios: HashMap<i64, (f32, String)>,
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
    season: Season,
    festival: bool,
    rng: Rng,
    /// Flowers, butterflies, shells... waiting to be gathered.
    finds: Vec<Find>,
    /// The Island Journal's thing id.
    journal_board: Option<i64>,
    /// The world's public address, for invite links.
    url: Option<String>,
    /// Visitors from other worlds waiting at the dock to be let in.
    waiting: HashMap<u64, (super::federation::Passport, ClientTx)>,
    /// Animals out in the wild, waiting to be befriended.
    wild: Vec<companions::Wild>,
    /// Who last hummed where, for noticing a jam.
    last_hum: HashMap<(i64, i64), (String, std::time::Instant)>,
    /// Gramophones playing music from someone's browser, by thing id.
    spins: HashMap<i64, gramophone::Spin>,
    /// Radios and the stream each is tuned to, by thing id.
    radios: HashMap<i64, String>,
    /// What's cooking at each campfire and oven, by thing id.
    cooking: HashMap<i64, cook::Cooking>,
    /// Jukeboxes' queues, by thing id.
    jukes: HashMap<i64, jukebox::Jukebox>,
    /// Radios' programme guides, for saying what's on air.
    radio_guides: HashMap<i64, String>,
    /// What each radio's on, as far as the world last looked.
    radio_news: HashMap<i64, radio::News>,
    /// Meeting spots' links, by thing id.
    meeting_links: HashMap<i64, String>,
    /// Timezones connections said they're in, until they're on the island.
    zones: HashMap<u64, chrono_tz::Tz>,
    /// When each hook last posted, so one can't flood its board.
    hook_last: HashMap<i64, std::time::Instant>,
    /// Progress bars showing on boards, by hook.
    bars: std::collections::BTreeMap<i64, progress::Progress>,
    /// Up-or-down lights showing on boards, by hook.
    lights: std::collections::BTreeMap<i64, lights::Status>,
    /// Buildings, by thing.
    builds: std::collections::BTreeMap<i64, builds::Build>,
    /// Sandcastles, until the tide comes in.
    castles: HashMap<(i64, i64), play::Built>,
    /// Snowmen, until winter's over.
    snowmen: HashMap<(i64, i64), play::Built>,
    /// Snow angels, until fresh snow fills them in.
    angels: HashMap<(i64, i64), play::Angels>,
    /// Little games going on, by spot.
    rounds: HashMap<(i64, i64), games::Round>,
    /// When a home's owner was last told someone was visiting, by (owner, visitor).
    doorbells: HashMap<(String, String), std::time::Instant>,
    /// When the tide next comes in.
    next_tide: std::time::Instant,
    /// For work done elsewhere (looking up what's on) to report back.
    inbox: tokio::sync::mpsc::UnboundedSender<Event>,
}

pub async fn run(world: World, mut events: UnboundedReceiver<Event>, inbox: tokio::sync::mpsc::UnboundedSender<Event>, url: Option<String>) {
    let seed = world.seed;
    let mut game = Game { world, players: HashMap::new(), phase: Clock::phase(), season: Clock::season(), festival: Clock::festival().is_some(), rng: Rng::new(seed), finds: Vec::new(), journal_board: None, last_hum: HashMap::new(), wild: Vec::new(), url, waiting: HashMap::new(), spins: HashMap::new(), radios: HashMap::new(), radio_guides: HashMap::new(), radio_news: HashMap::new(), inbox, jukes: HashMap::new(), cooking: HashMap::new(), hook_last: HashMap::new(), bars: std::collections::BTreeMap::new(), lights: std::collections::BTreeMap::new(), builds: std::collections::BTreeMap::new(), meeting_links: HashMap::new(), zones: HashMap::new(), castles: HashMap::new(), snowmen: HashMap::new(), angels: HashMap::new(), rounds: HashMap::new(), doorbells: HashMap::new(), next_tide: std::time::Instant::now() + Duration::from_secs(3 * 3600) };
    game.load_meetings();
    game.load_snowmen();
    game.load_progress();
    game.load_lights();
    game.load_buildings();
    game.load_radios();
    if let Err(e) = game.ensure_journal() {
        eprintln!("couldn't put up the Island Journal: {e}");
    }
    if let Err(e) = game.ensure_green() {
        eprintln!("couldn't lay out the village green: {e}");
    }
    let mut tick = tokio::time::interval(TICK);
    let mut stride = tokio::time::interval(Duration::from_millis(500));
    loop {
        tokio::select! {
            ev = events.recv() => match ev {
                Some(ev) => game.handle(ev),
                None => break,
            },
            _ = tick.tick() => game.tick(),
            _ = stride.tick() => {
                game.step_travellers();
                game.row_boats();
                game.update_radios();
                game.jukes_tick();
                game.cooking_tick();
                game.bars_tick();
                game.lights_tick();
                game.flush_status();
            }
        }
    }
}

impl Game {
    fn handle(&mut self, ev: Event) {
        match ev {
            Event::Join { id, bot, invite, token, back, tz, tx } => {
                self.note_zone(id, tz);
                self.join(id, bot, invite, token, back, tx)
            }
            Event::Visit { id, passport, tz, tx } => {
                self.note_zone(id, tz);
                self.visit(id, passport, tx)
            }
            Event::Leave { id } => {
                self.zones.remove(&id);
                self.leave(id)
            }
            Event::Audio { id, frames, title } => self.audio(id, frames, title),
            Event::RadioNews { thing, title } => self.radio_news(thing, title),
            Event::Shutdown => self.shutdown(),
            Event::Hook { secret, text, reply } => {
                let _ = reply.send(self.hook_post(&secret, &text));
            }
            Event::Queue { id, thing, songs } => self.queue_songs(id, thing, songs),
            Event::TrackDone { id, thing, key } => self.track_done(id, thing, &key),
            Event::Cmd { id, text } => {
                if let Some(p) = self.players.get_mut(&id) {
                    p.ambient_quiet = 0;
                }
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

    /// The server's stopping (usually a redeploy). Everyone's place is already
    /// saved with every step; say goodbye in a way browsers know to wait out,
    /// give the messages a moment to go, then stop.
    fn shutdown(&mut self) {
        println!("🌙 stopping: {} here", self.players.len());
        for p in self.players.values() {
            let _ = p.tx.send(ToClient::Restarting);
        }
        // Wait without blocking: the connections' writers may be queued on
        // this very thread, and they're what carry the goodbye out.
        tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            std::process::exit(0);
        });
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
        p.status_due.set(false);
        let sky = self.world.weather.sky_at(p.x, p.y);
        let people: Vec<(i64, i64)> =
            self.players.iter().filter(|(pid, _)| **pid != id).map(|(_, o)| (o.x, o.y)).collect();
        let msg = ToClient::Status {
            place: format!("{} · {}", self.world.place_title(p.x, p.y), super::islands::island_at(p.x, p.y).name),
            weather: format!(
                "{} {} · wind from the {}",
                sky.icon_in(Clock::phase(), Clock::season()),
                sky.label_in(Clock::season()),
                self.world.weather.wind_label()
            ),
            clock: Clock::label(),
            map: {
                let (mx, my) = self.world.outside(p.x, p.y);
                self.world.minimap(mx, my, &people, &self.find_positions())
            },
            here: self.others_here(id),
            humans: self.players.values().filter(|p| !p.bot).count() as u32,
            people: self.people_for(id),
            progress: [self.build_bars_for(id), self.bars_for(id)].concat(),
            lights: self.lights_for(id),
        };
        let _ = p.tx.send(msg);
    }

    /// Everyone else in the world, for the side panel: who's right here
    /// first, then people, then residents, each by name.
    fn people_for(&self, id: u64) -> Vec<crate::protocol::Person> {
        let Some(me) = self.players.get(&id) else { return vec![] };
        let mut people: Vec<crate::protocol::Person> = self
            .players
            .iter()
            .filter(|(pid, _)| **pid != id)
            .map(|(_, p)| crate::protocol::Person {
                name: p.name.clone(),
                here: (p.x, p.y) == (me.x, me.y),
                bot: p.bot,
                tz: (p.tz_known && !p.bot).then(|| p.tz.name().to_string()),
            })
            .collect();
        people.sort_by_key(|p| (!p.here, p.bot, p.name.to_lowercase()));
        people
    }

    fn status_all(&self) {
        for &id in self.players.keys() {
            self.status(id);
        }
    }

    /// After `id` changes these spots: their own side panel now, and a fresh
    /// one with the next stride for everyone else whose minimap shows them.
    /// A step only changes the maps that can see it, and a crowd's steps
    /// share one update each half second.
    pub(super) fn status_near(&self, id: u64, spots: &[(i64, i64)]) {
        let (rw, rh) = super::world::MINIMAP_REACH;
        for (&pid, p) in &self.players {
            if pid != id && spots.iter().any(|&(x, y)| (x - p.x).abs() <= rw && (y - p.y).abs() <= rh) {
                p.status_due.set(true);
            }
        }
        self.status(id);
    }

    /// Send the side panels that are due.
    fn flush_status(&self) {
        for (&id, p) in &self.players {
            if p.status_due.get() {
                self.status(id);
            }
        }
    }

    fn look(&self, id: u64) {
        let Some(p) = self.players.get(&id) else { return };
        let mut lines = self.world.describe(p.x, p.y, &self.others_here(id));
        let exits = lines.pop();
        lines.extend(self.board_lines_here(id));
        lines.extend(self.crab_lines_here(id));
        lines.extend(self.building_lines_here(id));
        lines.extend(self.garden_lines_here(id));
        lines.extend(self.lighthouse_lines_here(id));
        lines.extend(self.meeting_lines_here(id));
        lines.extend(self.play_lines_here(id));
        lines.extend(self.home_life_lines(id));
        lines.extend(self.cases_lines_here(id));
        lines.extend(self.games_lines_here(id));
        lines.extend(self.animal_lines(id));
        lines.extend(self.find_lines(p.x, p.y));
        lines.extend(exits);
        self.send(id, lines);
    }

    // ---- arriving and leaving -----------------------------------------

    fn join(&mut self, id: u64, bot: bool, invite: Option<String>, token: Option<String>, back: bool, tx: ClientTx) {
        let bye = |reason: &str| {
            let _ = tx.send(ToClient::Bye { reason: reason.to_string() });
        };
        // Who is this? An invite link (once), or a device token from one.
        let now = super::accounts::now();
        let mut new_token = None;
        let user = if let Some(inv) = invite.as_deref().map(str::trim).filter(|i| !i.is_empty()) {
            let Some(u) = self.world.db.redeem_invite(&super::names::hash(inv), now) else {
                bye("💌 This invite link has already been used or has expired. Ask an island keeper for a new one!");
                return;
            };
            let Ok(t) = super::accounts::new_secret() else { return };
            if let Err(e) = self.world.db.add_device(&u, &super::names::hash(&t), now) {
                eprintln!("adding a device for {u}: {e}");
                return;
            }
            new_token = Some(t);
            u
        } else if let Some(t) = token.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            match self.world.db.device_user(&super::names::hash(t), now) {
                Some(u) => u,
                None => {
                    bye("💌 This device isn't signed in any more. Ask an island keeper for a new invite link!");
                    return;
                }
            }
        } else {
            bye("💌 world.txt is invite-only. Open the invite link an island keeper sent you!");
            return;
        };
        let key_hash = token.as_deref().or(new_token.as_deref()).map(super::names::hash);
        self.enter(id, user, bot, new_token, key_hash, back, tx);
    }

    /// Someone signed in (or a visitor let in) arrives on the island.
    /// `back`: coming straight back after a dropped connection, which goes
    /// unannounced (after a redeploy, everyone is).
    fn enter(&mut self, id: u64, user: String, bot: bool, new_token: Option<String>, key_hash: Option<String>, back: bool, tx: ClientTx) {
        let bye = |reason: &str| {
            let _ = tx.send(ToClient::Bye { reason: reason.to_string() });
        };
        let now = super::accounts::now();
        let name = self.world.db.nickname(&user).unwrap_or_else(|| user.clone());
        self.world.nicks.insert(user.clone(), name.clone());
        // Already here: the newest arrival wins. The old one is often a phone
        // or tab gone to sleep, whose connection the server can't yet tell is
        // dead, and turning away the refresh would leave them locked out.
        if let Some((&old, p)) = self.players.iter().find(|(_, p)| p.user == user) {
            let _ = p.tx.send(ToClient::Bye { reason: "👋 You've wandered in from another window or device, so this one has gone quiet.".to_string() });
            self.leave(old);
        }
        if !self.is_keeper(&user)
            && let Some(until) = self.world.db.banned_until(&user, key_hash.as_deref(), now) {
                if until - now > 365 * 86_400 {
                    bye("🚫 The island keepers have closed the island to you.");
                } else {
                    let mins = ((until - now) / 60).max(1);
                    bye(&format!("🌙 A keeper has asked you to take a break. You can come back in about {mins} minutes."));
                }
                return;
            }
        let saved = self.world.db.player(&user);
        // Invited accounts start off the map, until their first arrival.
        let first_time = !bot && saved.is_none_or(|s| s.0 < 0 && crate::server::world::room_of(s.0, s.1).is_none());
        let tutorial = if bot { tutorial::DONE } else { saved.map_or(0, |s| s.2) };
        let (x, y) = saved
            .map(|s| (s.0, s.1))
            .filter(|&(x, y)| match crate::server::world::room_of(x, y) {
                Some(_) => self.world.room_building(x, y).is_some(),
                None => self.world.terrain(x, y).walkable(),
            })
            .unwrap_or_else(|| self.world.spawn_point());
        let sky = self.world.weather.sky_at(x, y);
        let companion = self.world.db.companion(&user);
        let muted = self.world.db.mutes(&user).unwrap_or_default().into_iter().map(|m| m.to_lowercase()).collect();
        let full = super::names::full(&name, &user);
        // Their timezone: what this device says, or where they were last time.
        let (tz, tz_known) = match self.zones.remove(&id) {
            Some(tz) => {
                let _ = self.world.db.set_zone(&user, tz.name());
                (tz, true)
            }
            None => (self.zone_of(&user), self.world.db.zone(&user).is_some()),
        };
        let _ = tx.send(ToClient::Welcome { token: new_token, user: user.clone(), nickname: name.clone() });
        self.players.insert(id, Player { name: name.clone(), user: user.clone(), x, y, tx, sky, bot, reply_to: None, tutorial, fishing: None, reading: None, muted, key_hash, travel: None, walk_to: None, heading: None, join_after: None, voyage: None, tz, tz_known, companion, last_ambient: None, ambient_quiet: 0, radios: HashMap::new(), seen_acts: HashMap::new(), status_due: std::cell::Cell::new(false), fun: play::Fun::wearing(self.world.db.wearing(&user)) });

        self.send(
            id,
            vec![
                Line::new().b(Col::Magenta, "✨ world.txt ✨"),
                Line::col(Col::Dim, "A small island with no battles, just bonhomie. Type 'help' if you're lost."),
                Line::new(),
            ],
        );
        self.look(id);
        self.status(id);
        if first_time {
            self.say1(id, Line::new().b(Col::Yellow, format!("🐣 Welcome to the island, {name}! Here's how to get started.")));
            self.say1(id, Line::col(Col::Dim, format!("Your username is {user}: it's yours forever. Your nickname ({name}) can change any time: nickname Pip")));
        }
        self.show_hint(id);
        if self.is_keeper(&user) {
            self.tell_keepers_about_visitors();
        }
        if let Some(f) = Clock::festival() {
            self.say1(id, Line::new().b(Col::Yellow, format!("{} {}", f.icon, f.greeting)));
        }
        self.deliver_mail(id);
        if !bot {
            self.garden_news(id);
        }
        if first_time {
            // They're here now: a dropped connection before their first step
            // shouldn't make them new all over again.
            self.save_progress(id);
        }
        for (&pid, p) in &self.players {
            if pid != id && !back {
                let line = if p.x == x && p.y == y {
                    Line::new().b(Col::Cyan, &name).c(Col::Cyan, " appears, blinking, as if from a dream.")
                } else if first_time {
                    Line::col(Col::Dim, format!("🐣 {full} has arrived on the island for the first time!"))
                } else {
                    Line::col(Col::Dim, format!("✧ {full} has wandered into the world."))
                };
                let _ = p.tx.send(ToClient::Lines { lines: vec![line] });
            }
        }
        self.status_all();
    }

    fn leave(&mut self, id: u64) {
        if self.waiting.remove(&id).is_some() {
            return; // a visitor who gave up waiting
        }
        self.juke_leave(id);
        self.games_walks(id);
        self.stop_spins_by(id);
        let Some(p) = self.players.remove(&id) else { return };
        if p.bot {
            // Residents keep no position or progress.
        } else if let Err(e) = self.world.db.save_player(&p.user, p.x, p.y, p.tutorial) {
            eprintln!("saving {}: {e}", p.user);
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
            if self.at_sea(id) {
                return Ok(());
            }
            if self.in_room(id) {
                return self.cmd_go_out(id);
            }
            self.stop_travelling(id, false);
            return self.walk(id, dx, dy);
        }
        if let Some(done) = self.crab_command(id, &verb, rest) {
            return done;
        }
        // In and out of buildings.
        let r = rest.trim().to_lowercase();
        match verb.as_str() {
            "enter" => return self.cmd_enter(id, rest.trim_start_matches("into ")),
            "in" | "inside" | "indoors" if r.is_empty() => return self.cmd_enter(id, ""),
            "go" | "walk" | "step" | "head" if matches!(r.as_str(), "in" | "inside" | "indoors") => return self.cmd_enter(id, ""),
            "go" | "walk" | "step" | "head" if r.starts_with("into ") || r.starts_with("in ") => {
                return self.cmd_enter(id, r.trim_start_matches("into ").trim_start_matches("in "));
            }
            "out" | "outside" | "outdoors" | "exit" if self.in_room(id) => return self.cmd_go_out(id),
            "leave" if self.in_room(id) && r.is_empty() => return self.cmd_go_out(id),
            "go" | "walk" | "step" | "head" if self.in_room(id) && matches!(r.as_str(), "out" | "outside" | "outdoors") => return self.cmd_go_out(id),
            _ => {}
        }
        if self.indoors_says_no(id, &verb, rest) {
            return Ok(());
        }
        if let Some(done) = self.cases_command(id, &verb, rest) {
            return done;
        }
        if let Some(done) = self.together_command(id, &verb, rest) {
            return done;
        }
        if let Some(done) = self.hills_command(id, &verb, rest) {
            return done;
        }
        if let Some(done) = self.games_command(id, &verb, rest) {
            return done;
        }
        if let Some(done) = self.play_command(id, &verb, rest) {
            return done;
        }
        if homes::BUILDING.contains(&verb.as_str()) && !self.may_build_here(id) {
            return Ok(());
        }

        match verb.as_str() {
            "help" | "?" => {
                let keeper = self.is_keeper(&self.user(id));
                self.send(id, help(rest, keeper));
            }
            "look" | "l" | "examine" | "x" if rest.is_empty() => self.look(id),
            "look" | "l" | "examine" | "x" => self.cmd_examine(id, rest),
            "go" | "walk" if matches!(rest.trim_start_matches("to the ").trim_start_matches("to ").trim(), "dock" | "the dock") => {
                return self.cmd_go_dock(id);
            }
            "go" | "walk" if is_go_event(rest) => {
                return self.cmd_go_event(id, rest.trim_start_matches("to ").trim_start_matches("event"));
            }
            "event" | "plan" => return self.cmd_event(id, rest),
            "events" | "calendar" => return self.cmd_events(id),
            "going" | "rsvp" | "attend" => return self.cmd_going(id, rest, true),
            "not" if rest.starts_with("going") => return self.cmd_going(id, rest.trim_start_matches("going"), false),
            "cancel" => return self.cmd_cancel_event(id, rest),
            "go" | "walk" => match DIRS.iter().find(|d| d.0 == rest || d.1 == rest) {
                Some(&(_, _, dx, dy)) => return self.walk(id, dx, dy),
                None if rest.trim_start_matches("to ").trim() == "home" => return self.cmd_home(id),
                None => return self.cmd_go_place(id, rest),
            },
            "places" | "where-to" => return self.cmd_places(id),
            "say" => return self.cmd_say(id, rest),
            "emote" | "me" => return self.cmd_emote(id, rest),
            "call" if rest.starts_with("https://") || rest == "off" || (rest.is_empty() && self.meeting_here(id)) => return self.cmd_meet(id, rest),
            "meet" | "meeting" => return self.cmd_meet(id, rest),
            "shout" | "yell" | "call" => return self.cmd_shout(id, rest),
            "hum" | "sing" | "whistle" => return self.cmd_hum(id, &verb, rest),
            "who" => self.cmd_who(id),
            "time" | "clock" => self.cmd_time(id, rest),
            "sail" | "row" | "boat" => return self.cmd_sail(id, rest.trim_start_matches("to ")),
            "befriend" | "feed" | "tame" => return self.cmd_befriend(id, rest),
            "petname" => return self.cmd_nickname(id, rest),
            "nickname" | "nick" => return self.cmd_rename(id, rest),
            "pet" | "pat" | "cuddle" => return self.cmd_pet(id),
            "dismiss" => return self.cmd_let_go(id),
            "let" if rest.trim_start().starts_with("go") => return self.cmd_let_go(id),
            "teach" => return self.cmd_teach(id, rest),
            "play" | "wind" if rest.starts_with("music") || rest.starts_with("songs") || rest.starts_with("records") => {
                return self.cmd_play_music(id);
            }
            "play" | "wind" if self.has_gramophone_here(id) && !self.has_music_box_here(id) => return self.cmd_play_music(id),
            "play" | "wind" => return self.cmd_play(id),
            "tune" => return self.cmd_tune(id, rest),
            "stations" => return self.cmd_stations(id),
            "station" => return self.cmd_station(id, rest),
            "cook" | "bake" | "brew" | "grill" | "roast" | "toast" => return self.cmd_cook(id, rest),
            "eat" | "drink" | "sip" | "nibble" => return self.cmd_eat(id, rest),
            "serve" => return self.cmd_serve(id, rest),
            "add" | "invite" if events::names_and_event(rest, "to").is_some() => {
                let (who, which) = events::names_and_event(rest, "to").unwrap_or_default();
                return self.cmd_add_to_event(id, who, which);
            }
            "remove" | "uninvite" if events::names_and_event(rest, "from").is_some() => {
                let (who, which) = events::names_and_event(rest, "from").unwrap_or_default();
                return self.cmd_remove_from_event(id, who, which);
            }
            "add" if self.adding_to_board(id, rest) => return self.cmd_add_to_board(id, rest),
            "add" | "queue" if self.has_jukebox_here(id) => return self.cmd_add_songs(id),
            "jukebox" | "juke" => return self.cmd_jukebox(id),
            "skip" if rest.is_empty() && self.juke_skippable(id) => return self.cmd_skip_song(id),
            "unqueue" => return self.cmd_unqueue(id),
            "radio" if rest.trim() == "off" => return self.cmd_radio_off(id),
            "radio" if self.has_radio_here(id) => return self.cmd_tune(id, rest),
            "join" | "visit" => return self.cmd_join(id, rest),
            "stop" if rest.starts_with("music") || rest.starts_with("the music") => return self.cmd_stop_music(id),
            "stop" => self.stop_travelling(id, true),
            "map" | "m" => {
                if let Some(p) = self.players.get(&id) {
                    let people: Vec<(i64, i64)> =
                        self.players.iter().filter(|(pid, _)| **pid != id).map(|(_, o)| (o.x, o.y)).collect();
                    let mut lines = vec![Line::new().b(Col::Magenta, "🗺  The island")];
                    lines.extend(self.world.island_map(self.world.outside(p.x, p.y), &people));
                    self.send(id, lines);
                }
            }
            "claim" => return self.cmd_claim(id),
            "home" => return self.cmd_home(id),
            "share" => return self.cmd_invite(id, rest, true),
            "unshare" => return self.cmd_invite(id, rest, false),
            "invite" => return self.cmd_new_invite(id, rest),
            "link" => return self.cmd_new_link(id, rest),
            "describe" => return self.cmd_describe(id, rest),
            "fold" => return self.cmd_fold(id, rest),
            "invites" => return self.cmd_invites(id),
            "revoke" => return self.cmd_revoke(id, rest),
            "guests" => return self.cmd_guests(id),
            "read" => return self.cmd_read(id, rest),
            "post" | "note" => return self.cmd_post(id, rest),
            "follow" => return self.cmd_follow(id, true, rest),
            "leave" if rest.trim_start().starts_with("event") => return self.cmd_leave_event(id, rest.trim_start().trim_start_matches("event")),
            "unfollow" | "leave" => return self.cmd_follow(id, false, rest),
            "boards" => return self.cmd_boards(id),
            "archive" => return self.cmd_archive(id),
            "hook" => return self.cmd_hook(id, rest, "notes"),
            "progress" => return self.cmd_hook(id, rest, "progress"),
            "status" => return self.cmd_hook(id, rest, "status"),
            "clear" => return self.cmd_clear_bar(id, rest),
            "unhook" => return self.cmd_unhook(id, rest),
            "hooks" => return self.cmd_hooks(id),
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
            "build" | "construct" if rest.trim().is_empty() => return self.cmd_blueprints(id),
            "blueprints" | "buildings" => return self.cmd_blueprints(id),
            "make" | "build" | "create" | "construct" => {
                if let Some(done) = self.cmd_build(id, &verb, rest) {
                    return done;
                }
                return self.cmd_make(id, rest);
            }
            "craft" => return self.cmd_craft(id, rest),
            "mend" | "repair" | "fix" => return self.cmd_mend(id),
            "recipes" => return self.cmd_recipes(id),
            "unmake" | "destroy" | "remove" | "demolish" | "unbuild" | "knock" => return self.cmd_unmake(id, rest.trim_start().strip_prefix("down ").unwrap_or(rest)),
            "undo" => return self.cmd_undo(id),
            "mute" | "block" => return self.cmd_mute(id, rest, true),
            "unmute" | "unblock" => return self.cmd_mute(id, rest, false),
            "mutes" => return self.cmd_mutes(id),
            "kick" => return self.cmd_kick(id, rest),
            "ban" => return self.cmd_ban(id, rest),
            "unban" => return self.cmd_unkick(id, rest),
            "promote" if rest.contains('@') => self.say1(id, Line::plain("Visitors from other worlds can't be keepers here.")),
            "promote" => return self.cmd_promote(id, rest, true),
            "admit" => return self.cmd_admit(id, rest),
            "turn" if rest.trim_start().starts_with("away") => return self.cmd_turn_away(id, rest),
            "visitors" | "guests-from-afar" => return self.cmd_visitors(id),
            "trust" => return self.cmd_trust(id, rest, true),
            "untrust" | "distrust" => return self.cmd_trust(id, rest, false),
            "worlds" => return self.cmd_worlds(id),
            "demote" => return self.cmd_promote(id, rest, false),
            "signout" | "sign-out" => return self.cmd_signout(id, rest),
            "users" | "accounts" => return self.cmd_users(id),
            "unkick" => return self.cmd_unkick(id, rest),
            "push" | "carry" | "move" => return self.cmd_push(id, rest),
            "paint" | "repaint" | "colour" | "color" => return self.cmd_paint(id, rest),
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
        let quiet = p.travel.is_some() || p.walk_to.is_some(); // auto-walking: no look at every step
        let (nx, ny) = (ox + dx, oy + dy);
        let to = self.world.terrain(nx, ny);
        if !in_bounds(nx, ny) || !to.walkable() {
            self.say1(id, Line::col(Col::Blue, "The water gets too deep to wade any further. You turn back."));
            return Ok(());
        }
        let dir = DIRS.iter().find(|d| d.2 == dx && d.3 == dy).map_or("away", |d| d.1);
        let back = DIRS.iter().find(|d| d.2 == -dx && d.3 == -dy).map_or("somewhere", |d| d.1);
        self.kite_walks(id);
        self.games_walks(id);
        let with = self.with_companion(id);
        self.tell_here(ox, oy, id, Line::col(Col::Cyan, format!("{name} wanders off to the {dir}{with}.")));
        if let Some(p) = self.players.get_mut(&id) {
            p.x = nx;
            p.y = ny;
            p.sky = self.world.weather.sky_at(nx, ny);
            p.fishing = None; // walking off reels the line in
        }
        self.tell_here(nx, ny, id, Line::col(Col::Cyan, format!("{name} arrives from the {back}{with}.")));
        self.visit_home(id, (ox, oy), (nx, ny));
        if quiet {
            // just the step, below
        } else {
            if to.is_wet() && !self.world.terrain(ox, oy).is_wet() {
                self.say1(id, Line::col(Col::Blue, "You wade in. The water is cool around your legs."));
            }
            self.look(id);
        }
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
        self.status_near(id, &[(ox, oy), (nx, ny)]);
        Ok(())
    }

    fn cmd_say(&mut self, id: u64, msg: &str) -> anyhow::Result<()> {
        if msg.is_empty() {
            self.say1(id, Line::plain("Say what, babe?"));
            return Ok(());
        }
        let Some(p) = self.players.get(&id) else { return Ok(()) };
        let (x, y, name, user) = (p.x, p.y, p.name.clone(), p.user.clone());
        self.say1(id, Line::new().c(Col::Dim, "You say, ").c(Col::White, format!("\"{msg}\"")));
        self.tell_here_from(x, y, id, &user, Line::new().b(Col::Cyan, &name).c(Col::Dim, " says, ").c(Col::White, format!("\"{msg}\"")));
        self.react_to_words(x, y);
        Ok(())
    }

    fn cmd_emote(&mut self, id: u64, msg: &str) -> anyhow::Result<()> {
        if msg.is_empty() {
            self.say1(id, Line::plain("Emote what? e.g. ':stretches and yawns'"));
            return Ok(());
        }
        let Some(p) = self.players.get(&id) else { return Ok(()) };
        let (x, y, name, user) = (p.x, p.y, p.name.clone(), p.user.clone());
        let line = Line::new().b(Col::Cyan, &name).c(Col::Cyan, format!(" {msg}"));
        self.say1(id, line.clone());
        self.tell_here_from(x, y, id, &user, line);
        Ok(())
    }

    fn cmd_shout(&mut self, id: u64, msg: &str) -> anyhow::Result<()> {
        if msg.is_empty() {
            self.say1(id, Line::plain("Shout what?"));
            return Ok(());
        }
        let Some(me) = self.players.get(&id) else { return Ok(()) };
        self.say1(id, Line::new().c(Col::Dim, "You shout, ").b(Col::White, format!("\"{msg}!\"")));
        // Indoors, a shout fills the room and carries just outside; walls
        // keep it from anyone in another room.
        let my_room = crate::server::world::room_of(me.x, me.y);
        let from = self.world.outside(me.x, me.y);
        let indoors = my_room.and_then(|_| self.world.room_building(me.x, me.y)).map(|b| the(&b.name));
        for (&pid, p) in &self.players {
            if pid == id || !p.hears(&me.user) {
                continue;
            }
            let their_room = crate::server::world::room_of(p.x, p.y);
            if their_room.is_some() && their_room != my_room {
                continue;
            }
            let (dx, dy) = if their_room.is_some() { (0, 0) } else { (from.0 - p.x, from.1 - p.y) };
            let d = dx.abs().max(dy.abs());
            if d > SHOUT_RANGE || (indoors.is_some() && their_room.is_none() && d > 1) {
                continue;
            }
            let line = if let (Some(b), None) = (&indoors, their_room) {
                Line::new().c(Col::Dim, format!("From inside {b}, ")).b(Col::Cyan, &me.name).c(Col::Dim, " shouts, ").c(Col::White, format!("\"{msg}!\""))
            } else if d == 0 {
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
        self.echo(id, msg);
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
        self.notice_jam(x, y, &name);
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
        if !p.bot
            && let Err(e) = self.world.db.save_player(&p.user, p.x, p.y, p.tutorial) {
                eprintln!("saving {}: {e}", p.user);
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
        let exact = self.players.iter().find(|(_, p)| p.name.to_lowercase() == n || p.user == n);
        let prefix = || self.players.iter().find(|(_, p)| p.name.to_lowercase().starts_with(&n));
        exact.or_else(prefix).map(|(&pid, _)| pid)
    }

    fn cmd_tell(&mut self, id: u64, to: &str, msg: &str) -> anyhow::Result<()> {
        if to.is_empty() || msg.is_empty() {
            self.say1(id, Line::plain("Tell who what? e.g. '@Poppy want to build a boat?'"));
            return Ok(());
        }
        let Some(tid) = self.find_player(to) else {
            return self.leave_mail(id, to, msg);
        };
        if tid == id {
            self.say1(id, Line::plain("You mutter to yourself. Very wise."));
            return Ok(());
        }
        let (from, from_user) = (self.players[&id].name.clone(), self.players[&id].user.clone());
        let (target, target_user) = (self.players[&tid].name.clone(), self.players[&tid].user.clone());
        self.say1(id, Line::new().c(Col::Pink, format!("✉ You say to {target}: ")).c(Col::White, msg));
        if !self.players[&tid].hears(&from_user) {
            return Ok(()); // muted: quietly not delivered
        }
        self.say1(tid, Line::new().b(Col::Pink, format!("✉ {from}")).c(Col::Pink, " says to you: ").c(Col::White, msg));
        if let Some(t) = self.players.get_mut(&tid) {
            t.reply_to = Some(from_user);
        }
        if let Some(me) = self.players.get_mut(&id) {
            me.reply_to.get_or_insert(target_user);
        }
        Ok(())
    }

    /// `@Name` for someone who isn't here: keep it for when they next arrive.
    fn leave_mail(&mut self, id: u64, to: &str, msg: &str) -> anyhow::Result<()> {
        let Some(target_user) = self.world.db.resolve_user(to) else {
            self.say1(id, Line::plain(format!("There's nobody called {to} on the island. Try 'who'.")));
            return Ok(());
        };
        let target = self.world.nick(&target_user);
        let from = self.user(id);
        let text: String = msg.chars().take(280).collect();
        self.world.db.send_mail(&target_user, &from, &text, boards::now())?;
        if let Some(me) = self.players.get_mut(&id) {
            me.reply_to = Some(target_user.clone());
        }
        self.say1(id, Line::new().c(Col::Pink, format!("📬 {target} isn't here right now, so your message will be waiting for them: ")).c(Col::White, text));
        Ok(())
    }

    /// Hand over anything people said while this player was away.
    fn deliver_mail(&mut self, id: u64) {
        let Some(p) = self.players.get(&id) else { return };
        let Ok(mail) = self.world.db.take_mail(&p.user) else { return };
        let mail: Vec<_> = mail.into_iter().filter(|(from, _, _)| p.hears(from)).collect();
        if mail.is_empty() {
            return;
        }
        let mut lines = vec![Line::new().b(Col::Pink, "📬 While you were away:")];
        let now = boards::now();
        for (from, text, at) in &mail {
            let ago = now - at;
            let when = match ago {
                ..3600 => format!("{}m ago", (ago / 60).max(1)),
                3600..86_400 => format!("{}h ago", ago / 3600),
                _ => format!("{}d ago", ago / 86_400),
            };
            lines.push(Line::new().t("  ").b(Col::Pink, self.world.nick(from)).c(Col::Dim, format!(" ({when}): ")).c(Col::White, text.clone()));
        }
        lines.push(Line::col(Col::Dim, "  (reply to answer the last one)"));
        let last = mail.last().map(|m| m.0.clone());
        if let Some(p) = self.players.get_mut(&id) {
            p.reply_to = last;
        }
        self.send(id, lines);
    }

    fn cmd_chat(&mut self, id: u64, msg: &str) -> anyhow::Result<()> {
        if msg.is_empty() {
            self.say1(id, Line::plain("Chat what? Everyone in the world will hear it."));
            return Ok(());
        }
        let (from, from_user) = (self.players[&id].name.clone(), self.players[&id].user.clone());
        let line = Line::new().c(Col::Green, "💬 ").b(Col::Green, &from).c(Col::Green, ": ").c(Col::White, msg);
        for p in self.players.values().filter(|p| p.hears(&from_user)) {
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
        let place = place.strip_prefix("Inside ").map_or(place.clone(), |p| format!("{p} (inside)"));
        let ((mx, my), (tx, ty)) = (self.world.outside(me.x, me.y), self.world.outside(them.x, them.y));
        let (dx, dy) = (tx - mx, ty - my);
        let d = dx.abs().max(dy.abs());
        let line = if tid == id {
            "You're right here, silly.".to_string()
        } else if d == 0 {
            format!("{} is right here with you!", them.name)
        } else {
            let dir = DIRS.iter().find(|q| q.2 == dx.signum() && q.3 == dy.signum()).map_or("away", |q| q.1);
            let steps = if d == 1 { "1 step".to_string() } else { format!("{d} steps") };
            let (mine, theirs) = (super::islands::island_at(me.x, me.y), super::islands::island_at(them.x, them.y));
            if mine.id != theirs.id {
                format!("{} is at {place}, over on {}. Sail there from a dock! ⛵", them.name, theirs.name)
            } else {
                format!("{} is at {place}, about {steps} to the {dir}.", them.name)
            }
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
                    .c(Col::Dim, format!(" {}", p.user))
                    .c(Col::Magenta, p.fun.wearing.as_ref().map_or(String::new(), |w| format!(" (wearing {w})")))
                    .c(Col::Dim, format!("  · {}", self.world.place_title(p.x, p.y)))
                    .c(Col::Dim, if p.bot { "  island resident".to_string() } else if p.tz_known { format!("  · {}", time_there(p.tz)) } else { String::new() }),
            );
        }
        lines.push(Line::col(Col::Dim, "Talk to anyone, anywhere: @Name hello"));
        self.send(id, lines);
    }

    /// `time`: the real time where you are; `time Corey`: where they are
    /// (or were last), even while they're away.
    fn cmd_time(&self, id: u64, who: &str) {
        let Some(me) = self.players.get(&id) else { return };
        let who = who.trim().trim_start_matches('@');
        if who.is_empty() || who.eq_ignore_ascii_case("me") {
            let now = chrono::Utc::now().with_timezone(&me.tz);
            self.say1(id, Line::col(Col::Cyan, format!("🕰 It's {} where you are ({}). The island's own clock says {}.", now.format("%a %-d %b, %-I:%M%P"), now.format("%Z"), Clock::label())));
            return;
        }
        let Some(user) = self.find_player(who).map(|pid| self.players[&pid].user.clone()).or_else(|| self.world.db.resolve_user(who)) else {
            self.say1(id, Line::plain(format!("There's nobody called {who}. Try 'who'.")));
            return;
        };
        let name = self.world.nick(&user);
        let here = self.players.values().find(|p| p.user == user);
        let line = match (here, self.world.db.zone(&user).and_then(|z| z.parse::<chrono_tz::Tz>().ok())) {
            (Some(p), _) if p.tz_known => format!("🕰 It's {} for {name}.", time_there(p.tz)),
            (None, Some(tz)) => format!("🕰 It's {} for {name}, as far as the island knows (they're away just now).", time_there(tz)),
            _ => format!("🕰 The island doesn't know where in the world {name} is yet: their device hasn't said."),
        };
        self.say1(id, Line::col(Col::Cyan, line));
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
                lines.push(Line::col(Col::Dim, format!("Made by {}.", self.world.nick(&t.maker))));
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
            Stone => self.reshape(
                id,
                Hill,
                "You chip away at the rock, stone by stone, until it's worn down to a hill. ⛏",
                "chips away at the rock until it's just a hill.",
            ),
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

    /// `describe A messy little den…`: your own words for a spot at your home,
    /// shown in place of the land's when anyone looks. `describe` alone clears it.
    fn cmd_describe(&mut self, id: u64, text: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        // Homes are checked before this (only their people may change them).
        if self.world.home_at(x, y).is_none() && crate::server::world::room_of(x, y).is_none() && !self.is_keeper(&user) {
            self.say1(id, Line::plain("You can describe places at your home. Find a spot you like and 'claim' it first!"));
            return Ok(());
        }
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(DESCRIBE_MAX).collect();
        if text.is_empty() {
            if self.world.descriptions.contains_key(&(x, y)) {
                self.world.set_description(x, y, None, &user, boards::now())?;
                self.say1(id, Line::col(Col::Green, "You let this place go back to how it was."));
            } else {
                self.say1(id, Line::plain("Describe it how? e.g. 'describe A messy little den, records stacked everywhere and fairy lights over the jukebox.'"));
            }
            return Ok(());
        }
        let mut text = capitalise(&text);
        if !text.ends_with(['.', '!', '?', '…', '"', ')']) {
            text.push('.');
        }
        self.world.set_description(x, y, Some(text), &user, boards::now())?;
        self.say1(id, Line::col(Col::Green, "✍ You take in the place and make it yours. 'look' to see it."));
        self.tell_here(x, y, id, Line::col(Col::Green, format!("{who} looks around, making this place their own.")));
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
        if gramophone::is_gramophone(&name) || radio::is_radio(&name) || jukebox::is_jukebox(&name) {
            if let Some(other) = self.sound_too_close(x, y, None) {
                self.say1(id, Line::plain(format!("{} is too close: two songs at once would be a muddle. Try ten or so steps further away.", capitalise(&the(&other)))));
                return Ok(());
            }
            let (kind, how) = if jukebox::is_jukebox(&name) {
                (jukebox::kind(), "Anyone can 'add songs' from their computer, and it plays one each, taking turns.")
            } else if gramophone::is_gramophone(&name) {
                (gramophone::kind(), "Stand here and 'play music' to play songs from your computer for everyone nearby.")
            } else {
                (radio::kind(), "Tune it to one of the island's stations: tune radio <station> ('stations' lists them)")
            };
            self.world.add_thing(x, y, name.clone(), None, Trigger::Always, &self.user(id), Some(kind))?;
            self.say1(id, Line::new().c(Col::Pink, "🎶 You make ").b(Col::Pink, &name).c(Col::Pink, format!(". {how}")));
            self.tell_here(x, y, id, Line::col(Col::Pink, format!("🎶 {who} makes {name}.")));
            self.status_all();
            return Ok(());
        }
        if let Some(kind) = cook::kitchen(&name) {
            self.world.add_thing(x, y, name.clone(), None, Trigger::Always, &self.user(id), Some(kind))?;
            self.say1(id, Line::new().c(Col::Yellow, "🔥 You make ").b(Col::Yellow, &name).c(Col::Yellow, ". Type cook to see what you could make here."));
            self.tell_here(x, y, id, Line::col(Col::Yellow, format!("🔥 {who} makes {name}.")));
            self.status_all();
            return Ok(());
        }
        if cases::is_case(&name) {
            self.world.add_thing(x, y, name.clone(), None, Trigger::Always, &self.user(id), Some(cases::KIND))?;
            self.say1(id, Line::new().c(Col::Yellow, "🫙 You make ").b(Col::Yellow, &name).c(Col::Yellow, ". Show off your finds in it: put firefly in jar, put shell in case…"));
            self.tell_here(x, y, id, Line::col(Col::Yellow, format!("🫙 {who} makes {name}.")));
            self.status_all();
            return Ok(());
        }
        if music::is_music_box(&name) {
            self.world.add_thing(x, y, name.clone(), None, Trigger::Always, &self.user(id), Some(music::music_kind()))?;
            self.say1(id, Line::new().c(Col::Pink, "🎶 You make ").b(Col::Pink, &name).c(Col::Pink, ". Teach it a tune: teach C E G E C"));
            self.tell_here(x, y, id, Line::col(Col::Pink, format!("🎶 {who} makes {name}.")));
            self.status_all();
            return Ok(());
        }
        if meetings::is_meeting_spot(&name) {
            if self.world.things_at(x, y).any(|t| t.kind.as_deref() == Some(meetings::kind())) {
                self.say1(id, Line::plain("There's already a gathering spot here. Give it its links: meet https://…"));
                return Ok(());
            }
            self.world.add_thing(x, y, name.clone(), None, Trigger::Always, &self.user(id), Some(meetings::kind()))?;
            self.say1(id, Line::new().c(Col::Cyan, "🔗 You make ").b(Col::Cyan, &name).c(Col::Cyan, ". Give it a link or two for whatever happens here: meet https://…"));
            self.say1(id, Line::col(Col::Dim, "Then anyone here can type meet to get them. Events planned here use them too."));
            self.say1(id, Line::col(Col::Dim, "Next: plan something here (event here fri 4pm Tea party), then bring your people: add Poppy, Sam to event 1. (help gatherings)"));
            self.tell_here(x, y, id, Line::col(Col::Cyan, format!("🔗 {who} makes {name}.")));
            self.status_all();
            return Ok(());
        }
        if boards::is_board_name(&name) {
            if self.world.things_at(x, y).any(|t| t.is_board()) {
                self.say1(id, Line::plain("There's already a board here. Try 'post hello!'"));
                return Ok(());
            }
            self.world.add_thing(x, y, name.clone(), None, Trigger::Always, &self.user(id), Some(boards::board_kind()))?;
            self.say1(id, Line::new().c(Col::Yellow, "📌 You put up ").b(Col::Yellow, &name).c(Col::Yellow, ". Anyone passing can read it and post notes."));
            self.say1(id, Line::col(Col::Dim, "Try: post hello everyone! · read · follow"));
            if self.world.things_at(x, y).any(|t| t.kind.as_deref() == Some(meetings::kind())) {
                self.say1(id, Line::col(Col::Dim, "It's beside a gathering table, so its followers are that table's group. Bring them in: add Poppy, Sam to board"));
            }
            self.tell_here(x, y, id, Line::col(Col::Yellow, format!("📌 {who} puts up {name}.")));
            self.status_all();
            return Ok(());
        }
        self.world.add_thing(x, y, name.clone(), action, trigger, &self.user(id), None)?;
        self.say1(id, Line::new().c(Col::Yellow, "✨ You make ").b(Col::Yellow, &name).c(Col::Yellow, "."));
        self.tell_here(x, y, id, Line::col(Col::Yellow, format!("✨ {who} makes {name}.")));
        self.status_all();
        Ok(())
    }

    /// Keep the timezone a connection says it's in, if it's a real one.
    fn note_zone(&mut self, id: u64, tz: Option<String>) {
        if let Some(tz) = tz.and_then(|t| t.parse::<chrono_tz::Tz>().ok()) {
            self.zones.insert(id, tz);
        }
    }

    /// Someone's timezone: theirs if they're here, where they last were if
    /// not, or the world's own.
    fn zone_of(&self, user: &str) -> chrono_tz::Tz {
        self.players
            .values()
            .find(|p| p.user == user)
            .map(|p| p.tz)
            .or_else(|| self.world.db.zone(user).and_then(|t| t.parse().ok()))
            .unwrap_or_else(world_zone)
    }

    fn is_keeper(&self, user: &str) -> bool {
        self.world.db.is_keeper(user)
    }

    fn link_line(&self, who: &str, user: &str, token: &str) -> Vec<Line> {
        vec![
            Line::new().b(Col::Magenta, format!("💌 Magic link for {}", super::names::full(who, user))).c(Col::Dim, format!(" (one use, within {} days):", super::accounts::INVITE_DAYS)),
            Line::new().t("   ").b(Col::Yellow, super::accounts::link(self.url.as_deref(), token)),
            Line::col(Col::Dim, "   Send it just to them: whoever opens it first becomes them."),
        ]
    }

    /// Keepers: `invite Poppy` makes an account and a magic link for it.
    fn cmd_new_invite(&mut self, id: u64, nickname: &str) -> anyhow::Result<()> {
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("Only island keepers can invite people. Ask one! (To let a friend build at your home: share Lu)"));
            return Ok(());
        }
        let nick = clean_name(nickname);
        if nick.is_empty() {
            self.say1(id, Line::plain("Invite who? e.g. 'invite Poppy' (that'll be their nickname)"));
            return Ok(());
        }
        if self.world.db.resolve_user(&nick).is_some() {
            self.say1(id, Line::plain(format!("Someone's already called {nick}. For their new device: link {nick}")));
            return Ok(());
        }
        let user = super::accounts::create_account(&self.world.db, &nick, false)?;
        self.world.nicks.insert(user.clone(), nick.clone());
        let token = super::accounts::issue_invite(&self.world.db, &user)?;
        let lines = self.link_line(&nick, &user, &token);
        self.send(id, lines);
        Ok(())
    }

    /// Keepers: `link Poppy` makes a new magic link for someone's next device.
    fn cmd_new_link(&mut self, id: u64, name: &str) -> anyhow::Result<()> {
        let me = self.user(id);
        let target = name.trim().to_lowercase();
        if target.is_empty() || target == "me" || target == "myself" || target == "my device" || target == "new device" {
            return self.cmd_link_me(id, &me);
        }
        if !self.is_keeper(&me) {
            self.say1(id, Line::plain("Only island keepers can make links for other people. For your own new device: link me"));
            return Ok(());
        }
        if self.world.db.matching_users(name)?.is_empty() {
            self.say1(id, Line::plain(format!("There's nobody called {} on the island. For someone new: invite {}", name.trim(), name.trim())));
            return Ok(());
        }
        let Some(user) = self.one_user(id, name) else { return Ok(()) };
        let token = super::accounts::issue_invite(&self.world.db, &user)?;
        let lines = self.link_line(&self.world.nick(&user), &user, &token);
        self.send(id, lines);
        Ok(())
    }

    /// `link me`: a magic link for your own new device. You're already signed
    /// in to ask, so it can't get anyone into an account they couldn't already.
    fn cmd_link_me(&mut self, id: u64, me: &str) -> anyhow::Result<()> {
        if me.contains('@') {
            self.say1(id, Line::plain("You're visiting from another world: on a new device, sign in at home and sail over from there."));
            return Ok(());
        }
        let token = super::accounts::issue_invite(&self.world.db, me)?;
        self.send(
            id,
            vec![
                Line::new().b(Col::Magenta, "💌 A magic link for your new device").c(Col::Dim, format!(" (one use, within {} days):", super::accounts::INVITE_DAYS)),
                Line::new().t("   ").b(Col::Yellow, super::accounts::link(self.url.as_deref(), &token)),
                Line::col(Col::Dim, "   Open it on your phone, laptop or tablet, and you'll be you there too. Keep it to yourself: whoever opens it first becomes you."),
            ],
        );
        Ok(())
    }

    /// `nickname Pip`: change what someone's called. Their username never changes.
    fn cmd_rename(&mut self, id: u64, new: &str) -> anyhow::Result<()> {
        let Some(p) = self.players.get(&id) else { return Ok(()) };
        let (old, user) = (p.name.clone(), p.user.clone());
        let nick = clean_name(new);
        if nick.is_empty() {
            self.say1(id, Line::plain(format!("You're {old}, and your username is {user}. Change your nickname with: nickname Pip")));
            return Ok(());
        }
        // Until invites arrive, people still join by nickname, so they must be unique.
        if self.world.db.resolve_user(&nick).is_some_and(|u| u != user) {
            self.say1(id, Line::plain(format!("Someone's already called {nick}. Try another?")));
            return Ok(());
        }
        self.world.db.set_nickname(&user, &nick)?;
        self.world.nicks.insert(user.clone(), nick.clone());
        if let Some(p) = self.players.get_mut(&id) {
            p.name = nick.clone();
        }
        let (x, y) = (self.players[&id].x, self.players[&id].y);
        self.say1(id, Line::col(Col::Cyan, format!("✨ You're {nick} now. (Your username is still {user}.)")));
        self.tell_here(x, y, id, Line::col(Col::Cyan, format!("✨ {old} is now called {nick} ({user}).")));
        self.status_all();
        Ok(())
    }

    /// Someone's username, from their connection.
    fn user(&self, id: u64) -> String {
        self.players.get(&id).map(|p| p.user.clone()).unwrap_or_default()
    }

    /// Only whoever made something (or an island keeper) may unmake or push it.
    fn may_change(&self, who: &str, i: usize) -> bool {
        // `who` is a username.
        self.world.things[i].maker.eq_ignore_ascii_case(who) || self.is_keeper(who)
    }

    fn not_yours(&self, i: usize) -> Line {
        let t = &self.world.things[i];
        let what = crate::server::world::strip_article(&t.name).to_string();
        let maker = self.world.nick(&t.maker);
        if self.world.nicks.contains_key(&t.maker) {
            Line::plain(format!("That's {maker}'s {what}. Only {maker} can unmake or move it. (Ask them: @{maker} ...)"))
        } else {
            Line::plain(format!("That {what} belongs to {maker}. It stays as it is."))
        }
    }

    fn cmd_mute(&mut self, id: u64, name: &str, on: bool) -> anyhow::Result<()> {
        if !self.players.contains_key(&id) { return Ok(()) }
        let name = name.trim();
        if name.is_empty() {
            self.say1(id, Line::plain(if on { "Mute who? e.g. 'mute Sam'" } else { "Unmute who?" }));
            return Ok(());
        }
        // Find them online or not, by nickname or username.
        let known = self
            .find_player(name)
            .map(|pid| self.players[&pid].user.clone())
            .or_else(|| self.world.db.resolve_user(name));
        let Some(target_user) = known else {
            self.say1(id, Line::plain(format!("There's nobody called {name} on the island.")));
            return Ok(());
        };
        let target = super::names::full(&self.world.nick(&target_user), &target_user);
        let who = self.user(id);
        if target_user == who {
            self.say1(id, Line::plain("You can't mute yourself, silly."));
            return Ok(());
        }
        self.world.db.set_mute(&who, &target_user, on)?;
        if let Some(p) = self.players.get_mut(&id) {
            if on {
                p.muted.insert(target_user.to_lowercase());
            } else {
                p.muted.remove(&target_user.to_lowercase());
            }
        }
        let msg = if on {
            format!("🔇 You won't hear from {target} any more, or any music they play. They aren't told. (unmute {target_user} to undo)")
        } else {
            format!("🔈 You'll hear from {target} again.")
        };
        self.say1(id, Line::col(Col::Dim, msg));
        Ok(())
    }

    fn cmd_mutes(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((_, _, _who)) = self.here(id) else { return Ok(()) };
        let mutes: Vec<String> = self
            .world
            .db
            .mutes(&self.user(id))?
            .into_iter()
            .map(|u| super::names::full(&self.world.nick(&u), &u))
            .collect();
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
        if !self.is_keeper(&self.user(id)) {
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
        let (tuser, thash) = match target {
            Some(pid) => (self.players[&pid].user.clone(), self.players[&pid].key_hash.clone()),
            None => match self.world.db.resolve_user(name) {
                Some(u) => (u, None),
                None => {
                    self.say1(id, Line::plain(format!("There's nobody called {name} on the island.")));
                    return Ok(());
                }
            },
        };
        let tname = super::names::full(&self.world.nick(&tuser), &tuser);
        if self.is_keeper(&tuser) {
            self.say1(id, Line::plain("Keepers can't kick other keepers."));
            return Ok(());
        }
        self.world.db.add_ban(&tuser, thash.as_deref(), boards::now() + minutes * 60)?;
        if let Some(pid) = target
            && let Some(p) = self.players.get(&pid) {
                let _ = p.tx.send(ToClient::Bye {
                    reason: format!("🌙 A keeper has asked you to take a break. You can come back in about {minutes} minutes."),
                });
            }
        let gone = if target.is_some() { "has been sent off" } else { "isn't here, but" };
        self.say1(id, Line::col(Col::Yellow, format!("👢 {tname} {gone} for {minutes} minutes (name and device). 'unkick {tuser}' to let them back early.")));
        println!("👢 {who} kicked {tname} for {minutes} minutes");
        Ok(())
    }

    fn cmd_unkick(&mut self, id: u64, name: &str) -> anyhow::Result<()> {
        if !self.players.contains_key(&id) { return Ok(()) }
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("Only island keepers can do that."));
            return Ok(());
        }
        let user = self.world.db.resolve_user(name).unwrap_or_else(|| name.trim().to_string());
        let msg = if self.world.db.lift_ban(&user)? {
            format!("🌤 {} can come back whenever they like.", super::names::full(&self.world.nick(&user), &user))
        } else {
            format!("{} wasn't on a break.", name.trim())
        };
        self.say1(id, Line::col(Col::Green, msg));
        Ok(())
    }

    /// Find someone for a keeper command, by nickname or username.
    fn keeper_target(&self, id: u64, name: &str) -> Option<String> {
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("Only island keepers can do that."));
            return None;
        }
        let found = self.one_user(id, name);
        if found.is_none() && self.world.db.matching_users(name).is_ok_and(|m| m.is_empty()) {
            self.say1(id, Line::plain(format!("There's nobody called '{}' on the island. ('users' lists everyone)", name.trim())));
        }
        found
    }

    /// The one account a name means. If it could mean several (old accounts
    /// from before nicknames), say which usernames to use instead.
    fn one_user(&self, id: u64, name: &str) -> Option<String> {
        let found = self.world.db.matching_users(name).unwrap_or_default();
        if found.len() > 1 {
            let which: Vec<String> = found.iter().map(|u| super::names::full(&self.world.nick(u), u)).collect();
            self.say1(id, Line::plain(format!("'{}' could be {}. Use their username.", name.trim(), which.join(" or "))));
            return None;
        }
        found.into_iter().next()
    }

    /// Keepers: recent magic links and what became of them.
    fn cmd_invites(&mut self, id: u64) -> anyhow::Result<()> {
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("Only island keepers can see invites."));
            return Ok(());
        }
        let now = super::accounts::now();
        let mut lines = vec![Line::new().b(Col::Magenta, "💌 Recent magic links")];
        let list = self.world.db.invites(20)?;
        if list.is_empty() {
            lines.push(Line::col(Col::Dim, "  None yet. 'invite Poppy' makes one."));
        }
        for (user, nick, expires, used_at) in list {
            let state = super::accounts::invite_state(expires, used_at, now);
            let col = if state.starts_with("waiting") { Col::Yellow } else { Col::Dim };
            lines.push(Line::new().t("  ").b(Col::Cyan, nick).c(Col::Dim, format!(" {user}  · ")).c(col, state));
        }
        lines.push(Line::col(Col::Dim, "  'revoke Sam' cancels their unused links."));
        self.send(id, lines);
        Ok(())
    }

    /// Keepers: `revoke Sam` cancels their unused links.
    fn cmd_revoke(&mut self, id: u64, name: &str) -> anyhow::Result<()> {
        let Some(target) = self.keeper_target(id, name) else { return Ok(()) };
        let who = super::names::full(&self.world.nick(&target), &target);
        let msg = match self.world.db.revoke_invites(&target)? {
            0 => format!("{who} has no unused links."),
            1 => format!("✂️  Cancelled {who}'s unused link."),
            n => format!("✂️  Cancelled {who}'s {n} unused links."),
        };
        self.say1(id, Line::col(Col::Yellow, msg));
        Ok(())
    }

    /// Keepers: `fold Poppy into sunny-otter-42` moves an old account's things
    /// into someone's account and removes the old one.
    fn cmd_fold(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("Only island keepers can do that."));
            return Ok(());
        }
        let Some((old, new)) = rest.split_once(" into ").map(|(a, b)| (a.trim(), b.trim())).filter(|(a, b)| !a.is_empty() && !b.is_empty()) else {
            self.say1(id, Line::plain("Fold who into who? e.g. 'fold Poppy into sunny-otter-42' (use usernames: 'users' lists them)"));
            return Ok(());
        };
        // Usernames only: this can't be undone, so no guessing from nicknames.
        let exact = |n: &str| self.world.db.matching_users(n).ok().and_then(|m| m.into_iter().find(|u| u.eq_ignore_ascii_case(n)));
        let (Some(old), Some(new)) = (exact(old), exact(new)) else {
            self.say1(id, Line::plain("Use both usernames exactly ('users' lists them), e.g. 'fold Poppy into sunny-otter-42'."));
            return Ok(());
        };
        if old == new {
            self.say1(id, Line::plain("That's the same account."));
            return Ok(());
        }
        let from = super::names::full(&self.world.nick(&old), &old);
        let to = super::names::full(&self.world.nick(&new), &new);
        self.send_off(&old, &format!("🧺 This account has been folded into {to}. Ask an island keeper for a magic link."));
        self.world.fold_account(&old, &new)?;
        self.say1(id, Line::col(Col::Green, format!("🧺 Folded {from} into {to}: their satchel, journal, made things, notes and mail are {}'s now.", self.world.nick(&new))));
        println!("🧺 {} folded {from} into {to}", self.user(id));
        Ok(())
    }

    fn send_off(&self, user: &str, reason: &str) {
        for p in self.players.values().filter(|p| p.user == user) {
            let _ = p.tx.send(ToClient::Bye { reason: reason.to_string() });
        }
    }

    fn cmd_ban(&mut self, id: u64, name: &str) -> anyhow::Result<()> {
        let Some(target) = self.keeper_target(id, name) else { return Ok(()) };
        if self.is_keeper(&target) {
            self.say1(id, Line::plain("Keepers can't ban other keepers. (demote them first, if you must)"));
            return Ok(());
        }
        // A ban is a break that never ends.
        self.world.db.add_ban(&target, None, i64::MAX / 2)?;
        self.send_off(&target, "🚫 The island keepers have closed the island to you.");
        let who = super::names::full(&self.world.nick(&target), &target);
        self.say1(id, Line::col(Col::Yellow, format!("🚫 {who} is banned. 'unban {target}' to let them back.")));
        println!("🚫 {} banned {who}", self.user(id));
        Ok(())
    }

    fn cmd_promote(&mut self, id: u64, name: &str, keeper: bool) -> anyhow::Result<()> {
        let Some(target) = self.keeper_target(id, name) else { return Ok(()) };
        let who = super::names::full(&self.world.nick(&target), &target);
        if !keeper && target == self.user(id) && self.world.db.keeper_count() <= 1 {
            self.say1(id, Line::plain("You're the last keeper, so you'd better stay one! Promote someone else first."));
            return Ok(());
        }
        self.world.db.set_keeper(&target, keeper)?;
        if keeper {
            self.say1(id, Line::col(Col::Green, format!("🗝  {who} is an island keeper now.")));
            if let Some(pid) = self.find_player(&target) {
                self.say1(pid, Line::col(Col::Green, "🗝  You're an island keeper now! Type help to see what keepers can do."));
            }
        } else {
            self.say1(id, Line::col(Col::Dim, format!("{who} isn't a keeper any more.")));
        }
        Ok(())
    }

    fn cmd_signout(&mut self, id: u64, name: &str) -> anyhow::Result<()> {
        let Some(target) = self.keeper_target(id, name) else { return Ok(()) };
        let n = self.world.db.remove_devices(&target)?;
        self.send_off(&target, "💌 You've been signed out. Ask an island keeper for a new magic link to come back.");
        let who = super::names::full(&self.world.nick(&target), &target);
        let devices = if n == 1 { "1 device".to_string() } else { format!("{n} devices") };
        self.say1(id, Line::col(Col::Yellow, format!("💌 Signed {who} out of {devices}. 'link {target}' makes them a new magic link.")));
        Ok(())
    }

    fn cmd_users(&mut self, id: u64) -> anyhow::Result<()> {
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("Only island keepers can see everyone. Try 'who' for who's here now."));
            return Ok(());
        }
        let now = super::accounts::now();
        let mut lines = vec![Line::new().b(Col::Magenta, "🪪 Everyone on the island")];
        for (user, nick, keeper, devices, seen) in self.world.db.accounts()? {
            let seen = match seen.map(|t| now - t) {
                None => "never signed in".to_string(),
                Some(s) if s < 3600 => "seen just now".to_string(),
                Some(s) if s < 86_400 => format!("seen {}h ago", s / 3600),
                Some(s) => format!("seen {}d ago", s / 86_400),
            };
            let online = if self.players.values().any(|p| p.user == user) { " ☺ here" } else { "" };
            lines.push(
                Line::new()
                    .t("  ")
                    .b(Col::Cyan, nick)
                    .c(Col::Dim, format!(" {user}"))
                    .c(Col::Yellow, if keeper { " 🗝" } else { "" })
                    .c(Col::Dim, format!("  · {devices} device{} · {seen}", if devices == 1 { "" } else { "s" }))
                    .c(Col::Green, online),
            );
        }
        self.send(id, lines);
        Ok(())
    }

    fn cmd_undo(&mut self, id: u64) -> anyhow::Result<()> {
        if !self.players.contains_key(&id) { return Ok(()) }
        let me = self.user(id);
        let restored = self.world.undo_remove(&me)?.map(|t| (t.x, t.y, t.name.clone()));
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
            Some(i) if self.world.things[i].is_fixed() => {
                self.say1(id, Line::plain("That belongs to the whole island. It stays right where it is."));
            }
            Some(i) if !what.is_empty() && !self.may_change(&self.user(id), i) => {
                self.say1(id, self.not_yours(i));
            }
            Some(i) if !what.is_empty() => {
                let (tid, tx, ty) = (self.world.things[i].id, self.world.things[i].x, self.world.things[i].y);
                self.empty_room(tid, tx, ty);
                let t = self.world.remove_thing(i, &self.user(id))?;
                self.forget_jukebox(t.id);
                self.forget_kitchen(t.id);
                self.stop_spin(t.id);
                self.forget_radio(t.id);
                if t.kind.as_deref() == Some(cases::KIND) && self.empty_case(t.id)? {
                    self.say1(id, Line::col(Col::Dim, "The finds inside go back to whoever put them there."));
                }
                if t.kind.as_deref() == Some(builds::KIND) {
                    self.say1(id, Line::col(Col::Dim, format!("You take down {}, bit by bit, and the bits go back to the island.", the(&t.name))));
                    self.tell_here(x, y, id, Line::col(Col::Dim, format!("{who} takes down {}.", t.name)));
                } else {
                    self.say1(id, Line::col(Col::Dim, format!("{} fades gently away.", capitalise(&the(&t.name)))));
                    self.tell_here(x, y, id, Line::col(Col::Dim, format!("{who} unmakes {}.", t.name)));
                }
                self.say1(id, Line::col(Col::Dim, "(changed your mind? type undo)"));
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
        if self.world.things[i].is_fixed() {
            self.say1(id, Line::plain("That won't budge. It belongs right here."));
            return Ok(());
        }
        if !self.may_change(&self.user(id), i) {
            self.say1(id, self.not_yours(i));
            return Ok(());
        }
        let name = self.world.things[i].name.clone();
        if gramophone::makes_sound(self.world.things[i].kind.as_deref())
            && let Some(other) = self.sound_too_close(x + dx, y + dy, Some(self.world.things[i].id)) {
                self.say1(id, Line::plain(format!("That would put it too close to {}: two songs at once would be a muddle.", the(&other))));
                return Ok(());
            }
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
        self.growing_tick();
        self.lighthouse_tick();
        self.play_tick();
        self.builds_tick();

        let season = Clock::season();
        if season != self.season {
            let was = self.season;
            self.season = season;
            self.play_season(was);
            for p in self.players.values() {
                let _ = p.tx.send(ToClient::Lines { lines: vec![Line::new().b(Col::Magenta, season.arrival())] });
            }
        }
        let festival = Clock::festival();
        if festival.is_some() != self.festival {
            self.festival = festival.is_some();
            let line = match festival {
                Some(f) => Line::new().b(Col::Yellow, format!("{} {}", f.icon, f.greeting)),
                None => Line::col(Col::Dim, "The festival is over. What a day! ✨"),
            };
            for p in self.players.values() {
                let _ = p.tx.send(ToClient::Lines { lines: vec![line.clone()] });
            }
        }
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
                if let Some(msg) = sky_change(p.sky, sky, Clock::season()) {
                    self.say1(id, Line::col(Col::Blue, msg));
                }
                if let Some(p) = self.players.get_mut(&id) {
                    p.sky = sky;
                }
            } else if p.ambient_quiet < AMBIENT_WHILE_IDLE && self.rng.chance(1.0 / 14.0) {
                let terrain = self.world.terrain(x, y);
                let last = p.last_ambient;
                let hint = if self.rng.chance(0.3) { self.hint(id, sky) } else { None };
                if let Some(line) = hint.or_else(|| ambient(&mut self.rng, terrain, sky, phase, Clock::season())).filter(|l| Some(*l) != last) {
                    self.say1(id, Line::col(Col::Dim, line));
                    if let Some(p) = self.players.get_mut(&id) {
                        p.last_ambient = Some(line);
                        p.ambient_quiet += 1;
                    }
                }
            }
        }

        self.nature_tick();
        self.events_tick();
        self.animals_tick();

        // Things doing their things, but only when someone's there to see.
        let mut acts: Vec<(i64, i64, i64, String)> = Vec::new();
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
                acts.push((t.id, t.x, t.y, sentence(&format!("{} {action}", the(&t.name)))));
            }
        }
        // Each thing at most every few minutes for each person, and like the
        // scenery, not on and on to someone who's stepped away.
        for (tid, x, y, text) in acts {
            for p in self.players.values_mut().filter(|p| p.x == x && p.y == y && !p.bot) {
                let recent = p.seen_acts.get(&tid).is_some_and(|t| t.elapsed() < ACT_EVERY);
                if recent || p.ambient_quiet >= AMBIENT_WHILE_IDLE {
                    continue;
                }
                p.seen_acts.insert(tid, std::time::Instant::now());
                p.ambient_quiet += 1;
                let _ = p.tx.send(ToClient::Lines { lines: vec![Line::col(Col::Yellow, text.clone())] });
            }
        }

        self.status_all();
    }
}

/// `say hi`, `'hi` or `"hi`, with something actually said.
fn said_something(input: &str, verb: &str) -> bool {
    let text = if verb == "say" {
        input.split_once(char::is_whitespace).map_or("", |(_, r)| r)
    } else {
        input.strip_prefix(SAY_QUOTES).unwrap_or_default()
    };
    !text.trim().is_empty()
}

fn sky_change(from: Sky, to: Sky, season: Season) -> Option<&'static str> {
    if season == Season::Winter {
        return Some(match (from, to) {
            (_, Sky::Rain) if from != Sky::Drizzle => "Snow starts to fall, thick and soft. ❄",
            (Sky::Drizzle, Sky::Rain) => "The flurries thicken into proper snow. ❄",
            (_, Sky::Drizzle) if from == Sky::Rain => "The snow thins to a few drifting flakes.",
            (_, Sky::Drizzle) => "A few snowflakes come drifting down. ❄",
            (Sky::Rain | Sky::Drizzle, Sky::Cloudy) => "The snow stops. Everything is white and hushed.",
            (Sky::Clear, Sky::Cloudy) => "Heavy grey clouds roll in. It smells like snow.",
            (_, Sky::Clear) => "The clouds part, and the frost sparkles in the light.",
            _ => return None,
        });
    }
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

/// How often one made thing does its thing for one person.
const ACT_EVERY: Duration = Duration::from_secs(180);

/// Scenery lines someone gets while not doing anything, before it goes quiet.
const AMBIENT_WHILE_IDLE: u8 = 2;

fn ambient(rng: &mut Rng, terrain: Terrain, sky: Sky, phase: Phase, season: Season) -> Option<&'static str> {
    use Terrain::*;
    let night = phase == Phase::Night;
    if season == Season::Winter && sky.wet() {
        return Some(*rng.pick(&[
            "Snowflakes settle on your shoulders.",
            "Your footsteps crunch in the fresh snow.",
            "The whole world is hushed under the snow.",
        ]));
    }
    if rng.chance(0.35) {
        let seasonal: &[&str] = match (season, terrain) {
            (Season::Spring, Meadow | Grass) => &["Blossom petals drift by on the breeze.", "A blackbird is singing its heart out."],
            (Season::Spring, Forest) => &["Fresh green leaves are unfurling all around.", "Bluebells nod in the shade."],
            (Season::Summer, Sand | Water) => &["The water glitters in the summer sun.", "Warm sand shifts under your feet."],
            (Season::Summer, _) => &["The air hums with summer.", "A dragonfly hangs in the warm air, then darts away."],
            (Season::Autumn, Forest) => &["A leaf spins down past your nose.", "Somewhere, a squirrel is busily burying acorns."],
            (Season::Autumn, _) => &["A cool wind sends leaves skittering past.", "There's woodsmoke on the breeze."],
            (Season::Winter, _) => &["Your breath puffs out in little clouds.", "A robin hops along, bright against the frost."],
            _ => &[],
        };
        if !seasonal.is_empty() {
            return Some(*rng.pick(seasonal));
        }
    }
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
        (Hill | Stone, _, true) => &[
            "The whole sky is wheeling with stars up here.",
            "A shooting star streaks across the dark and is gone.",
            "The wind is cool and quiet over the rocks tonight.",
            "Far below, the sea glints silver under the moon.",
        ],
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
/// "3:12am Fri in London": the time somewhere, and where (from the zone's name).
fn time_there(tz: chrono_tz::Tz) -> String {
    let now = chrono::Utc::now().with_timezone(&tz);
    let city = tz.name().rsplit('/').next().unwrap_or("").replace('_', " ");
    let place = if city.is_empty() || city.eq_ignore_ascii_case("UTC") { now.format("%Z").to_string() } else { city };
    format!("{} {} in {place}", now.format("%-I:%M%P"), now.format("%a"))
}

/// The world's own timezone (TZ, or the machine's), for anyone who hasn't
/// said theirs.
fn world_zone() -> chrono_tz::Tz {
    crate::identity::local_zone().and_then(|t| t.parse().ok()).unwrap_or(chrono_tz::UTC)
}

/// `go event` or `go to event 2`, but not `go to Eventide Cove`.
fn is_go_event(rest: &str) -> bool {
    let r = rest.trim().trim_start_matches("to ").trim();
    r == "event" || r.strip_prefix("event ").is_some_and(|n| n.trim().parse::<usize>().is_ok())
}

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

/// Help, a topic at a time: `help` gives the essentials and the topics,
/// `help music` one topic, `help all` the lot.
struct HelpTopic {
    name: &'static str,
    also: &'static [&'static str],
    title: &'static str,
    keepers: bool,
    /// On the help front page. The rest still answer if asked (and in
    /// help all), but are mostly there to be stumbled on.
    listed: bool,
    items: &'static [(&'static str, &'static str)],
    footer: Option<&'static str>,
}

const HELP: &[HelpTopic] = &[
    HelpTopic {
        name: "moving",
        also: &["move", "walking", "walk", "travel", "sailing", "sail", "map"],
        title: "🧭 getting about",
        keepers: false,
        listed: false,
        items: &[
            ("n s e w ne nw se sw", "walk about"),
            ("look / look <thing>", "take it all in"),
            ("map", "the whole island, with friends and named places"),
            ("who", "who's out in the world, and what time it is for them"),
            ("time / time Corey", "the real time where you are, or where they are (even if they're away)"),
            ("find Poppy", "which way to walk to them"),
            ("join Poppy / stop", "walk to them automatically"),
            ("go dock / sail Driftwood", "walk to the dock ⚓, then row to another island"),
            ("sail to other.world", "visit another world (their keepers let you in)"),
            ("events / go event", "what's coming up on the island, and walk there"),
            ("places / go to The Greenhouse", "every named spot on every island, and walk there (rowing across if need be; go 3 works too)"),
            ("event here fri 4pm Tea party", "plan an event just for this place: only whoever's going, a board's followers here and people nearby hear about it"),
            ("event here fridays 5pm Frog choir", "a repeating one (also weekdays, daily, every mon wed)"),
            ("add Poppy, Sam to event 2", "sign people up to a 📍 event (its host, or anyone going); remove Poppy from event 2 for the host"),
            ("going 2 / not going 2", "say you'll come to a 📍 event, so you're reminded wherever you are (for a repeating one, not going skips just the next)"),
            ("leave event 2", "drop out of a repeating event for good"),
            ("meet", "get the links for the gathering here (at a gathering table, or an event that has some)"),
        ],
        footer: Some("There are four islands: sail between them from the docks. A whole day passes in 24 minutes."),
    },
    HelpTopic {
        name: "talking",
        also: &["talk", "chat", "people", "friends"],
        title: "💬 talking",
        keepers: false,
        listed: false,
        items: &[
            ("say hi  (or 'hi)", "talk to people here"),
            ("emote waves  (or :waves)", "do a little something"),
            ("sit / rest / dance / wave", "a little bonhomie"),
            ("@Poppy hi  (or tell)", "talk to someone anywhere (kept for them if they're away)"),
            ("reply thanks!  (or r)", "answer whoever told you"),
            ("chat hi everyone  (@all)", "everyone in the world hears"),
            ("shout hello", "carries a long way"),
            ("nickname Pip", "change what you're called (your username stays)"),
            ("link me", "a magic link to sign in on another device of yours"),
            ("mute Sam / unmute Sam / mutes", "stop hearing from someone (they aren't told)"),
            ("ask How many ducks are on the pond?", "everyone here answers in secret (answer 7), and the answers all come out together"),
        ],
        footer: None,
    },
    HelpTopic {
        name: "music",
        also: &["sound", "songs", "radio", "gramophone", "tunes"],
        title: "🎶 music",
        keepers: false,
        listed: true,
        items: &[
            ("hum C E G", "a little tune (hum together to jam!)"),
            ("make a music box / teach C E G / play", "a box that remembers your tune"),
            ("make a gramophone / play music / stop music", "play songs from your computer to everyone nearby"),
            ("make a jukebox / add songs / jukebox", "everyone adds songs from their computer; it plays one each, taking turns"),
            ("skip / unqueue", "skip your song (keepers: any), or take yours out of the queue"),
            ("make a radio / tune radio <station> / radio off", "play one of the island's radio stations for everyone nearby"),
            ("stations", "the island's radio stations (keepers choose them)"),
        ],
        footer: Some("Turn the sound on (🔊 in the browser) to hear them. Gramophones and radios carry five steps, and you only ever hear the nearest."),
    },
    HelpTopic {
        name: "land",
        also: &["shaping", "shape", "digging", "dig", "planting", "garden", "gardening"],
        title: "🌱 shaping the land",
        keepers: false,
        listed: false,
        items: &[
            ("dig / fill", "ponds, earth, sandbanks (and chip rock down to hills)"),
            ("plant tree|flowers|grass", "make things grow (slowly: trees over days, flowers over hours)"),
            ("plant apple tree / plant bluebells", "choose: oak birch willow apple cherry pine rowan, bluebells poppies sunflowers lavender foxgloves"),
            ("raise / path", "hills, rocks, paths"),
            ("name The Willow Bank", "name this place"),
        ],
        footer: None,
    },
    HelpTopic {
        name: "foraging",
        also: &["forage", "gather", "picking", "fishing", "fish", "satchel", "journal", "crafting", "craft"],
        title: "🧺 foraging and crafting",
        keepers: false,
        listed: false,
        items: &[
            ("gather  (or pick, catch)", "take what's here: look for pink • on the map"),
            ("fish", "by the water, then wait for a bite"),
            ("satchel / journal", "what you carry / everything you've found"),
            ("give daisy to Poppy", "share with someone here"),
            ("place daisy / release moth", "decorate, or set something free"),
            ("recipes", "what you can make from what you've gathered"),
            ("mend lighthouse", "help everyone rebuild the old lighthouse on Driftwood Isle"),
            ("craft wind chime", "make it (then place it, or give it away)"),
        ],
        footer: None,
    },
    HelpTopic {
        name: "cooking",
        also: &["cook", "food", "kitchen", "eat", "baking", "bake"],
        title: "🍳 cooking",
        keepers: false,
        listed: false,
        items: &[
            ("make a campfire / make a clay oven", "somewhere to cook, wherever you like"),
            ("cook / cook mint tea", "what you could make, then make it (a few moments)"),
            ("eat stew / serve crumble", "enjoy it, or share with everyone here"),
        ],
        footer: Some("Herbs and honey turn up as you wander. While it cooks, the smell drifts to everyone nearby."),
    },
    HelpTopic {
        name: "buildings",
        also: &["building", "blueprints", "houses", "house", "demolish"],
        title: "🏗️ buildings",
        keepers: false,
        listed: false,
        items: &[
            ("build", "what you could build, and whether your satchel has enough"),
            ("build a mushroom house", "put one up right here: it takes a few minutes, with a bar in your side panel"),
            ("build a turret", "some things only go next door to a building, making it bigger"),
            ("demolish mushroom house", "take down something you built (undo brings it back)"),
        ],
        footer: Some("Everything's built from what you gather: four or five things at most."),
    },
    HelpTopic {
        name: "animals",
        also: &["companions", "companion", "pets", "pet"],
        title: "🦊 animal friends",
        keepers: false,
        listed: false,
        items: &[
            ("befriend fox", "offer a wild animal a berry (it may take a few)"),
            ("petname Rusty / pet / let go", "your friend follows you everywhere"),
        ],
        footer: None,
    },
    HelpTopic {
        name: "making",
        also: &["make", "build", "things"],
        title: "✨ making things",
        keepers: false,
        listed: false,
        items: &[
            ("make a frog that croaks when it rains", "also: at night, during the day, when it's sunny, when someone arrives"),
            ("make a gathering table / meet https://…", "a spot that holds links for gatherings there (up to three); meet hands them over (meet off clears them)"),
            ("push bench north", "move something"),
            ("unmake bench / undo", "take away something you made, or bring it back"),
        ],
        footer: None,
    },
    HelpTopic {
        name: "home",
        also: &["homes", "claim"],
        title: "🏡 home",
        keepers: false,
        listed: false,
        items: &[
            ("claim", "make the patch around you your home (one each)"),
            ("home  (or go home)", "walk home from anywhere, rowing across if need be"),
            ("share Lu / unshare Lu / guests", "who else may build at your home"),
            ("describe A cosy den, fairy lights…", "in your own words, what people see here when they look (describe alone clears it)"),
            ("go in / out", "step inside a finished building (yours or a friend's): a room to describe, furnish and paint, rain or shine"),
            ("make a jar / put firefly in jar", "show off your finds at home (a display case, a shelf, a fish tank, a bug hotel…); take firefly from jar brings one back"),
            ("paint the cottage pink / paint home lilac", "any colour you like, for things you've made or your whole home (it shows on the map); paint … plain scrubs it off"),
        ],
        footer: None,
    },
    HelpTopic {
        name: "gatherings",
        also: &["gathering", "meetings", "meeting", "meet", "events", "event", "group", "groups", "team", "get-togethers", "together"],
        title: "🫖 gatherings: a spot for your group",
        keepers: false,
        listed: true,
        items: &[
            ("go to the village green", "everyone's spot: a village noticeboard, and a long table anyone can leave links on (or make your own, below)"),
            ("name The Frog Pond Gazebo", "1. find a spot and name it, so it's easy to find (places, go to)"),
            ("make a gathering table", "2. somewhere to gather (a gazebo works too)"),
            ("meet https://… https://…", "3. leave links there for whatever you get up to (up to three)"),
            ("event here weekdays 7am Sunrise stretch", "4. plan it at the table (or a one-off: event here fri 4pm Tea party)"),
            ("add Poppy, Sam to event 1", "5. bring your people: they hear about it from anywhere, with the links"),
            ("events", "what's coming up, who's going, and the links (go event 1 walks there, meet hands over the links)"),
            ("not going 1 / leave event 1", "skip just the next one of a repeating event, or drop out for good"),
            ("remove Poppy from event 1", "take someone off (the host or a keeper)"),
            ("make a noticeboard", "optional: a board on the same spot is a chat for the group, and its followers hear about everything planned there"),
        ],
        footer: Some("Times are your own, and everyone sees them in theirs. cancel event 1 stops one (or a whole repeating series)."),
    },
    HelpTopic {
        name: "boards",
        also: &["noticeboards", "noticeboard", "notes", "board"],
        title: "📌 noticeboards",
        keepers: false,
        listed: true,
        items: &[
            ("make a noticeboard", "put one up for everyone"),
            ("read / read more", "the notes here, then older ones"),
            ("post see you at sunset!", "leave a note (and follow the board)"),
            ("boards / follow / unfollow", "boards you hear about from anywhere (unfollow 2 leaves one from anywhere)"),
            ("add Poppy, Sam to board", "they follow this board now, like adding them to a group chat (anyone who follows it can)"),
            ("archive / read week 2026-11-02", "past pages of the Island Journal"),
            ("hook Doorbell / hooks / unhook Doorbell", "a secret address your gadgets (a doorbell, a kettle…) can post notes to (the board's maker or a keeper)"),
            ("progress Pizza / clear Pizza", "a secret address a gadget posts 37/100 to: a progress bar in the side panel of everyone following or nearby"),
            ("status Surf / clear Surf", "one a gadget posts up or down to: a light in the same side panels, and a message when it goes down or comes back up"),
        ],
        footer: None,
    },
    HelpTopic {
        name: "keepers",
        also: &["keeper", "admin"],
        title: "🗝 island keepers",
        keepers: true,
        listed: true,
        items: &[
            ("invite Poppy", "make an account for someone new, and get their magic link"),
            ("link Poppy", "a new magic link for someone's new device (anyone can: link me)"),
            ("invites / revoke Sam", "see recent magic links, or cancel someone's unused ones"),
            ("users", "everyone, with their devices"),
            ("kick Sam [minutes] / unkick Sam", "send someone off for a break (30 min)"),
            ("ban Sam / unban Sam", "close the island to someone"),
            ("promote Lu / demote Lu", "make someone a keeper, or not"),
            ("signout Sam", "sign out all their devices (a lost iPad)"),
            ("fold Poppy into sunny-otter-42", "move an old account's things into someone's account"),
            ("visitors / admit / turn away", "people sailing in from other worlds"),
            ("trust their.address / untrust / worlds", "let a friendly world's people straight in"),
            ("event sat 4pm Beach party", "plan an event for the whole island, right here (cancel event 1)"),
            ("event fri 4pm Tea party https://…", "an event with links (at a gathering table, it uses the table's)"),
            ("station add Night Owl FM https://… [guide https://…]", "add a radio station anyone can tune to (station remove)"),
        ],
        footer: None,
    },
];

fn help_item(cmd: &str, what: &str) -> Line {
    // No column padding, so it reads fine on a narrow phone screen too.
    let line = Line::new().c(Col::Yellow, format!("  {cmd}"));
    if what.is_empty() { line } else { line.c(Col::Dim, format!("  {what}")) }
}

fn help_topic(t: &HelpTopic) -> Vec<Line> {
    let mut lines = vec![Line::col(Col::Pink, t.title)];
    lines.extend(t.items.iter().map(|(c, w)| help_item(c, w)));
    if let Some(f) = t.footer {
        lines.push(Line::col(Col::Dim, format!("  {f}")));
    }
    lines
}

fn help(topic: &str, keeper: bool) -> Vec<Line> {
    let topic = topic.trim().to_lowercase();
    let topics = || HELP.iter().filter(|t| keeper || !t.keepers);
    if topic == "all" {
        let mut lines = vec![Line::new().b(Col::Magenta, "✨ Everything you can do ✨")];
        for t in topics() {
            lines.extend(help_topic(t));
        }
        return lines;
    }
    if !topic.is_empty()
        && let Some(t) = topics().find(|t| t.name == topic || t.also.contains(&topic.as_str())) {
            return help_topic(t);
        }
    let mut lines = Vec::new();
    if topic.is_empty() {
        lines.push(Line::new().b(Col::Magenta, "✨ Things you can do ✨"));
        lines.extend(
            [
                ("n s e w  (and ne nw se sw)", "walk about"),
                ("look / map", "take it all in, or the whole island"),
                ("say hi  (or 'hi)", "talk to people here"),
                ("@Poppy hi / who", "talk to anyone, anywhere, and see who's about"),
                ("gather / satchel", "pick up what you find (pink • on the map), and see what you carry"),
                ("make a frog that croaks when it rains", "make anything you can imagine"),
                ("claim / home", "make a spot your home, and wander back to it"),
                ("mute Sam", "stop hearing from someone (they aren't told)"),
            ]
            .iter()
            .map(|(c, w)| help_item(c, w)),
        );
        lines.push(Line::col(Col::Dim, "  ↑ for things you've typed before, Ctrl-R to search them (browser)"));
        lines.push(Line::col(Col::Pink, "  …and lots more to stumble on. Poke around, try things, see what happens. 🌿"));
    } else {
        lines.push(Line::plain(format!("There's no help on '{topic}'. Try one of these:")));
    }
    lines.push(Line::col(Col::Pink, "more help"));
    for t in topics().filter(|t| t.listed) {
        let blurb = t.title.split_once(' ').map_or(t.title, |(_, rest)| rest);
        lines.push(help_item(&format!("help {}", t.name), blurb));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_comes_a_topic_at_a_time() {
        let text = |lines: Vec<Line>| lines.iter().map(|l| l.0.iter().map(|s| s.t.as_str()).collect::<String>()).collect::<Vec<_>>().join("\n");
        let short = text(help("", false));
        assert!(short.contains("help music") && short.contains("help gatherings") && short.contains("mute"));
        assert!(!short.contains("help cooking") && !short.contains("help all"), "the rest are for stumbling on");
        assert!(text(help("cooking", false)).contains("make a campfire"), "but still answer if asked");
        assert!(!short.contains("help keepers"), "keepers' topic only for keepers");
        assert!(text(help("", true)).contains("help keepers"));
        assert!(text(help("radio", false)).contains("tune radio"));
        assert!(text(help("MUSIC", false)).contains("make a gramophone"));
        assert!(text(help("nonsense", false)).contains("no help on 'nonsense'"));
        assert!(!text(help("keepers", false)).contains("invite Poppy"));
        assert!(text(help("all", true)).contains("invite Poppy") && text(help("all", false)).contains("befriend fox"));
        // Every name and alias leads to just one topic.
        let mut seen = std::collections::HashSet::new();
        for t in HELP {
            for n in std::iter::once(&t.name).chain(t.also) {
                assert!(seen.insert(*n), "'{n}' is used twice");
            }
        }
    }

    #[test]
    fn go_event_isnt_a_place_starting_event() {
        assert!(is_go_event("event") && is_go_event("to event 2") && is_go_event("event 3"));
        assert!(!is_go_event("to Eventide Cove") && !is_go_event("eventide"));
    }

    #[test]
    fn says_whose_time_where() {
        assert!(time_there(chrono_tz::America::New_York).ends_with(" in New York"));
        assert!(time_there(chrono_tz::Europe::London).ends_with(" in London"));
        assert!(time_there(chrono_tz::UTC).ends_with(" in UTC"));
    }

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
