//! Little bits of nonsense, for the fun of it: messages in bottles,
//! sandcastles the tide takes, kites, a cheeky seagull, silly hats, puddles,
//! snow angels, snowmen and skimming stones. None of it is in the help: it's
//! there to be stumbled on, with the odd hint in the scenery (see `hint`). And none of it
//! chatters at someone who's stepped away.

use super::{AMBIENT_WHILE_IDLE, Game};
use crate::protocol::{Col, Line, ToClient};
use crate::server::nature::{self, Kind};
use crate::server::terrain::Terrain;
use crate::server::weather::{Season, Sky};
use crate::server::world::DIRS;
use std::time::{Duration, Instant};

/// How long a note in a bottle may be.
const BOTTLE_MAX: usize = 200;
/// How far off a washed-up bottle catches your eye.
const GLINT: i64 = 3;
/// Bottles wash up somewhere at least this far from where they were thrown.
const BOTTLE_AWAY: i64 = 12;
/// How long a bottle's at sea, in seconds: a couple of hours to a day and a bit.
const AT_SEA: (f64, f64) = (2.0 * 3600.0, 30.0 * 3600.0);
/// How long between tides, in seconds.
const TIDES: (f64, f64) = (2.0 * 3600.0, 4.0 * 3600.0);
/// The least wind a kite flies in.
const KITE_WIND: f64 = 0.2;
/// How far off you can see someone's kite.
const KITE_SEEN: i64 = 4;
/// How long a seagull circles before it swoops.
const GULL_CIRCLES: Duration = Duration::from_secs(15);
/// The least time between one person's seagulls.
const GULL_EVERY: Duration = Duration::from_secs(40 * 60);
/// What someone's wearing, at most.
const WEAR_MAX: usize = 40;
/// Sandcastles and snowmen only get so grand.
const MOST_BITS: usize = 6;

/// One person's bits of fun.
#[derive(Default)]
pub struct Fun {
    pub kite: Option<Kite>,
    /// A seagull's circling, since then.
    gull: Option<Instant>,
    gull_last: Option<Instant>,
    pub wearing: Option<String>,
    /// Hints already given this visit, so none comes round twice.
    hinted: std::collections::HashSet<&'static str>,
}

impl Fun {
    pub fn wearing(wearing: Option<String>) -> Fun {
        Fun {
            wearing,
            ..Default::default()
        }
    }
}

pub struct Kite {
    colour: &'static str,
    pub(super) at: (i64, i64),
    tangled: bool,
}

/// Something built on a spot by one person or more.
#[derive(Default)]
pub struct Built {
    builders: Vec<String>,
    bits: Vec<String>,
}

impl Built {
    fn add(&mut self, who: &str, bit: &str) {
        if !self.builders.iter().any(|b| b == who) {
            self.builders.push(who.to_string());
        }
        self.bits.push(bit.to_string());
    }
}

pub struct Angels {
    who: Vec<String>,
    since: Instant,
}

const CASTLE_BITS: &[&str] = &[
    "a moat with a seaweed drawbridge",
    "three wobbly turrets",
    "a shell for a door",
    "a gull feather flag on top",
    "a little tower with a pebble window",
    "a winding staircase drawn with a finger",
    "a very grand gate",
    "a courtyard just the right size for a crab",
    "a keep that leans like it's had a long day",
];

const SNOWMAN_BITS: &[&str] = &[
    "a carrot nose",
    "two pebble eyes",
    "a stick for each arm",
    "a scarf of knotted seaweed",
    "a smile made of acorns",
    "a bucket for a hat",
    "three shell buttons",
    "a pine cone pipe",
];

const KITES: &[&str] = &[
    "red",
    "yellow",
    "rainbow-striped",
    "blue diamond",
    "fish-shaped",
    "dragon",
    "patchwork",
    "butterfly",
];

/// "a, b and c"
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn number(n: u64) -> &'static str {
    [
        "no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
        "eleven", "twelve",
    ]
    .get(n as usize)
    .copied()
    .unwrap_or("lots of")
}

/// How a skimmed stone goes, in words, never a score.
pub fn skim(roll: u64, flourish: &str) -> String {
    match roll % 100 {
        0..10 => "It goes plop. Straight down. The water barely notices.".into(),
        10..20 => "It skips once, then sinks with great dignity.".into(),
        95.. => {
            "It skips so far you lose sight of it. Somewhere out there, a fish applauds.".into()
        }
        r => {
            let n = 2 + (r / 10) % 9;
            let times = if n == 2 {
                "twice".to_string()
            } else {
                format!("{} times", number(n))
            };
            if r % 2 == 0 {
                format!("It skips {times}, and {flourish}.")
            } else {
                format!("It skips {times}.")
            }
        }
    }
}

/// "a daisy chain", "sunglasses", "my lucky hat" → "a lucky hat"
pub fn wearable(words: &str) -> Option<String> {
    let w: String = words
        .trim()
        .trim_start_matches("my ")
        .trim_start_matches("on ")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    let w = w.trim();
    if w.is_empty() || w.chars().count() > WEAR_MAX {
        return None;
    }
    let first = w.split_whitespace().next().unwrap_or("").to_lowercase();
    let has_article = [
        "a", "an", "the", "some", "two", "three", "one", "her", "his", "their",
    ]
    .contains(&first.as_str());
    Some(if has_article || w.ends_with('s') {
        w.to_string()
    } else {
        nature::a(w)
    })
}

/// Is the bit before the words a bottle? "bottle hi", "throw a bottle hi"
fn bottle_words(rest: &str) -> &str {
    let r = rest.trim();
    for pre in ["a bottle", "bottle", "in a bottle"] {
        if let Some(after) = r.strip_prefix(pre) {
            return after.trim_start_matches([':', '-', ',']).trim();
        }
    }
    r
}

impl Game {
    /// The fun commands, if this is one. They come before building, so a
    /// sandcastle is anyone's to make anywhere there's sand.
    pub(super) fn play_command(
        &mut self,
        id: u64,
        verb: &str,
        rest: &str,
    ) -> Option<anyhow::Result<()>> {
        let r = rest.to_lowercase();
        let has = |w: &str| r.contains(w);
        Some(match verb {
            "make" | "build" | "create" if has("sandcastle") || has("sand castle") => {
                self.cmd_sandcastle(id)
            }
            "make" | "build" | "create"
                if has("snowman") || has("snow man") || has("snowwoman") || has("snowperson") =>
            {
                self.cmd_snowman(id)
            }
            "make" | "create" | "do" if has("snow angel") => self.cmd_snow_angel(id),
            "fly" | "kite" if has("kite") || verb == "kite" => self.cmd_kite(id),
            "reel" | "wind" if has("in") || has("kite") => {
                match self.players.get(&id).is_some_and(|p| p.fun.kite.is_some()) {
                    true => self.cmd_reel_in(id),
                    false => return None,
                }
            }
            "untangle" | "rescue" if verb == "untangle" || has("kite") => self.cmd_untangle(id),
            "shoo" | "scare" => self.cmd_shoo(id),
            "jump" | "splash" | "leap" | "stomp" | "puddle" => self.cmd_splash(id),
            "skim" => self.cmd_skim(id),
            "skip" if has("stone") || has("pebble") || has("rock") => self.cmd_skim(id),
            "bottle" => self.cmd_bottle(id, bottle_words(rest)),
            "throw" | "send" | "message" if has("bottle") => {
                self.cmd_bottle(id, bottle_words(rest.trim_start_matches("in a bottle")))
            }
            "open" | "read" | "uncork" if has("bottle") => self.cmd_open_bottle(id),
            "wear" | "don" => self.cmd_wear(id, Some(rest)),
            "put" if r.starts_with("on ") => self.cmd_wear(id, Some(rest)),
            "take" if r.starts_with("off") => self.cmd_wear(id, None),
            "unwear" | "undress" => self.cmd_wear(id, None),
            "look" | "l" | "examine" | "x" if !r.is_empty() => match self.person_here(id, &r) {
                Some(other) => {
                    self.cmd_look_at(id, other);
                    Ok(())
                }
                None => return None,
            },
            _ => return None,
        })
    }

    // ---- where things are --------------------------------------------

    fn sea_by(&self, x: i64, y: i64) -> bool {
        std::iter::once((0, 0))
            .chain(DIRS.iter().map(|d| (d.2, d.3)))
            .any(|(dx, dy)| {
                matches!(
                    self.world.terrain(x + dx, y + dy),
                    Terrain::Water | Terrain::DeepWater
                )
            })
    }

    /// Water here or beside, and whether it's a pond.
    fn water_by(&self, x: i64, y: i64) -> Option<bool> {
        let around: Vec<Terrain> = std::iter::once((0, 0))
            .chain(DIRS.iter().map(|d| (d.2, d.3)))
            .map(|(dx, dy)| self.world.terrain(x + dx, y + dy))
            .collect();
        if around.contains(&Terrain::Pond) {
            Some(true)
        } else if around.iter().any(|t| t.is_wet()) {
            Some(false)
        } else {
            None
        }
    }

    fn trees_by(&self, x: i64, y: i64) -> bool {
        std::iter::once((0, 0))
            .chain(DIRS.iter().map(|d| (d.2, d.3)))
            .any(|(dx, dy)| self.world.terrain(x + dx, y + dy) == Terrain::Forest)
    }

    fn person_here(&self, id: u64, name: &str) -> Option<u64> {
        let me = self.players.get(&id)?;
        let name = name.trim().trim_start_matches("at ").trim();
        self.players
            .iter()
            .find(|(pid, p)| {
                **pid != id
                    && (p.x, p.y) == (me.x, me.y)
                    && (p.name.eq_ignore_ascii_case(name) || p.user.eq_ignore_ascii_case(name))
            })
            .map(|(pid, _)| *pid)
    }

    /// Everyone at (x, y) except `except` who's about (not stepped away),
    /// for things that would otherwise chatter on.
    fn tell_about(&mut self, x: i64, y: i64, except: u64, line: &Line) {
        for (&pid, p) in self.players.iter_mut() {
            if pid != except
                && (p.x, p.y) == (x, y)
                && !p.bot
                && p.ambient_quiet < AMBIENT_WHILE_IDLE
            {
                p.ambient_quiet += 1;
                let _ = p.tx.send(ToClient::Lines {
                    lines: vec![line.clone()],
                });
            }
        }
    }

    // ---- looking ------------------------------------------------------

    /// For `look`: whatever fun is going on here.
    pub(super) fn play_lines_here(&self, id: u64) -> Vec<Line> {
        let Some(me) = self.players.get(&id) else {
            return vec![];
        };
        let (x, y) = (me.x, me.y);
        let mut lines = Vec::new();
        for p in self.players.values() {
            if p.user != me.user
                && (p.x, p.y) == (x, y)
                && let Some(w) = &p.fun.wearing
            {
                lines.push(Line::col(
                    Col::Magenta,
                    format!("✨ {} is wearing {w}.", p.name),
                ));
            }
        }
        if let Some(c) = self.castles.get(&(x, y)) {
            lines.push(Line::col(
                Col::Yellow,
                format!(
                    "🏰 A sandcastle stands here, built by {}: {}.",
                    self.names(&c.builders),
                    and_list(&c.bits)
                ),
            ));
        }
        if let Some(s) = self.snowmen.get(&(x, y)) {
            lines.push(Line::col(
                Col::White,
                format!(
                    "⛄ A snowman stands here, made by {}, with {}.",
                    self.names(&s.builders),
                    and_list(&s.bits)
                ),
            ));
        }
        if let Some(a) = self.angels.get(&(x, y)) {
            let whose = self.names(&a.who);
            let line = if a.who.len() == 1 {
                format!("👼 A snow angel ({whose}'s) is pressed into the snow here.")
            } else {
                format!(
                    "👼 {} snow angels lie side by side here ({whose}).",
                    capital(number(a.who.len() as u64))
                )
            };
            lines.push(Line::col(Col::White, line));
        }
        for p in self.players.values() {
            let Some(k) = &p.fun.kite else { continue };
            if (p.x - x).abs().max((p.y - y).abs()) > KITE_SEEN {
                continue;
            }
            let whose = if p.user == me.user {
                "Your".to_string()
            } else {
                format!("{}'s", p.name)
            };
            let line = if k.tangled {
                format!(
                    "🪁 {whose} {} kite is stuck in a tree! (untangle)",
                    k.colour
                )
            } else {
                format!("🪁 {whose} {} kite dances high overhead.", k.colour)
            };
            lines.push(Line::col(Col::Cyan, line));
        }
        let now = super::boards::now();
        let bottles = self
            .world
            .db
            .bottles_near(x, y, GLINT, now)
            .unwrap_or_default();
        if let Some(b) = bottles.iter().find(|b| me.hears(&b.by)) {
            let line = if b.at == (x, y) {
                "🍾 A bottle has washed up here, with a rolled-up note inside. (open bottle)"
                    .to_string()
            } else {
                let (dx, dy) = ((b.at.0 - x).signum(), (b.at.1 - y).signum());
                let dir = DIRS
                    .iter()
                    .find(|d| (d.2, d.3) == (dx, dy))
                    .map_or("nearby", |d| d.1);
                format!("🍾 Something glints at the water's edge, a little way to the {dir}.")
            };
            lines.push(Line::col(Col::Green, line));
        }
        lines
    }

    fn names(&self, users: &[String]) -> String {
        and_list(&users.iter().map(|u| self.world.nick(u)).collect::<Vec<_>>())
    }

    fn cmd_look_at(&self, id: u64, other: u64) {
        let Some(p) = self.players.get(&other) else {
            return;
        };
        let mut line = format!("☺ {} is right here", p.name);
        if let Some(w) = &p.fun.wearing {
            line.push_str(&format!(", wearing {w}"));
        }
        if let Some(k) = &p.fun.kite {
            line.push_str(&format!(", holding the string of a {} kite", k.colour));
        }
        line.push('.');
        self.say1(id, Line::col(Col::Cyan, line));
    }

    // ---- the seaside -------------------------------------------------

    fn cmd_sandcastle(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        if self.world.terrain(x, y) != Terrain::Sand {
            self.say1(
                id,
                Line::plain("You need sand for a sandcastle. Head down to a beach!"),
            );
            return Ok(());
        }
        let user = self.user(id);
        let castle = self.castles.entry((x, y)).or_default();
        if castle.bits.len() >= MOST_BITS {
            self.say1(id, Line::plain("It's as grand as a sandcastle can get. One more turret and the whole thing topples."));
            return Ok(());
        }
        let left: Vec<&&str> = CASTLE_BITS
            .iter()
            .filter(|b| !castle.bits.iter().any(|c| c == **b))
            .collect();
        let bit: &str = left[(self.rng.next() % left.len() as u64) as usize];
        let first = castle.bits.is_empty();
        castle.add(&user, bit);
        let (mine, theirs) = if first {
            (
                format!(
                    "🏰 You pat and scoop and shape the damp sand into a sandcastle, with {bit}."
                ),
                format!("🏰 {name} builds a sandcastle, with {bit}."),
            )
        } else {
            (
                format!("🏰 You add {bit} to the sandcastle."),
                format!("🏰 {name} adds {bit} to the sandcastle."),
            )
        };
        self.say1(id, Line::col(Col::Yellow, mine));
        self.tell_here(x, y, id, Line::col(Col::Yellow, theirs));
        Ok(())
    }

    /// Someone who isn't a player (island folk) adds a bit to the sandcastle
    /// at (x, y), or starts one. The bit added, if there was room for one.
    pub(super) fn sandcastle_by(&mut self, x: i64, y: i64, who: &str) -> Option<&'static str> {
        let castle = self.castles.entry((x, y)).or_default();
        let left: Vec<&'static str> = CASTLE_BITS
            .iter()
            .copied()
            .filter(|b| !castle.bits.iter().any(|c| c == b))
            .collect();
        if castle.bits.len() >= MOST_BITS || left.is_empty() {
            return None;
        }
        let bit = left[(self.rng.next() % left.len() as u64) as usize];
        self.castles.entry((x, y)).or_default().add(who, bit);
        Some(bit)
    }

    fn cmd_skim(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let Some(pond) = self.water_by(x, y) else {
            self.say1(
                id,
                Line::plain("You need some water to skim a stone across. Find a pond, or the sea!"),
            );
            return Ok(());
        };
        let flourishes: &[&str] = if pond {
            &[
                "a duck looks deeply offended",
                "a frog harrumphs",
                "a heron pretends not to have noticed",
                "the ripples spread out in rings",
            ]
        } else {
            &[
                "a seal pops up to watch",
                "a gull looks deeply unimpressed",
                "a crab gives it a slow clap",
                "the ripples spread out in rings",
            ]
        };
        let flourish = *self.rng.pick(flourishes);
        let how = skim(self.rng.next(), flourish);
        self.say1(
            id,
            Line::col(
                Col::Blue,
                format!("🪨 You find a flat stone and send it skimming. {how}"),
            ),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(Col::Blue, format!("🪨 {name} skims a stone. {how}")),
        );
        Ok(())
    }

    fn cmd_bottle(&mut self, id: u64, words: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        if !self.sea_by(x, y) {
            self.say1(
                id,
                Line::plain("You need to be by the sea to send a bottle off. Find a beach!"),
            );
            return Ok(());
        }
        let words: String = words
            .chars()
            .filter(|c| !c.is_control())
            .take(BOTTLE_MAX)
            .collect();
        let words = words.trim();
        if words.is_empty() {
            self.say1(id, Line::plain("🍾 What will your note say? e.g. bottle To whoever finds this: have a lovely day!"));
            return Ok(());
        }
        let user = self.user(id);
        let now = super::boards::now();
        if self.world.db.bottle_afloat(&user, now)? {
            self.say1(id, Line::plain("Your last bottle is still bobbing about out at sea. Give it time to wash up somewhere!"));
            return Ok(());
        }
        let Some(to) = self.landing_spot(x, y) else {
            self.say1(
                id,
                Line::plain("The tide's coming in. Try again in a moment?"),
            );
            return Ok(());
        };
        let lands = now + self.rng.range(AT_SEA.0, AT_SEA.1) as i64;
        self.world
            .db
            .throw_bottle(&user, words, now, (x, y), lands, to)?;
        self.say1(id, Line::col(Col::Green, "🍾 You roll up your note, push in the cork and throw the bottle as far as you can. It bobs away on the waves. Who knows where it'll wash up?"));
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Green,
                format!("🍾 {name} throws a bottle out to sea. It bobs away on the waves."),
            ),
        );
        Ok(())
    }

    /// A beach somewhere else for a bottle to wash up on.
    fn landing_spot(&mut self, x: i64, y: i64) -> Option<(i64, i64)> {
        let size = crate::server::world::SIZE;
        let beaches: Vec<(i64, i64)> = (0..size)
            .flat_map(|by| (0..size).map(move |bx| (bx, by)))
            .filter(|&(bx, by)| (bx - x).abs().max((by - y).abs()) >= BOTTLE_AWAY)
            .filter(|&(bx, by)| self.world.terrain(bx, by) == Terrain::Sand && self.sea_by(bx, by))
            .collect();
        (!beaches.is_empty()).then(|| beaches[(self.rng.next() % beaches.len() as u64) as usize])
    }

    fn cmd_open_bottle(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let now = super::boards::now();
        let user = self.user(id);
        let hears =
            |b: &crate::server::db::Bottle| self.players.get(&id).is_some_and(|p| p.hears(&b.by));
        let Some(b) = self
            .world
            .db
            .bottles_near(x, y, 0, now)?
            .into_iter()
            .find(hears)
        else {
            self.say1(id, Line::plain("There's no bottle here. They wash up on beaches now and then, if you keep an eye out."));
            return Ok(());
        };
        self.world.db.find_bottle(b.id, &user, now)?;
        let from = crate::server::islands::island_at(b.from.0, b.from.1).name;
        let here = crate::server::islands::island_at(x, y).name;
        let when = crate::server::gardens::ago(now - b.thrown_at);
        let mut lines = vec![
            Line::col(
                Col::Green,
                "🍾 You pull out the cork and shake out the note. It says:",
            ),
            Line::col(Col::White, format!("   \"{}\"", b.text)),
        ];
        if b.by == user {
            lines.push(Line::col(Col::Dim, format!("   It's your own, thrown off {from} {when}, come back to you. The sea has a sense of humour.")));
            self.send(id, lines);
            return Ok(());
        }
        let thrower = self.world.nick(&b.by);
        lines.push(Line::col(
            Col::Dim,
            format!("   ({thrower} threw it off {from}, {when}.)"),
        ));
        self.send(id, lines);
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Green,
                format!("🍾 {name} uncorks a bottle that's washed up, and reads the note inside."),
            ),
        );
        let landed = if here == from {
            format!("further along the shore of {here}")
        } else {
            format!("on {here}")
        };
        match self.find_player(&b.by) {
            Some(tid) => self.say1(
                tid,
                Line::col(
                    Col::Green,
                    format!("🍾 {name} found the bottle you threw off {from}, washed up {landed}!"),
                ),
            ),
            None => self.world.db.send_mail(
                &b.by,
                &user,
                &format!("🍾 I found your message in a bottle, washed up {landed}!"),
                now,
            )?,
        }
        Ok(())
    }

    // ---- kites ---------------------------------------------------------

    fn cmd_kite(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        if self.players.get(&id).is_some_and(|p| p.fun.kite.is_some()) {
            self.say1(
                id,
                Line::plain(
                    "🪁 Your kite's already up there, swooping about. (reel in to bring it down)",
                ),
            );
            return Ok(());
        }
        if self.world.terrain(x, y) == Terrain::Forest {
            self.say1(
                id,
                Line::plain("Too many trees here for a kite. Find some open ground!"),
            );
            return Ok(());
        }
        if self.world.terrain(x, y).is_wet() {
            self.say1(id, Line::plain("Not while you're paddling. Find dry land!"));
            return Ok(());
        }
        if self.world.weather.speed() < KITE_WIND {
            self.say1(id, Line::plain("🪁 There's not a breath of wind. Your kite flops onto the ground. Try again when it's breezy!"));
            return Ok(());
        }
        let colour = *self.rng.pick(KITES);
        let up = if self.world.weather.breezy() {
            "The wind snatches it up and it climbs and climbs!"
        } else {
            "It wobbles, catches the breeze, and up it goes."
        };
        if let Some(p) = self.players.get_mut(&id) {
            p.fun.kite = Some(Kite {
                colour,
                at: (x, y),
                tangled: false,
            });
        }
        self.say1(
            id,
            Line::col(
                Col::Cyan,
                format!(
                    "🪁 You let out the string and throw your {colour} kite up into the wind. {up}"
                ),
            ),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(Col::Cyan, format!("🪁 {name} flies a {colour} kite. {up}")),
        );
        Ok(())
    }

    fn cmd_reel_in(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let Some(k) = self.players.get_mut(&id).and_then(|p| p.fun.kite.take()) else {
            return Ok(());
        };
        let msg = if k.tangled {
            "You let the string go. The tree can keep the kite for now."
        } else {
            "🪁 You wind in your kite, hand over hand, and tuck it under your arm."
        };
        self.say1(id, Line::col(Col::Cyan, msg));
        if !k.tangled {
            self.tell_here(
                x,
                y,
                id,
                Line::col(Col::Cyan, format!("🪁 {name} winds in their kite.")),
            );
        }
        Ok(())
    }

    /// Walking off brings the kite with you.
    pub(super) fn kite_walks(&mut self, id: u64) {
        let Some(k) = self.players.get_mut(&id).and_then(|p| p.fun.kite.take()) else {
            return;
        };
        let msg = if k.tangled {
            "You leave your kite to the tree. Maybe the birds will like it."
        } else {
            "🪁 You wind in your kite as you set off."
        };
        self.say1(id, Line::col(Col::Dim, msg));
    }

    fn cmd_untangle(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let stuck = self
            .players
            .iter()
            .filter(|(pid, p)| {
                **pid != id
                    && p.fun.kite.as_ref().is_some_and(|k| k.tangled)
                    && (p.x - x).abs().max((p.y - y).abs()) <= KITE_SEEN
            })
            .map(|(pid, p)| (*pid, p.name.clone()))
            .next();
        let Some((owner, owner_name)) = stuck else {
            let msg = if self
                .players
                .get(&id)
                .is_some_and(|p| p.fun.kite.as_ref().is_some_and(|k| k.tangled))
            {
                "You tug and tug, but it's well and truly stuck. Someone else could climb up for it. (or reel in to let it be)"
            } else {
                "There's no kite stuck in a tree around here."
            };
            self.say1(id, Line::plain(msg));
            return Ok(());
        };
        if let Some(k) = self
            .players
            .get_mut(&owner)
            .and_then(|p| p.fun.kite.as_mut())
        {
            k.tangled = false;
        }
        self.say1(id, Line::col(Col::Green, format!("🌳 You climb up and carefully untangle {owner_name}'s kite. It swoops back up into the wind!")));
        self.say1(
            owner,
            Line::col(
                Col::Green,
                format!("🌳 {name} climbs up and frees your kite! It swoops back into the sky. 🪁"),
            ),
        );
        Ok(())
    }

    // ---- seagulls ------------------------------------------------------

    fn cmd_shoo(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let gull = self.players.get_mut(&id).and_then(|p| p.fun.gull.take());
        if gull.is_none() {
            self.say1(id, Line::plain("There's no seagull about. (Yet.)"));
            return Ok(());
        }
        self.say1(id, Line::col(Col::Cyan, "🐦 You flap your arms and shout. The seagull squawks indignantly and wheels away over the waves."));
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Cyan,
                format!("🐦 {name} flaps their arms at a seagull. It squawks off in a huff."),
            ),
        );
        Ok(())
    }

    // ---- weather toys ---------------------------------------------------

    fn cmd_splash(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        if self.world.terrain(x, y).is_wet() {
            self.say1(
                id,
                Line::plain(
                    "You jump up and down in the water. You were already wet, to be fair. 💦",
                ),
            );
            return Ok(());
        }
        let sky = self.world.weather.sky_at(x, y);
        if !sky.wet() {
            self.say1(
                id,
                Line::plain("There are no puddles about. Wait for rain! ☔"),
            );
            return Ok(());
        }
        if self.season == Season::Winter {
            self.say1(
                id,
                Line::col(
                    Col::White,
                    "❄ You leap into a snowdrift and come up covered in snow from head to toe.",
                ),
            );
            self.tell_here(
                x,
                y,
                id,
                Line::col(
                    Col::White,
                    format!("❄ {name} leaps into a snowdrift. Snow everywhere!"),
                ),
            );
            return Ok(());
        }
        let mine = *self.rng.pick(&[
            "💦 You jump in a puddle with both feet. SPLOSH!",
            "💦 You find the biggest puddle around and stamp in it. Very satisfying.",
            "💦 You jump in a puddle. It's deeper than it looked. Your socks are soaked.",
            "💦 You hop from puddle to puddle, splash, splash, splash.",
        ]);
        self.say1(id, Line::col(Col::Blue, mine));
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Blue,
                format!("💦 {name} jumps in a puddle, and you get splashed!"),
            ),
        );
        Ok(())
    }

    fn cmd_snow_angel(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        if !self.snowy(x, y) {
            self.say1(
                id,
                Line::plain("There's no snow for that. Come back in winter! ❄"),
            );
            return Ok(());
        }
        let user = self.user(id);
        let a = self.angels.entry((x, y)).or_insert_with(|| Angels {
            who: vec![],
            since: Instant::now(),
        });
        if !a.who.contains(&user) {
            a.who.push(user);
        }
        a.since = Instant::now();
        self.say1(id, Line::col(Col::White, "👼 You flop backwards into the snow and sweep your arms and legs up and down. A snow angel!"));
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::White,
                format!("👼 {name} flops backwards into the snow and makes a snow angel."),
            ),
        );
        Ok(())
    }

    fn cmd_snowman(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        if !self.snowy(x, y) {
            self.say1(
                id,
                Line::plain("There's no snow for a snowman. Come back in winter! ❄"),
            );
            return Ok(());
        }
        let user = self.user(id);
        let s = self.snowmen.entry((x, y)).or_default();
        if s.bits.len() >= MOST_BITS {
            self.say1(
                id,
                Line::plain(
                    "This snowman is perfect already. Any more and it'd need a name and a job.",
                ),
            );
            return Ok(());
        }
        let left: Vec<&&str> = SNOWMAN_BITS
            .iter()
            .filter(|b| !s.bits.iter().any(|c| c == **b))
            .collect();
        let bit: &str = left[(self.rng.next() % left.len() as u64) as usize];
        let first = s.bits.is_empty();
        s.add(&user, bit);
        let (builders, bits) = (s.builders.join(","), s.bits.join("|"));
        self.world
            .db
            .set_snowman(x, y, &builders, &bits, super::boards::now())?;
        let (mine, theirs) = if first {
            (
                format!(
                    "⛄ You roll a big snowball, then a smaller one, and stack them up. A snowman! You give it {bit}."
                ),
                format!("⛄ {name} builds a snowman, with {bit}."),
            )
        } else {
            (
                format!("⛄ You give the snowman {bit}."),
                format!("⛄ {name} gives the snowman {bit}."),
            )
        };
        self.say1(id, Line::col(Col::White, mine));
        self.tell_here(x, y, id, Line::col(Col::White, theirs));
        Ok(())
    }

    fn snowy(&self, x: i64, y: i64) -> bool {
        self.season == Season::Winter && !self.world.terrain(x, y).is_wet()
    }

    pub(super) fn load_snowmen(&mut self) {
        match self.world.db.snowmen() {
            Ok(rows) => {
                for (x, y, builders, bits) in rows {
                    let b = Built {
                        builders: builders
                            .split(',')
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect(),
                        bits: bits
                            .split('|')
                            .filter(|s| !s.is_empty())
                            .map(String::from)
                            .collect(),
                    };
                    self.snowmen.insert((x, y), b);
                }
            }
            Err(e) => eprintln!("couldn't load snowmen: {e}"),
        }
    }

    /// The season's turned: winter's snow goes.
    pub(super) fn play_season(&mut self, from: Season) {
        if from != Season::Winter {
            return;
        }
        self.angels.clear();
        let spots: Vec<(i64, i64)> = self.snowmen.drain().map(|(k, _)| k).collect();
        if let Err(e) = self.world.db.melt_snowmen() {
            eprintln!("melting snowmen: {e}");
        }
        for (x, y) in spots {
            self.tell_here(x, y, 0, Line::col(Col::White, "⛄ The snowman slumps, gives you one last smile, and melts away into the spring."));
        }
    }

    // ---- what you're wearing --------------------------------------------

    fn cmd_wear(&mut self, id: u64, what: Option<&str>) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let now_wearing = self.players.get(&id).and_then(|p| p.fun.wearing.clone());
        let Some(what) = what else {
            let Some(w) = now_wearing else {
                self.say1(id, Line::plain("You're not wearing anything special."));
                return Ok(());
            };
            self.world.db.set_wearing(&user, None)?;
            if let Some(p) = self.players.get_mut(&id) {
                p.fun.wearing = None;
            }
            self.say1(
                id,
                Line::col(Col::Magenta, format!("You take off {w} and tuck it away.")),
            );
            return Ok(());
        };
        if what.trim().is_empty() {
            let msg = match now_wearing {
                Some(w) => format!("You're wearing {w}. (take off to take it off)"),
                None => "Wear what? Anything you like: wear a seashell crown".to_string(),
            };
            self.say1(id, Line::plain(msg));
            return Ok(());
        }
        let Some(w) = wearable(what) else {
            self.say1(
                id,
                Line::plain(format!(
                    "That's a lot to wear. Something shorter? ({WEAR_MAX} letters or fewer)"
                )),
            );
            return Ok(());
        };
        self.world.db.set_wearing(&user, Some(&w))?;
        if let Some(p) = self.players.get_mut(&id) {
            p.fun.wearing = Some(w.clone());
        }
        let compliment = *self.rng.pick(&[
            "Gorgeous.",
            "Stunning.",
            "Very fetching.",
            "Iconic.",
            "The island approves.",
        ]);
        self.say1(
            id,
            Line::col(Col::Magenta, format!("✨ You put on {w}. {compliment}")),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(Col::Magenta, format!("✨ {name} puts on {w}. {compliment}")),
        );
        Ok(())
    }

    // ---- as time goes by -------------------------------------------------

    /// Every tick: tides, kites, seagulls, fresh snow.
    pub(super) fn play_tick(&mut self) {
        // The tide takes every sandcastle.
        if Instant::now() >= self.next_tide {
            self.next_tide =
                Instant::now() + Duration::from_secs_f64(self.rng.range(TIDES.0, TIDES.1));
            let spots: Vec<(i64, i64)> = self.castles.drain().map(|(k, _)| k).collect();
            for (x, y) in spots {
                self.tell_here(x, y, 0, Line::col(Col::Blue, "🌊 A wave creeps up the beach and gently smooths the sandcastle back into sand. The tide always wins."));
            }
        }
        // Fresh snow fills in snow angels.
        let filled: Vec<(i64, i64)> = self
            .angels
            .iter()
            .filter(|(k, a)| {
                a.since.elapsed() > Duration::from_secs(120)
                    && self.world.weather.sky_at(k.0, k.1).wet()
            })
            .map(|(k, _)| *k)
            .collect();
        for (x, y) in filled {
            self.angels.remove(&(x, y));
            self.tell_here(
                x,
                y,
                0,
                Line::col(Col::White, "❄ Fresh snow softly fills in the snow angels."),
            );
        }
        let ids: Vec<u64> = self.players.keys().copied().collect();
        for id in ids {
            self.kite_tick(id);
            self.gull_tick(id);
        }
    }

    fn kite_tick(&mut self, id: u64) {
        let Some(p) = self.players.get(&id) else {
            return;
        };
        let Some(k) = &p.fun.kite else { return };
        let (x, y, name, idle) = (
            p.x,
            p.y,
            p.name.clone(),
            p.ambient_quiet >= AMBIENT_WHILE_IDLE,
        );
        if k.at != (x, y) {
            // Carried off some other way (a boat).
            if let Some(p) = self.players.get_mut(&id) {
                p.fun.kite = None;
            }
            return;
        }
        if k.tangled {
            return;
        }
        if self.world.weather.speed() < KITE_WIND - 0.03 {
            if let Some(p) = self.players.get_mut(&id) {
                p.fun.kite = None;
            }
            self.say1(
                id,
                Line::col(
                    Col::Cyan,
                    "🪁 The wind drops, and your kite drifts gently back down into your arms.",
                ),
            );
            return;
        }
        if self.trees_by(x, y) && self.rng.chance(1.0 / 100.0) {
            if let Some(k) = self.players.get_mut(&id).and_then(|p| p.fun.kite.as_mut()) {
                k.tangled = true;
            }
            self.say1(id, Line::col(Col::Cyan, "🪁 A gust swings your kite into the branches of a tree, and it's stuck fast! A friend could untangle it."));
            self.tell_here(
                x,
                y,
                id,
                Line::col(
                    Col::Cyan,
                    format!("🪁 {name}'s kite is stuck in a tree! (untangle)"),
                ),
            );
            return;
        }
        // Now and then, how it's going: never on and on to someone who's stepped away.
        if idle || !self.rng.chance(1.0 / 25.0) {
            return;
        }
        let line = *self.rng.pick(&[
            "🪁 Your kite swoops and loops on the wind.",
            "🪁 Your kite tugs at the string like it wants to go somewhere.",
            "🪁 A gull flies alongside your kite for a bit, deeply confused.",
            "🪁 Your kite hangs perfectly still, high up, like it's been pinned to the sky.",
        ]);
        if let Some(p) = self.players.get_mut(&id) {
            p.ambient_quiet += 1;
        }
        self.say1(id, Line::col(Col::Cyan, line));
        self.tell_about(
            x,
            y,
            id,
            &Line::col(
                Col::Cyan,
                format!("🪁 {name}'s kite swoops and loops above you."),
            ),
        );
    }

    fn gull_tick(&mut self, id: u64) {
        let Some(p) = self.players.get(&id) else {
            return;
        };
        let (x, y, name, user) = (p.x, p.y, p.name.clone(), p.user.clone());
        let beach = self.world.terrain(x, y) == Terrain::Sand || self.sea_by(x, y);
        if let Some(since) = p.fun.gull {
            if !beach {
                if let Some(p) = self.players.get_mut(&id) {
                    p.fun.gull = None; // it's lost interest
                }
                return;
            }
            if since.elapsed() < GULL_CIRCLES {
                return;
            }
            if let Some(p) = self.players.get_mut(&id) {
                p.fun.gull = None;
                p.fun.gull_last = Some(Instant::now());
            }
            let Some(food) = self.food_of(&user) else {
                return;
            };
            if self.world.db.satchel_add(&user, food.id, -1).is_err() {
                return;
            }
            self.say1(id, Line::col(Col::Cyan, format!("🐦 SWOOP! The seagull snatches {} right out of your satchel and flaps off, looking very pleased with itself.", nature::a(food.name))));
            self.tell_here(
                x,
                y,
                id,
                Line::col(
                    Col::Cyan,
                    format!(
                        "🐦 A seagull swoops down and snatches {name}'s {}! The cheek of it.",
                        food.name
                    ),
                ),
            );
            return;
        }
        // Only someone who's about (just did something), now and then, with
        // something cooked: a story, never a nuisance.
        let about = !p.bot && p.ambient_quiet == 0;
        let rested = p.fun.gull_last.is_none_or(|t| t.elapsed() > GULL_EVERY);
        if !(beach && about && rested && self.rng.chance(1.0 / 300.0))
            || self.food_of(&user).is_none()
        {
            return;
        }
        if let Some(p) = self.players.get_mut(&id) {
            p.fun.gull = Some(Instant::now());
        }
        self.say1(
            id,
            Line::col(
                Col::Cyan,
                "🐦 A seagull circles overhead, eyeing your satchel with great interest... (shoo!)",
            ),
        );
    }

    /// Somebody whispers about dark mode, or, once it's on, about the way out.
    fn dark_hint(&self, id: u64) -> Option<&'static str> {
        Some(if self.is_dark(id) {
            "Something is very wrong with the animals today. Maybe it's time for light mode."
        } else {
            "For half a second the whole island flickers, like someone reaching for a light switch. Somewhere, someone whispers about a dark mode."
        })
    }

    fn food_of(&self, user: &str) -> Option<&'static nature::Species> {
        self.world
            .db
            .satchel(user)
            .ok()?
            .iter()
            .filter_map(|(i, n)| (*n > 0).then(|| nature::by_id(i)).flatten())
            .find(|s| s.kind == Kind::Food)
    }

    /// A little nudge in the scenery towards something to do here: the
    /// island's way of showing what's possible, a bit at a time, rather
    /// than a help page listing everything.
    pub(super) fn hint(&mut self, id: u64, sky: Sky) -> Option<&'static str> {
        let p = self.players.get(&id)?;
        let (x, y, user, befriended) = (p.x, p.y, p.user.clone(), p.companion.is_some());
        let t = self.world.terrain(x, y);
        let dry = !t.is_wet();
        let mut hints: Vec<&'static str> = Vec::new();
        if let Some(b) = self.world.room_building(x, y) {
            // Indoors: only indoor thoughts.
            if self
                .world
                .home_at(b.x, b.y)
                .is_some_and(|h| h.welcomes(&user))
                || b.maker == user
            {
                hints.extend([
                    "This room could be anything you like. Describe it in your own words. (describe …)",
                    "A feather cushion and a leaf rug would make it in here cosier. (recipes)",
                    "A jar on the windowsill would be just the place for your finds.",
                    "A typewriter would sit nicely in a quiet corner in here. Whole island stories to copy out, a page at a time.",
                ]);
            }
            hints.push("Rain or shine, it's snug in here.");
            if self.typewriter_here(x, y) {
                hints.push("A typewriter waits with a fresh sheet of paper in it. It's practically begging to be typed on.");
            }
            hints.extend(self.dark_hint(id));
            let fresh: Vec<&'static str> = hints
                .into_iter()
                .filter(|h| {
                    !self
                        .players
                        .get(&id)
                        .is_some_and(|p| p.fun.hinted.contains(h))
                })
                .collect();
            if fresh.is_empty() {
                return None;
            }
            let hint = *self.rng.pick(&fresh);
            self.players.get_mut(&id)?.fun.hinted.insert(hint);
            return Some(hint);
        }
        // Toys.
        if self.season == Season::Winter && dry {
            hints.extend([
                "The snow here is just right for a snowman.",
                "The snow looks soft enough to lie down and make a snow angel in.",
            ]);
        } else if sky.wet() && dry {
            hints.push("Puddles are gathering everywhere. Very jumpable.");
        }
        if self.folk_nearby(x, y) {
            hints.extend([
                "Somebody nearby is whistling to themself.",
                "You hear someone chatting away nearby, to nobody in particular.",
            ]);
        }
        if t == Terrain::Sand && !self.castles.contains_key(&(x, y)) {
            hints.push("The damp sand here would make a lovely sandcastle.");
        }
        if self.sea_by(x, y) {
            hints.push(
                "An empty bottle bobs past on the waves. Someone ought to put a note in one.",
            );
        }
        if self.water_by(x, y).is_some() {
            hints.extend(["A flat, smooth stone catches your eye. Perfect for skimming.", "A fish jumps, and rings of ripples spread out. Anyone with a line could fish here."]);
        }
        if self.world.weather.breezy() && t != Terrain::Forest && dry {
            hints.push("The wind tugs at your sleeves. Proper kite weather.");
        }
        if super::hills::high(t) {
            hints.extend([
                "The wind's fresh up here, and the view must be something. You could look out over the whole island.",
                "Up here, your voice would carry for miles. The hills might even shout back.",
                "A round stone sits at the top of the slope, just asking to be rolled.",
            ]);
        }
        if t == Terrain::Meadow {
            hints.extend([
                "Daisies everywhere. You could make a daisy chain and wear it.",
                "You catch a smell of wild mint. It'd make lovely tea, over a campfire.",
            ]);
        }
        hints.extend(self.build_hints(id));
        if self
            .players
            .iter()
            .any(|(pid, p)| *pid != id && (p.x, p.y) == (x, y) && !p.bot)
        {
            hints.push("Two's company. You could settle just about anything with a game of rock, paper, scissors.");
            let night = matches!(
                crate::server::weather::Clock::phase(),
                crate::server::weather::Phase::Night
            );
            if sky.wet() {
                hints.push("The rain's coming down. An umbrella's just big enough for two.");
            } else if night && sky == Sky::Clear {
                hints.push("The stars are out, bright and thick. Lie back together and you might spot a constellation.");
            } else if !night && dry && matches!(t, Terrain::Meadow | Terrain::Grass) {
                hints.push("What a spot for a picnic. Good grass, good company.");
            }
        }
        hints.extend(self.dark_hint(id));
        if self.typewriter_here(x, y) {
            hints.push("A typewriter waits with a fresh sheet of paper in it. It's practically begging to be typed on.");
        }
        // Everything else there is to do.
        if matches!(t, Terrain::Grass | Terrain::Dirt) {
            hints.push("The earth here is soft. Easy to dig, or to plant something in.");
        }
        if dry
            && self.world.tile(x, y).is_some_and(|t| t.name.is_none())
            && self.world.home_at(x, y).is_none()
        {
            hints.push("This spot doesn't have a name yet. You could give it one.");
        }
        if self.world.home_at(x, y).is_some_and(|h| h.owner == user) {
            hints.push(
                "A pot of paint sits by your door. Anything here could be any colour you like.",
            );
            hints.push("Your home could do with somewhere to sit. A feather cushion, maybe, or a toadstool stool. (recipes)");
            hints.push("An empty shelf by the door is just waiting for a few of your finds. A jar of fireflies would glow lovely at night.");
        }
        if dry
            && matches!(t, Terrain::Grass | Terrain::Meadow)
            && !self.world.homes.iter().any(|h| h.owner == user)
        {
            hints.push("This would be a lovely spot to make a home. You could claim it.");
        }
        if !befriended
            && self
                .wild
                .iter()
                .any(|w| (w.x - x).abs().max((w.y - y).abs()) <= 4)
        {
            hints.push("Something small and furry is watching you from a little way off. It might like a berry, if you offered one.");
        }
        if self.world.things_at(x, y).any(|t| t.is_dock()) {
            hints.extend(["The rowing boat bobs at the end of the dock. You could sail across to another island.", "A weathered signpost by the dock points the way to all the named places. (places)"]);
        }
        if self.pieces_on(x, y) {
            hints.push("There's talk of something in pieces, scattered about this island. The pieces must belong somewhere.");
        }
        if crate::server::islands::island_at(x, y).id == 1 && !self.lighthouse_lit() {
            hints.push("Out on the point, the old lighthouse waits for someone to mend it.");
        }
        hints.extend([
            "You find yourself humming a little tune.",
            "You think of everything you've found so far. Your journal must be filling up.",
        ]);
        let fresh: Vec<&'static str> = hints
            .into_iter()
            .filter(|h| {
                !self
                    .players
                    .get(&id)
                    .is_some_and(|p| p.fun.hinted.contains(h))
            })
            .collect();
        if fresh.is_empty() {
            return None;
        }
        let hint = *self.rng.pick(&fresh);
        self.players.get_mut(&id)?.fun.hinted.insert(hint);
        Some(hint)
    }
}

fn capital(s: &str) -> String {
    crate::server::world::capitalise(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skimming_is_told_in_words() {
        assert!(skim(3, "x").contains("plop"));
        assert!(skim(15, "x").contains("once"));
        assert!(skim(97, "x").contains("lose sight"));
        assert_eq!(
            skim(40, "a duck looks deeply offended"),
            "It skips six times, and a duck looks deeply offended."
        );
        assert_eq!(skim(41, "x"), "It skips six times.");
        assert_eq!(skim(91, "x"), "It skips twice.");
        for r in 0..100 {
            assert!(
                !skim(r, "x").chars().any(|c| c.is_ascii_digit()),
                "never a number: {}",
                skim(r, "x")
            );
        }
    }

    #[test]
    fn wearing_reads_nicely() {
        assert_eq!(
            wearable("seashell crown").as_deref(),
            Some("a seashell crown")
        );
        assert_eq!(wearable("my lucky hat").as_deref(), Some("a lucky hat"));
        assert_eq!(wearable("sunglasses").as_deref(), Some("sunglasses"));
        assert_eq!(wearable("a daisy chain").as_deref(), Some("a daisy chain"));
        assert_eq!(
            wearable("on an old straw hat").as_deref(),
            Some("an old straw hat")
        );
        assert_eq!(wearable(""), None);
        assert_eq!(wearable(&"x".repeat(50)), None);
    }

    #[test]
    fn bottles_take_the_words_after() {
        assert_eq!(bottle_words("Hello there"), "Hello there");
        assert_eq!(bottle_words("a bottle: hi"), "hi");
        assert_eq!(bottle_words("bottle hi"), "hi");
    }

    #[test]
    fn lists_read_nicely() {
        assert_eq!(
            and_list(&["a".into(), "b".into(), "c".into()]),
            "a, b and c"
        );
        assert_eq!(and_list(&["a".into()]), "a");
    }
}
