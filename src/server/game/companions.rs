//! Companions: wild animals turn up near people now and then. Offer one a
//! berry and it may decide you're friends, then it follows you everywhere.
//!
//! Until then they lead lives of their own: they wander their kind of ground
//! a step at a time, nose at whatever people have left about, size each other
//! up, and put up with a pat, mostly. None of it is in the help.

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
    /// How it gets about: "trots".
    moves: &'static str,
    /// Seconds between steps, at the quickest and slowest.
    pace: (f64, f64),
    /// Patted while it's still wild.
    patted: &'static [&'static str],
    /// What it gets up to on its own.
    busy: &'static [&'static str],
}

pub const ANIMALS: &[Animal] = &[
    Animal {
        id: "fox",
        name: "fox",
        icon: "🦊",
        ground: &[T::Forest],
        night: None,
        here: "peeks out from behind a tree, ears twitching",
        tricks: &[
            "sniffs the air",
            "pounces on a leaf",
            "curls its tail around your feet",
            "yips softly",
        ],
        moves: "trots",
        pace: (20.0, 50.0),
        patted: &[
            "holds very still and lets you, just once, then skitters back a step",
            "sniffs your fingers and decides you'll do",
            "ducks your hand and gives you a look",
        ],
        busy: &[
            "digs at something only it can smell",
            "pounces on a beetle and misses",
            "sits and washes one paw, very thoroughly",
            "stares into the trees at nothing at all",
            "curls up in a ball with its tail over its nose",
        ],
    },
    Animal {
        id: "duckling",
        name: "duckling",
        icon: "🐥",
        ground: &[T::Pond, T::Water],
        night: Some(false),
        here: "paddles about, peeping",
        tricks: &[
            "peeps happily",
            "waddles in a little circle",
            "shakes water off its fluffy feathers",
        ],
        moves: "paddles",
        pace: (15.0, 40.0),
        patted: &[
            "peeps and lets you stroke its fluffy head",
            "paddles off in a huff, then paddles back",
        ],
        busy: &[
            "upends itself to look for weed, tail in the air",
            "chases a dragonfly and gives up",
            "peeps at its own reflection",
            "has a tremendous splashy bath",
        ],
    },
    Animal {
        id: "rabbit",
        name: "rabbit",
        icon: "🐇",
        ground: &[T::Meadow, T::Grass],
        night: Some(false),
        here: "sits up on its back legs, nose twitching",
        tricks: &[
            "nibbles some clover",
            "does a happy little hop",
            "twitches its nose at you",
        ],
        moves: "hops",
        pace: (12.0, 35.0),
        patted: &[
            "freezes, allows exactly one stroke, then flicks its ears",
            "thumps a back foot, but stays put",
            "flops onto its side, utterly unbothered",
        ],
        busy: &[
            "nibbles the clover, ears swivelling",
            "does a sudden twisting leap for no reason at all",
            "washes its face with both paws",
            "sits up tall, keeping watch",
        ],
    },
    Animal {
        id: "hedgehog",
        name: "hedgehog",
        icon: "🦔",
        ground: &[T::Forest, T::Grass, T::Meadow],
        night: Some(true),
        here: "snuffles through the leaves",
        tricks: &[
            "snuffles in the leaves",
            "curls into a prickly ball, then uncurls",
            "trundles along beside you",
        ],
        moves: "trundles",
        pace: (30.0, 70.0),
        patted: &[
            "curls into a prickly ball. Careful!",
            "snuffles your fingers, then trundles on as if nothing happened",
        ],
        busy: &[
            "snuffles noisily through the leaves",
            "finds a worm and is very pleased about it",
            "stops, sniffs, and changes its mind about where it was going",
            "grunts contentedly to itself",
        ],
    },
    Animal {
        id: "turtle",
        name: "turtle",
        icon: "🐢",
        ground: &[T::Sand],
        night: None,
        here: "blinks at you from the sand",
        tricks: &[
            "blinks slowly",
            "pokes its head out to look around",
            "trundles very, very slowly after you",
        ],
        moves: "plods",
        pace: (60.0, 140.0),
        patted: &[
            "closes its eyes, which might mean it likes it",
            "pulls its head in, then pokes it out again",
        ],
        busy: &[
            "blinks at the sea for a long time",
            "digs a small hole, then sits in it",
            "nibbles a strand of seaweed, slowly",
        ],
    },
];

pub fn animal(id: &str) -> Option<&'static Animal> {
    ANIMALS.iter().find(|a| a.id == id)
}

pub struct Wild {
    pub animal: &'static Animal,
    pub x: i64,
    pub y: i64,
    pub until: Instant,
    /// When it next takes a step.
    pub next_step: Instant,
    /// When it last did something someone saw, so it doesn't go on and on.
    pub last_seen_doing: Instant,
}

/// The way from one spot to the next: "north", or "north-east".
fn way(dx: i64, dy: i64) -> &'static str {
    DIRS.iter()
        .find(|q| q.2 == dx.signum() && q.3 == dy.signum())
        .map_or("nearby", |q| q.1)
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
        let spots: Vec<(u64, i64, i64)> = self
            .players
            .iter()
            .filter(|(_, p)| p.about())
            .map(|(&id, p)| (id, p.x, p.y))
            .collect();
        for (id, px, py) in spots {
            let near = self
                .wild
                .iter()
                .any(|w| (w.x - px).abs() <= 6 && (w.y - py).abs() <= 4);
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
            let until = now + Duration::from_secs(self.rng.range(360.0, 720.0) as u64);
            let next_step = now + Duration::from_secs_f64(self.rng.range(a.pace.0, a.pace.1));
            self.wild.push(Wild {
                animal: a,
                x,
                y,
                until,
                next_step,
                last_seen_doing: now,
            });
            let d = dx.abs().max(dy.abs());
            let line = if d == 0 {
                format!("{} A {} {} right here!", a.icon, a.name, a.here)
            } else {
                format!(
                    "{} A {} {}, just to the {}.",
                    a.icon,
                    a.name,
                    a.here,
                    way(dx, dy)
                )
            };
            self.say1(id, Line::col(Col::Green, line));
        }

        self.wild_wander(now);
        self.wild_busy(now);

        // Companions get up to little things.
        let pets: Vec<(i64, i64, String)> = self
            .players
            .values()
            .filter_map(|p| p.companion.as_ref().map(|(a, n)| (p.x, p.y, a, n)))
            .filter_map(|(x, y, a, n)| animal(a).map(|an| (x, y, an, n.clone())))
            .filter_map(|(x, y, an, n)| {
                if self.rng.chance(1.0 / 25.0) {
                    let trick = *self.rng.pick(an.tricks);
                    Some((
                        x,
                        y,
                        format!("{} {} {trick}.", an.icon, capitalise(&label(an, &n))),
                    ))
                } else {
                    None
                }
            })
            .collect();
        for (x, y, line) in pets {
            self.tell_watchers(x, y, Line::col(Col::Green, line));
        }
    }

    /// Each wild animal ambles about its own kind of ground, now and then
    /// drawn towards whoever's close by. People at either end see it go.
    fn wild_wander(&mut self, now: Instant) {
        let night = Clock::phase() == Phase::Night;
        for i in 0..self.wild.len() {
            if self.wild[i].next_step > now {
                continue;
            }
            let (a, x, y) = (self.wild[i].animal, self.wild[i].x, self.wild[i].y);
            self.wild[i].next_step =
                now + Duration::from_secs_f64(self.rng.range(a.pace.0, a.pace.1));
            if self.rng.chance(0.35) {
                continue; // happy where it is, for now
            }
            // Curious: sometimes it heads for the nearest person within a few steps.
            let near = self
                .players
                .values()
                .filter(|p| {
                    p.about() && (p.x, p.y) != (x, y) && (p.x - x).abs().max((p.y - y).abs()) <= 3
                })
                .map(|p| ((p.x - x).signum(), (p.y - y).signum()))
                .next();
            let fits = |g: &Self, dx: i64, dy: i64| {
                a.ground.contains(&g.world.terrain(x + dx, y + dy))
                    && g.world.home_at(x + dx, y + dy).is_none()
            };
            let step = match near {
                Some((dx, dy)) if self.rng.chance(0.4) && fits(self, dx, dy) => Some((dx, dy)),
                _ => {
                    let options: Vec<(i64, i64)> = DIRS
                        .iter()
                        .map(|d| (d.2, d.3))
                        .filter(|&(dx, dy)| fits(self, dx, dy))
                        .collect();
                    (!options.is_empty()).then(|| *self.rng.pick(&options))
                }
            };
            let Some((dx, dy)) = step else { continue };
            let (nx, ny) = (x + dx, y + dy);
            self.wild[i].x = nx;
            self.wild[i].y = ny;
            // Night creatures turn in at dawn, day ones at dusk: it just doesn't come back.
            if a.night.is_some_and(|n| n != night) {
                self.wild[i].until = now;
            }
            let the = format!(
                "{} The {} {} off to the {}.",
                a.icon,
                a.name,
                a.moves,
                way(dx, dy)
            );
            self.tell_watchers(x, y, Line::col(Col::Green, the));
            let arrives = format!(
                "{} A {} {} in from the {}.",
                a.icon,
                a.name,
                a.moves,
                way(-dx, -dy)
            );
            self.tell_watchers(nx, ny, Line::col(Col::Green, arrives));
        }
    }

    /// With someone watching, a wild animal gets on with its day: its own
    /// business, a nose at what's been left about, or the other animals.
    fn wild_busy(&mut self, now: Instant) {
        for i in 0..self.wild.len() {
            let (a, x, y) = (self.wild[i].animal, self.wild[i].x, self.wild[i].y);
            let watched = self
                .players
                .values()
                .any(|p| (p.x, p.y) == (x, y) && p.about());
            if !watched
                || now.duration_since(self.wild[i].last_seen_doing) < Duration::from_secs(45)
                || !self.rng.chance(1.0 / 12.0)
            {
                continue;
            }
            self.wild[i].last_seen_doing = now;
            let others: Vec<&'static Animal> = self
                .wild
                .iter()
                .enumerate()
                .filter(|(j, w)| *j != i && (w.x, w.y) == (x, y))
                .map(|(_, w)| w.animal)
                .collect();
            let pets: Vec<String> = self
                .players
                .values()
                .filter(|p| (p.x, p.y) == (x, y))
                .filter_map(|p| {
                    p.companion
                        .as_ref()
                        .and_then(|(c, n)| animal(c).map(|an| (an, n.clone())))
                })
                .map(|(an, n)| {
                    if an.id == a.id {
                        format!("same:{}", label(an, &n))
                    } else {
                        label(an, &n)
                    }
                })
                .collect();
            let things: Vec<String> = self.world.things_at(x, y).map(|t| t.name.clone()).collect();
            let roll = self.rng.next() % 4;
            let line = if roll == 0 && !others.is_empty() {
                let b = *self.rng.pick(&others);
                let how = *self.rng.pick(&[
                    "eye each other warily",
                    "touch noses, very briefly",
                    "pretend not to have noticed each other",
                    "have a quiet stand-off over the best spot",
                ]);
                format!("{} The {} and the {} {how}.", a.icon, a.name, b.name)
            } else if roll == 1 && !pets.is_empty() {
                let pet = self.rng.pick(&pets).clone();
                match pet.strip_prefix("same:") {
                    Some(p) => format!(
                        "{} The wild {} and {p} touch noses, then circle each other, tails up.",
                        a.icon, a.name
                    ),
                    None => format!("{} The {} keeps a careful eye on {pet}.", a.icon, a.name),
                }
            } else if roll == 2 && !things.is_empty() {
                let t = super::the(&self.rng.pick(&things).clone());
                let how = *self.rng.pick(&[
                    "sniffs at {t}",
                    "gives {t} a thorough inspection",
                    "sits down beside {t} as if it's always been there",
                    "peers at {t}, then backs off, unsure",
                ]);
                format!("{} The {} {}.", a.icon, a.name, how.replace("{t}", &t))
            } else {
                format!("{} The {} {}.", a.icon, a.name, self.rng.pick(a.busy))
            };
            self.tell_watchers(x, y, Line::col(Col::Green, line));
        }
    }

    /// For `look`: wild animals here, and everyone's companions.
    pub(super) fn animal_lines(&self, id: u64) -> Vec<Line> {
        let Some(me) = self.players.get(&id) else {
            return vec![];
        };
        let mut out = Vec::new();
        for w in self.wild.iter().filter(|w| (w.x, w.y) == (me.x, me.y)) {
            out.push(
                Line::new()
                    .c(
                        Col::Green,
                        format!("{} A {} {}.", w.animal.icon, w.animal.name, w.animal.here),
                    )
                    .c(
                        Col::Dim,
                        format!(" (befriend {}: it loves berries)", w.animal.name),
                    ),
            );
        }
        for p in self.players.values().filter(|p| (p.x, p.y) == (me.x, me.y)) {
            let Some((a, n)) = &p.companion else { continue };
            let Some(an) = animal(a) else { continue };
            let line = if p.user == me.user {
                format!(
                    "{} {} is right by your side.",
                    an.icon,
                    capitalise(&label(an, n))
                )
            } else {
                format!(
                    "{} {}, {}'s {}, is here.",
                    an.icon,
                    n.clone().unwrap_or_else(|| format!("A {}", an.name)),
                    p.name,
                    an.name
                )
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
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let what = crate::server::world::strip_article(what.trim()).to_lowercase();
        let Some(i) = self.wild.iter().position(|w| {
            (w.x, w.y) == (x, y) && (what.is_empty() || w.animal.name.contains(&what))
        }) else {
            self.say1(id, Line::plain("There's no wild animal here to befriend. Keep wandering and they'll turn up! 🦊🐇🦔"));
            return Ok(());
        };
        if let Some((a, n)) = self.players.get(&id).and_then(|p| p.companion.clone()) {
            let current = animal(&a).map_or("friend".to_string(), |an| label(an, &n));
            self.say1(
                id,
                Line::plain(format!(
                    "You already have {current}. (let go of them first, if you're sure)"
                )),
            );
            return Ok(());
        }
        let a = self.wild[i].animal;
        let berries: Vec<(&'static nature::Species, i64)> = self
            .world
            .db
            .satchel(&user)?
            .into_iter()
            .filter_map(|(sid, n)| nature::by_id(&sid).map(|s| (s, n)))
            .filter(|(s, _)| s.kind == Kind::Berry)
            .collect();
        let Some((berry, _)) = berries.iter().max_by_key(|(_, n)| *n) else {
            self.say1(
                id,
                Line::plain(format!(
                    "The {} looks hungry. {}s love berries: gather some first! 🫐",
                    a.name,
                    capitalise(a.name)
                )),
            );
            return Ok(());
        };
        self.world.db.satchel_add(&user, berry.id, -1)?;
        if !self.rng.chance(0.5) {
            self.say1(
                id,
                Line::col(
                    Col::Green,
                    format!(
                        "{} The {} gobbles up your {} and watches you carefully. Maybe one more?",
                        a.icon, a.name, berry.name
                    ),
                ),
            );
            return Ok(());
        }
        self.wild.remove(i);
        self.world.db.set_companion(&user, Some(a.id), None)?;
        if let Some(p) = self.players.get_mut(&id) {
            p.companion = Some((a.id.to_string(), None));
        }
        self.say1(id, Line::new().b(Col::Green, format!("{} The {} eats your {} and decides you're friends! It'll follow you everywhere.", a.icon, a.name, berry.name)));
        self.say1(
            id,
            Line::col(Col::Dim, "Give it a name: petname Rusty · pet · let go"),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Green,
                format!("{} {name} makes friends with a {}!", a.icon, a.name),
            ),
        );
        Ok(())
    }

    pub(super) fn cmd_nickname(&mut self, id: u64, new: &str) -> anyhow::Result<()> {
        let Some((_, _, _name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let Some((a, _)) = self.players.get(&id).and_then(|p| p.companion.clone()) else {
            self.say1(
                id,
                Line::plain("You don't have a companion yet. Befriend an animal with a berry!"),
            );
            return Ok(());
        };
        let nick: String = new
            .trim()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
            .take(20)
            .collect();
        if nick.trim().is_empty() {
            self.say1(id, Line::plain("Call them what? e.g. 'petname Rusty'"));
            return Ok(());
        }
        let nick = capitalise(nick.trim());
        self.world.db.set_companion(&user, Some(&a), Some(&nick))?;
        if let Some(p) = self.players.get_mut(&id) {
            p.companion = Some((a.clone(), Some(nick.clone())));
        }
        let icon = animal(&a).map_or("💚", |an| an.icon);
        self.say1(
            id,
            Line::col(
                Col::Green,
                format!("{icon} {nick} it is! They look very pleased."),
            ),
        );
        Ok(())
    }

    pub(super) fn cmd_pet(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        // A wild one here, if that's who they mean (or they've no companion to pat).
        let what = crate::server::world::strip_article(what.trim()).to_lowercase();
        let mine = self
            .players
            .get(&id)
            .and_then(|p| p.companion.as_ref())
            .and_then(|(a, n)| animal(a).map(|an| label(an, n).to_lowercase()));
        let means_mine = mine
            .as_ref()
            .is_some_and(|m| what.is_empty() || m.contains(&what));
        let wild = self.wild.iter().position(|w| {
            (w.x, w.y) == (x, y) && (what.is_empty() || w.animal.name.contains(&what))
        });
        if let (Some(i), false) = (wild, means_mine) {
            let a = self.wild[i].animal;
            let how = *self.rng.pick(a.patted);
            let stop = if how.ends_with('!') { "" } else { "." };
            self.say1(
                id,
                Line::col(
                    Col::Green,
                    format!("{} You reach out to the {}. It {how}{stop}", a.icon, a.name),
                ),
            );
            self.tell_here(
                x,
                y,
                id,
                Line::col(
                    Col::Green,
                    format!("{} {name} reaches out to pat the {}.", a.icon, a.name),
                ),
            );
            return Ok(());
        }
        let Some(an) = self
            .players
            .get(&id)
            .and_then(|p| p.companion.as_ref())
            .and_then(|(a, n)| animal(a).map(|an| (an, n.clone())))
        else {
            self.say1(
                id,
                Line::plain(
                    "There's nobody here who wants a pat. Befriend an animal with a berry!",
                ),
            );
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
        self.say1(
            id,
            Line::col(
                Col::Green,
                format!("{} You pet {who}. {} {happy}.", a.icon, capitalise(&who)),
            ),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(Col::Green, format!("{} {name} pets {who}.", a.icon)),
        );
        // The pat chip can rest a while: freshen the panel now.
        if let Some(p) = self.players.get(&id) {
            self.patted
                .insert(p.user.clone(), std::time::Instant::now());
            p.status_due.set(true);
        }
        Ok(())
    }

    pub(super) fn cmd_let_go(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let Some((a, n)) = self.players.get_mut(&id).and_then(|p| p.companion.take()) else {
            self.say1(id, Line::plain("You don't have a companion to let go."));
            return Ok(());
        };
        self.world.db.set_companion(&user, None, None)?;
        let msg = match animal(&a) {
            Some(an) => format!(
                "{} {} nuzzles you goodbye and wanders back to the wild. 💚",
                an.icon,
                capitalise(&label(an, &n))
            ),
            None => "Your companion wanders back to the wild. 💚".to_string(),
        };
        self.say1(id, Line::col(Col::Green, msg));
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Green,
                format!("{name}'s companion wanders back to the wild."),
            ),
        );
        Ok(())
    }
}
