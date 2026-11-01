//! Companions: wild animals turn up near people now and then. Offer one a
//! berry and it may decide you're friends, then it follows you everywhere.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::nature::{self, Kind};
use crate::server::terrain::Terrain as T;
use crate::server::weather::{Clock, Phase};
use crate::server::world::{DIRS, capitalise};
use std::time::{Duration, Instant};

pub struct Animal {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    ground: &'static [T],
    night: Option<bool>,
    /// "A fox {here}."
    here: &'static str,
    tricks: &'static [&'static str],
}

pub const ANIMALS: &[Animal] = &[
    Animal { id: "fox", name: "fox", icon: "🦊", ground: &[T::Forest], night: None, here: "peeks out from behind a tree, ears twitching",
        tricks: &["sniffs the air", "pounces on a leaf", "curls its tail around your feet", "yips softly"] },
    Animal { id: "duckling", name: "duckling", icon: "🐥", ground: &[T::Pond, T::Water], night: Some(false), here: "paddles about, peeping",
        tricks: &["peeps happily", "waddles in a little circle", "shakes water off its fluffy feathers"] },
    Animal { id: "rabbit", name: "rabbit", icon: "🐇", ground: &[T::Meadow, T::Grass], night: Some(false), here: "sits up on its back legs, nose twitching",
        tricks: &["nibbles some clover", "does a happy little hop", "twitches its nose at you"] },
    Animal { id: "hedgehog", name: "hedgehog", icon: "🦔", ground: &[T::Forest, T::Grass, T::Meadow], night: Some(true), here: "snuffles through the leaves",
        tricks: &["snuffles in the leaves", "curls into a prickly ball, then uncurls", "trundles along beside you"] },
    Animal { id: "turtle", name: "turtle", icon: "🐢", ground: &[T::Sand], night: None, here: "blinks at you from the sand",
        tricks: &["blinks slowly", "pokes its head out to look around", "trundles very, very slowly after you"] },
];

pub fn animal(id: &str) -> Option<&'static Animal> {
    ANIMALS.iter().find(|a| a.id == id)
}

pub struct Wild {
    pub animal: &'static Animal,
    pub x: i64,
    pub y: i64,
    pub until: Instant,
}

/// A companion's friendly label: "Rusty the fox", or "your fox".
pub fn label(animal: &Animal, name: &Option<String>) -> String {
    match name {
        Some(n) => format!("{n} the {}", animal.name),
        None => format!("the {}", animal.name),
    }
}

impl Game {
    /// Now and then, an animal wanders out near someone.
    pub(super) fn animals_tick(&mut self) {
        let now = Instant::now();
        self.wild.retain(|w| w.until > now);
        let night = Clock::phase() == Phase::Night;
        let spots: Vec<(u64, i64, i64)> =
            self.players.iter().filter(|(_, p)| !p.bot).map(|(&id, p)| (id, p.x, p.y)).collect();
        for (id, px, py) in spots {
            let near = self.wild.iter().any(|w| (w.x - px).abs() <= 6 && (w.y - py).abs() <= 4);
            if near || self.wild.len() >= 12 || !self.rng.chance(0.1) {
                continue;
            }
            // Right beside you, so "just to the north" is exactly one step.
            let dx = (self.rng.next() % 3) as i64 - 1;
            let dy = (self.rng.next() % 3) as i64 - 1;
            let (x, y) = (px + dx, py + dy);
            let ground = self.world.terrain(x, y);
            let options: Vec<&'static Animal> = ANIMALS
                .iter()
                .filter(|a| a.ground.contains(&ground) && a.night.is_none_or(|n| n == night))
                .collect();
            if options.is_empty() {
                continue;
            }
            let a = *self.rng.pick(&options);
            let until = now + Duration::from_secs(self.rng.range(180.0, 360.0) as u64);
            self.wild.push(Wild { animal: a, x, y, until });
            let d = dx.abs().max(dy.abs());
            let line = if d == 0 {
                format!("{} A {} {} right here!", a.icon, a.name, a.here)
            } else {
                let dir = DIRS.iter().find(|q| q.2 == dx.signum() && q.3 == dy.signum()).map_or("nearby", |q| q.1);
                format!("{} A {} {}, just to the {dir}.", a.icon, a.name, a.here)
            };
            self.say1(id, Line::col(Col::Green, line));
        }

        // Companions get up to little things.
        let pets: Vec<(i64, i64, String)> = self
            .players
            .values()
            .filter_map(|p| p.companion.as_ref().map(|(a, n)| (p.x, p.y, a, n)))
            .filter_map(|(x, y, a, n)| animal(a).map(|an| (x, y, an, n.clone())))
            .filter_map(|(x, y, an, n)| {
                if self.rng.chance(1.0 / 25.0) {
                    let trick = *self.rng.pick(an.tricks);
                    Some((x, y, format!("{} {} {trick}.", an.icon, capitalise(&label(an, &n)))))
                } else {
                    None
                }
            })
            .collect();
        for (x, y, line) in pets {
            self.tell_here(x, y, u64::MAX, Line::col(Col::Green, line));
        }
    }

    /// For `look`: wild animals here, and everyone's companions.
    pub(super) fn animal_lines(&self, id: u64) -> Vec<Line> {
        let Some(me) = self.players.get(&id) else { return vec![] };
        let mut out = Vec::new();
        for w in self.wild.iter().filter(|w| (w.x, w.y) == (me.x, me.y)) {
            out.push(
                Line::new()
                    .c(Col::Green, format!("{} A {} {}.", w.animal.icon, w.animal.name, w.animal.here))
                    .c(Col::Dim, format!(" (befriend {}: it loves berries)", w.animal.name)),
            );
        }
        for p in self.players.values().filter(|p| (p.x, p.y) == (me.x, me.y)) {
            let Some((a, n)) = &p.companion else { continue };
            let Some(an) = animal(a) else { continue };
            let line = if p.name == me.name {
                format!("{} {} is right by your side.", an.icon, capitalise(&label(an, n)))
            } else {
                format!("{} {}, {}'s {}, is here.", an.icon, n.clone().unwrap_or_else(|| format!("A {}", an.name)), p.name, an.name)
            };
            out.push(Line::col(Col::Green, line));
        }
        out
    }

    /// "with Rusty the fox" for arrival lines, if they have a companion.
    pub(super) fn with_companion(&self, id: u64) -> String {
        self.players
            .get(&id)
            .and_then(|p| p.companion.as_ref())
            .and_then(|(a, n)| animal(a).map(|an| format!(", with {} {}", an.icon, label(an, n))))
            .unwrap_or_default()
    }

    pub(super) fn cmd_befriend(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let what = crate::server::world::strip_article(what.trim()).to_lowercase();
        let Some(i) = self.wild.iter().position(|w| (w.x, w.y) == (x, y) && (what.is_empty() || w.animal.name.contains(&what))) else {
            self.say1(id, Line::plain("There's no wild animal here to befriend. Keep wandering and they'll turn up! 🦊🐇🦔"));
            return Ok(());
        };
        if let Some((a, n)) = self.players.get(&id).and_then(|p| p.companion.clone()) {
            let current = animal(&a).map_or("friend".to_string(), |an| label(an, &n));
            self.say1(id, Line::plain(format!("You already have {current}. (let go of them first, if you're sure)")));
            return Ok(());
        }
        let a = self.wild[i].animal;
        let berries: Vec<(&'static nature::Species, i64)> = self
            .world
            .db
            .satchel(&name)?
            .into_iter()
            .filter_map(|(sid, n)| nature::by_id(&sid).map(|s| (s, n)))
            .filter(|(s, _)| s.kind == Kind::Berry)
            .collect();
        let Some((berry, _)) = berries.iter().max_by_key(|(_, n)| *n) else {
            self.say1(id, Line::plain(format!("The {} looks hungry. {}s love berries: gather some first! 🫐", a.name, capitalise(a.name))));
            return Ok(());
        };
        self.world.db.satchel_add(&name, berry.id, -1)?;
        if !self.rng.chance(0.5) {
            self.say1(id, Line::col(Col::Green, format!("{} The {} gobbles up your {} and watches you carefully. Maybe one more?", a.icon, a.name, berry.name)));
            return Ok(());
        }
        self.wild.remove(i);
        self.world.db.set_companion(&name, Some(a.id), None)?;
        if let Some(p) = self.players.get_mut(&id) {
            p.companion = Some((a.id.to_string(), None));
        }
        self.say1(id, Line::new().b(Col::Green, format!("{} The {} eats your {} and decides you're friends! It'll follow you everywhere.", a.icon, a.name, berry.name)));
        self.say1(id, Line::col(Col::Dim, format!("Give it a name: nickname Rusty · pet · let go")));
        self.tell_here(x, y, id, Line::col(Col::Green, format!("{} {name} makes friends with a {}!", a.icon, a.name)));
        Ok(())
    }

    pub(super) fn cmd_nickname(&mut self, id: u64, new: &str) -> anyhow::Result<()> {
        let Some((_, _, name)) = self.here(id) else { return Ok(()) };
        let Some((a, _)) = self.players.get(&id).and_then(|p| p.companion.clone()) else {
            self.say1(id, Line::plain("You don't have a companion yet. Befriend an animal with a berry!"));
            return Ok(());
        };
        let nick: String = new.trim().chars().filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-').take(20).collect();
        if nick.trim().is_empty() {
            self.say1(id, Line::plain("Call them what? e.g. 'nickname Rusty'"));
            return Ok(());
        }
        let nick = capitalise(nick.trim());
        self.world.db.set_companion(&name, Some(&a), Some(&nick))?;
        if let Some(p) = self.players.get_mut(&id) {
            p.companion = Some((a.clone(), Some(nick.clone())));
        }
        let icon = animal(&a).map_or("💚", |an| an.icon);
        self.say1(id, Line::col(Col::Green, format!("{icon} {nick} it is! They look very pleased.")));
        Ok(())
    }

    pub(super) fn cmd_pet(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let Some(an) = self.players.get(&id).and_then(|p| p.companion.as_ref()).and_then(|(a, n)| animal(a).map(|an| (an, n.clone()))) else {
            self.say1(id, Line::plain("There's nobody here who wants a pat. Befriend an animal with a berry!"));
            return Ok(());
        };
        let (a, n) = an;
        let who = label(a, &n);
        let happy = match a.id {
            "fox" => "leans into your hand and closes its eyes",
            "duckling" => "peeps and snuggles into your palm",
            "rabbit" => "flops over, utterly content",
            "hedgehog" => "uncurls and snuffles your fingers",
            _ => "looks very content",
        };
        self.say1(id, Line::col(Col::Green, format!("{} You pet {who}. {} {happy}.", a.icon, capitalise(&who))));
        self.tell_here(x, y, id, Line::col(Col::Green, format!("{} {name} pets {who}.", a.icon)));
        Ok(())
    }

    pub(super) fn cmd_let_go(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let Some((a, n)) = self.players.get_mut(&id).and_then(|p| p.companion.take()) else {
            self.say1(id, Line::plain("You don't have a companion to let go."));
            return Ok(());
        };
        self.world.db.set_companion(&name, None, None)?;
        let msg = match animal(&a) {
            Some(an) => format!("{} {} nuzzles you goodbye and wanders back to the wild. 💚", an.icon, capitalise(&label(an, &n))),
            None => "Your companion wanders back to the wild. 💚".to_string(),
        };
        self.say1(id, Line::col(Col::Green, msg));
        self.tell_here(x, y, id, Line::col(Col::Green, format!("{name}'s companion wanders back to the wild.")));
        Ok(())
    }
}
