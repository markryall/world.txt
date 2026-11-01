//! Gathering: finding flowers, butterflies and shells while wandering,
//! fishing, and the satchel and journal that hold what you've found.

use super::Game;
use crate::protocol::{Col, Line, ToClient};
use crate::server::nature::{self, Find, Kind, Species};
use crate::server::weather::Clock;
use crate::server::world::{DIRS, Trigger, capitalise};

pub const SATCHEL_SIZE: i64 = 30;
const MAX_FINDS: usize = 80;
const PER_TILE: usize = 2;
const SPAWN_CHANCE: f64 = 0.6;
/// Within the minimap's view, so every find shows up as a pink dot.
const SPAWN_RX: i64 = 6;
const SPAWN_RY: i64 = 4;
/// Enough to always have something nearby, never so many it's cluttered.
const NEARBY_MAX: usize = 8;

impl Kind {
    pub fn icon(self) -> &'static str {
        match self {
            Kind::Flower => "🌼",
            Kind::Butterfly => "🦋",
            Kind::Bug => "🐞",
            Kind::Mushroom => "🍄",
            Kind::Shell => "🐚",
            Kind::Feather => "🪶",
            Kind::Stone => "💎",
            Kind::Fish => "🐟",
        }
    }
    fn verb(self) -> &'static str {
        match self {
            Kind::Flower | Kind::Mushroom => "pick",
            Kind::Butterfly | Kind::Bug | Kind::Fish => "catch",
            _ => "collect",
        }
    }
    fn words(self) -> &'static [&'static str] {
        match self {
            Kind::Flower => &["flower", "flowers"],
            Kind::Butterfly => &["butterfly", "butterflies", "moth"],
            Kind::Bug => &["bug", "bugs", "beetle", "insect"],
            Kind::Mushroom => &["mushroom", "mushrooms", "toadstool"],
            Kind::Shell => &["shell", "shells"],
            Kind::Feather => &["feather", "feathers"],
            Kind::Stone => &["stone", "stones", "rock", "crystal", "gem"],
            Kind::Fish => &["fish"],
        }
    }
}

fn matches(s: &Species, what: &str) -> bool {
    let w = crate::server::world::strip_article(what.trim()).to_lowercase();
    w.is_empty() || s.name.contains(&w) || s.id == w || s.kind.words().contains(&w.as_str())
}

impl Game {
    pub(super) fn find_positions(&self) -> Vec<(i64, i64)> {
        self.finds.iter().map(|f| (f.x, f.y)).collect()
    }

    /// Lines for `look`: whatever's lying (or fluttering) about here.
    pub(super) fn find_lines(&self, x: i64, y: i64) -> Vec<Line> {
        let here: Vec<&Find> = self.finds.iter().filter(|f| f.x == x && f.y == y).collect();
        let mut out: Vec<Line> = here
            .iter()
            .map(|f| {
                let s = f.species;
                Line::col(Col::Pink, format!("{} {} {}.", s.kind.icon(), capitalise(&nature::a(s.name)), s.here))
            })
            .collect();
        if !out.is_empty() {
            out.push(Line::col(Col::Dim, "  (type gather to take it)"));
        }
        out
    }

    // ---- the world growing things -------------------------------------

    pub(super) fn nature_tick(&mut self) {
        let now = std::time::Instant::now();
        self.finds.retain(|f| f.until > now);
        let phase = Clock::phase();

        // New finds appear near wandering people.
        let spots: Vec<(u64, i64, i64)> =
            self.players.iter().filter(|(_, p)| !p.bot).map(|(&id, p)| (id, p.x, p.y)).collect();
        for (id, px, py) in spots {
            let nearby = self.finds.iter().filter(|f| (f.x - px).abs() <= SPAWN_RX && (f.y - py).abs() <= SPAWN_RY).count();
            if self.finds.len() >= MAX_FINDS || nearby >= NEARBY_MAX || !self.rng.chance(SPAWN_CHANCE) {
                continue;
            }
            let dx = (self.rng.next() % (2 * SPAWN_RX as u64 + 1)) as i64 - SPAWN_RX;
            let dy = (self.rng.next() % (2 * SPAWN_RY as u64 + 1)) as i64 - SPAWN_RY;
            let (x, y) = (px + dx, py + dy);
            let ground = self.world.terrain(x, y);
            if ground.is_wet() || self.finds.iter().filter(|f| f.x == x && f.y == y).count() >= PER_TILE {
                continue;
            }
            let sky = self.world.weather.sky_at(x, y);
            let Some(species) = nature::choose(&mut self.rng, ground, sky, phase, false) else { continue };
            let until = nature::lifetime(&mut self.rng);
            self.finds.push(Find { species, x, y, until });

            // Fluttery things announce themselves when they're close.
            let d = dx.abs().max(dy.abs());
            if matches!(species.kind, Kind::Butterfly | Kind::Bug) && (1..=2).contains(&d) {
                let dir = DIRS.iter().find(|q| q.2 == dx.signum() && q.3 == dy.signum()).map_or("nearby", |q| q.1);
                self.say1(
                    id,
                    Line::col(Col::Pink, format!("{} {} flutters down just to the {dir}.", species.kind.icon(), capitalise(&nature::a(species.name)))),
                );
            }
        }

        // Lines out in the water.
        let fishers: Vec<u64> = self.players.iter().filter(|(_, p)| p.fishing.is_some()).map(|(&id, _)| id).collect();
        for id in fishers {
            let left = self.players.get_mut(&id).and_then(|p| {
                p.fishing = p.fishing.map(|n| n.saturating_sub(1));
                p.fishing
            });
            match left {
                Some(0) => {
                    if let Some(p) = self.players.get_mut(&id) {
                        p.fishing = None;
                    }
                    let _ = self.land_fish(id);
                }
                Some(1) => self.say1(id, Line::col(Col::Blue, "〰 Your float bobs... something's nibbling!")),
                _ => {}
            }
        }
    }

    fn water_near(&self, x: i64, y: i64) -> Option<crate::server::terrain::Terrain> {
        let here = self.world.terrain(x, y);
        if here.is_wet() {
            return Some(here);
        }
        let around: Vec<_> = DIRS.iter().map(|d| self.world.terrain(x + d.2, y + d.3)).filter(|t| t.is_wet()).collect();
        // A pond nearby beats the sea: ponds have their own fish.
        around.iter().copied().find(|t| *t == crate::server::terrain::Terrain::Pond).or(around.first().copied())
    }

    fn land_fish(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let Some(water) = self.water_near(x, y) else { return Ok(()) };
        let sky = self.world.weather.sky_at(x, y);
        let fish = if self.rng.chance(0.2) {
            None
        } else {
            nature::choose(&mut self.rng, water, sky, Clock::phase(), true)
        };
        match fish {
            None => self.say1(id, Line::col(Col::Blue, "🎣 The float goes still. Nothing bites this time. Try again?")),
            Some(s) => {
                if self.collect(id, s)? {
                    self.say1(id, Line::new().c(Col::Blue, "🎣 A tug! You reel in ").b(Col::Blue, nature::a(s.name)).c(Col::Blue, "!"));
                    self.tell_here(x, y, id, Line::col(Col::Blue, format!("🎣 {name} reels in {}!", nature::a(s.name))));
                }
            }
        }
        Ok(())
    }

    /// Put something in the satchel and the journal. False if there's no room.
    fn collect(&mut self, id: u64, s: &'static Species) -> anyhow::Result<bool> {
        let Some((_, _, name)) = self.here(id) else { return Ok(false) };
        let carried: i64 = self.world.db.satchel(&name)?.iter().map(|(_, n)| n).sum();
        if carried >= SATCHEL_SIZE {
            self.say1(id, Line::plain("🧺 Your satchel is full! Give, place or release something first."));
            return Ok(false);
        }
        self.world.db.satchel_add(&name, s.id, 1)?;
        if self.world.db.journal_add(&name, s.id)? {
            self.say1(
                id,
                Line::new().b(Col::Yellow, "📖 New in your journal: ").b(Col::White, capitalise(s.name)).c(Col::Yellow, s.rarity.label()),
            );
        }
        Ok(true)
    }

    // ---- commands -----------------------------------------------------

    pub(super) fn cmd_gather(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let Some(i) = self.finds.iter().position(|f| f.x == x && f.y == y && matches(f.species, what)) else {
            let msg = if what.trim().is_empty() || !self.finds.iter().any(|f| f.x == x && f.y == y) {
                "There's nothing to gather right here. Look for pink • on your map!".to_string()
            } else {
                format!("You don't see any {what} here.")
            };
            self.say1(id, Line::plain(msg));
            return Ok(());
        };
        let s = self.finds[i].species;
        if !nature::caught(&mut self.rng, s) {
            // It gets away, to a neighbouring spot it likes.
            let options: Vec<(i64, i64, &str)> = DIRS
                .iter()
                .map(|d| (x + d.2, y + d.3, d.1))
                .filter(|&(nx, ny, _)| s.ground.contains(&self.world.terrain(nx, ny)))
                .collect();
            if options.is_empty() {
                self.finds.remove(i);
                self.say1(id, Line::col(Col::Pink, format!("{} The {} slips away into the sky!", s.kind.icon(), s.name)));
            } else {
                let &(nx, ny, dir) = self.rng.pick(&options);
                self.finds[i].x = nx;
                self.finds[i].y = ny;
                self.say1(id, Line::col(Col::Pink, format!("{} So close! The {} flutters off to the {dir}.", s.kind.icon(), s.name)));
            }
            self.status_all();
            return Ok(());
        }
        if !self.collect(id, s)? {
            return Ok(());
        }
        self.finds.remove(i);
        let verb = s.kind.verb();
        self.say1(id, Line::new().c(Col::Pink, format!("{} You {verb} ", s.kind.icon())).b(Col::Pink, nature::a(s.name)).c(Col::Pink, "."));
        self.tell_here(x, y, id, Line::col(Col::Pink, format!("{} {name} {verb}s {}.", s.kind.icon(), nature::a(s.name))));
        self.status_all();
        Ok(())
    }

    pub(super) fn cmd_fish(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        if self.water_near(x, y).is_none() {
            self.say1(id, Line::plain("🎣 You'll need to be by the water to fish."));
            return Ok(());
        }
        let ticks = 2 + (self.rng.next() % 3) as u8;
        if let Some(p) = self.players.get_mut(&id) {
            if p.fishing.is_some() {
                self.say1(id, Line::plain("🎣 Your line's already in. Patience..."));
                return Ok(());
            }
            p.fishing = Some(ticks);
        }
        self.say1(id, Line::col(Col::Blue, "🎣 You cast your line and wait quietly..."));
        self.tell_here(x, y, id, Line::col(Col::Blue, format!("🎣 {name} casts a line into the water.")));
        Ok(())
    }

    pub(super) fn cmd_satchel(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((_, _, name)) = self.here(id) else { return Ok(()) };
        let items = self.world.db.satchel(&name)?;
        let total: i64 = items.iter().map(|(_, n)| n).sum();
        let mut lines = vec![Line::new().b(Col::Magenta, format!("🧺 Your satchel ({total}/{SATCHEL_SIZE})"))];
        if items.is_empty() {
            lines.push(Line::col(Col::Dim, "  Empty. Wander about and gather things!"));
        }
        for (sid, n) in items {
            if let Some(s) = nature::by_id(&sid) {
                let count = if n > 1 { format!("{n} × ") } else { String::new() };
                lines.push(Line::new().t(format!("  {} {count}{}", s.kind.icon(), s.name)));
            }
        }
        lines.push(Line::col(Col::Dim, "  give daisy to Poppy · place daisy · release butterfly · journal"));
        self.send(id, lines);
        Ok(())
    }

    pub(super) fn cmd_journal(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((_, _, name)) = self.here(id) else { return Ok(()) };
        let found = self.world.db.journal(&name)?;
        let mut lines = vec![Line::new().b(
            Col::Magenta,
            format!("📖 Your journal: {} of {} discovered", found.len(), nature::SPECIES.len()),
        )];
        for kind in Kind::ALL {
            let all: Vec<&Species> = nature::SPECIES.iter().filter(|s| s.kind == kind).collect();
            let mine: Vec<&&Species> = all.iter().filter(|s| found.iter().any(|f| f == s.id)).collect();
            lines.push(Line::new().b(Col::Yellow, format!("{}  {}/{}", kind.title(), mine.len(), all.len())));
            let mut l = Line::new().t("   ");
            for s in &mine {
                let c = match s.rarity {
                    nature::Rarity::Rare => Col::Magenta,
                    nature::Rarity::Uncommon => Col::Cyan,
                    nature::Rarity::Common => Col::Plain,
                };
                l = l.c(c, format!("{} ", s.name)).c(Col::Dim, "· ");
            }
            let missing = all.len() - mine.len();
            if missing > 0 {
                l = l.c(Col::Dim, format!("{} still to find", missing));
            } else {
                l = l.b(Col::Green, "all found! 🎉");
            }
            lines.push(l);
        }
        lines.push(Line::col(Col::Dim, "Some only appear at night, in the rain, or in certain places."));
        self.send(id, lines);
        Ok(())
    }

    /// Find something in the satchel by name.
    fn carried(&self, player: &str, what: &str) -> anyhow::Result<Option<&'static Species>> {
        Ok(self
            .world
            .db
            .satchel(player)?
            .iter()
            .filter_map(|(sid, _)| nature::by_id(sid))
            .find(|s| !what.trim().is_empty() && matches(s, what)))
    }

    pub(super) fn cmd_give(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let Some((what, to)) = rest.rsplit_once(" to ") else {
            self.say1(id, Line::plain("Give what to who? e.g. 'give daisy to Poppy'"));
            return Ok(());
        };
        let Some(s) = self.carried(&name, what)? else {
            self.say1(id, Line::plain(format!("You don't have any {} in your satchel.", what.trim())));
            return Ok(());
        };
        let Some(tid) = self.find_player(to) else {
            self.say1(id, Line::plain(format!("Nobody called {} is in the world right now.", to.trim())));
            return Ok(());
        };
        let (tname, tx, ty, tbot) = {
            let t = &self.players[&tid];
            (t.name.clone(), t.x, t.y, t.bot)
        };
        if tid == id {
            self.say1(id, Line::plain("You give it to yourself. It's already yours, sweetie."));
            return Ok(());
        }
        if (tx, ty) != (x, y) {
            self.say1(id, Line::plain(format!("{tname} isn't here. Try 'find {tname}' and go to them!")));
            return Ok(());
        }
        let theirs: i64 = self.world.db.satchel(&tname)?.iter().map(|(_, n)| n).sum();
        if !tbot && theirs >= SATCHEL_SIZE {
            self.say1(id, Line::plain(format!("{tname}'s satchel is too full to take it.")));
            return Ok(());
        }
        self.world.db.satchel_add(&name, s.id, -1)?;
        if !tbot {
            self.world.db.satchel_add(&tname, s.id, 1)?;
            if self.world.db.journal_add(&tname, s.id)? {
                self.say1(tid, Line::new().b(Col::Yellow, "📖 New in your journal: ").b(Col::White, capitalise(s.name)).c(Col::Yellow, s.rarity.label()));
            }
        }
        let thing = nature::a(s.name);
        self.say1(id, Line::col(Col::Pink, format!("🎁 You give {tname} {thing}.")));
        self.say1(tid, Line::new().c(Col::Pink, "🎁 ").b(Col::Pink, &name).c(Col::Pink, format!(" gives you {thing}!")));
        for (&pid, p) in &self.players {
            if pid != id && pid != tid && (p.x, p.y) == (x, y) {
                let _ = p.tx.send(ToClient::Lines { lines: vec![Line::col(Col::Pink, format!("🎁 {name} gives {tname} {thing}."))] });
            }
        }
        Ok(())
    }

    pub(super) fn cmd_place(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let Some(s) = self.carried(&name, what)? else {
            self.say1(id, Line::plain("Place what? Try 'satchel' to see what you're carrying."));
            return Ok(());
        };
        if s.kind.alive() {
            self.say1(id, Line::plain(format!("The {} would rather be free. Try 'release {}'.", s.name, s.name)));
            return Ok(());
        }
        if self.world.terrain(x, y).is_wet() {
            self.say1(id, Line::plain("It would just float away. Find dry ground."));
            return Ok(());
        }
        if self.world.things_at(x, y).count() >= 12 {
            self.say1(id, Line::plain("It's getting crowded here. Try somewhere else?"));
            return Ok(());
        }
        self.world.db.satchel_add(&name, s.id, -1)?;
        self.world.add_thing(x, y, nature::a(s.name), None, Trigger::Always, &name)?;
        self.say1(id, Line::col(Col::Yellow, format!("{} You place {} here, just so.", s.kind.icon(), nature::a(s.name))));
        self.tell_here(x, y, id, Line::col(Col::Yellow, format!("{} {name} places {} here.", s.kind.icon(), nature::a(s.name))));
        self.status_all();
        Ok(())
    }

    pub(super) fn cmd_release(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let found = self.carried(&name, what)?;
        let Some(s) = found.filter(|s| s.kind.alive()) else {
            self.say1(id, Line::plain("Release what? Butterflies, bugs and fish can go free."));
            return Ok(());
        };
        self.world.db.satchel_add(&name, s.id, -1)?;
        let how = match s.kind {
            Kind::Fish => "plops back into the water and swims away",
            Kind::Bug => "trundles off into the grass",
            _ => "flutters up and away into the sky",
        };
        self.say1(id, Line::col(Col::Pink, format!("{} The {} {how}. Bye bye!", s.kind.icon(), s.name)));
        self.tell_here(x, y, id, Line::col(Col::Pink, format!("{} {name} lets a {} go. It {how}.", s.kind.icon(), s.name)));
        Ok(())
    }
}
