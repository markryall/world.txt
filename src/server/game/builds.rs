//! Buildings: little houses made from what's in your satchel. `build` lists
//! what you could build, and `build a mushroom house` spends the things and
//! puts up scaffolding right where you stand. It goes up over a few real
//! minutes, with a bar in the side panel for whoever built it and anyone
//! nearby, then says it's finished, and the bar goes away a little later.
//!
//! Some things (a turret, a porch, a chimney) can only go next door to a
//! building, which makes it a bigger, more rambling sort of place. Whoever
//! built something can `demolish` it (it's `unmake`, so `undo` works too).
//!
//! Not in `help`'s front page: the scenery hints at it when your satchel is
//! full enough.

use super::Game;
use super::craft::{Need, need_label, plan_needs};
use super::progress::NEAR;
use crate::protocol::{Bar, Col, Line};
use crate::server::nature::Kind;
use crate::server::world::Trigger;
use Need::Any;

/// Something that can be built.
pub struct Plan {
    pub id: &'static str,
    /// "mushroom house", as in "build a mushroom house".
    pub name: &'static str,
    pub icon: &'static str,
    pub needs: &'static [(Need, i64)],
    /// How long it takes to go up, in seconds.
    pub secs: i64,
    /// Only goes next door to another building.
    pub extension: bool,
    /// What you see when you look at it.
    pub looks: &'static str,
    /// The scenery's nudge, when your satchel has enough for it.
    pub hint: &'static str,
}

pub const PLANS: &[Plan] = &[
    Plan {
        id: "mushroom_house",
        name: "mushroom house",
        icon: "🍄",
        needs: &[(Any(Kind::Mushroom), 3), (Any(Kind::Leaf), 1)],
        secs: 300,
        extension: false,
        looks: "A round little house with a spotty red roof, and a door just about big enough for you if you duck.",
        hint: "Your satchel has a good few mushrooms in it. Enough, with a leaf or a seed, to build a little mushroom house.",
    },
    Plan {
        id: "shell_cottage",
        name: "shell cottage",
        icon: "🐚",
        needs: &[(Any(Kind::Shell), 3), (Any(Kind::Stone), 1)],
        secs: 300,
        extension: false,
        looks: "A low cottage with shells pressed all over its walls. They click softly whenever the wind gets up.",
        hint: "All those shells in your satchel. You could build a whole shell cottage with them.",
    },
    Plan {
        id: "petal_pavilion",
        name: "petal pavilion",
        icon: "🌸",
        needs: &[(Any(Kind::Flower), 4)],
        secs: 240,
        extension: false,
        looks: "An open pavilion roofed with petals. It smells of summer, whatever the season.",
        hint: "Your satchel smells of flowers. There's enough in there to build a petal pavilion.",
    },
    Plan {
        id: "pebble_hut",
        name: "pebble hut",
        icon: "🪨",
        needs: &[(Any(Kind::Stone), 4)],
        secs: 300,
        extension: false,
        looks: "A sturdy hut of smooth pebbles, warm from the sun on one side and cool on the other.",
        hint: "Your satchel's heavy with stones. Enough to build a pebble hut, by the feel of it.",
    },
    Plan {
        id: "feather_aviary",
        name: "feather aviary",
        icon: "🪶",
        needs: &[(Any(Kind::Feather), 2), (Any(Kind::Leaf), 2)],
        secs: 240,
        extension: false,
        looks: "A tall, airy aviary woven from leaves and trimmed with feathers. Birds keep popping in to see who built it.",
        hint: "A couple of feathers and some leaves or seeds. You could build a little feather aviary with those.",
    },
    Plan {
        id: "berry_bakery",
        name: "berry bakery",
        icon: "🫐",
        needs: &[(Any(Kind::Berry), 3), (Any(Kind::Herb), 1)],
        secs: 300,
        extension: false,
        looks: "A tiny bakery with berry-stained shutters. Nobody's baking, but it smells as if somebody just was.",
        hint: "Berries and a herb (or a bit of honey) in your satchel. That's a berry bakery, if ever there was one.",
    },
    Plan {
        id: "fish_shack",
        name: "fish shack",
        icon: "🐟",
        needs: &[(Any(Kind::Fish), 2), (Any(Kind::Leaf), 2)],
        secs: 240,
        extension: false,
        looks: "A leaning shack thatched with leaves, with a hand-painted fish over the door. It's fine. It's mostly fine.",
        hint: "You've caught enough fish to build a fish shack. Leaves or seeds for the roof, fish for the sign.",
    },
    // Next door to a building only.
    Plan {
        id: "pebble_turret",
        name: "pebble turret",
        icon: "🏰",
        needs: &[(Any(Kind::Stone), 3)],
        secs: 180,
        extension: true,
        looks: "A little pebble turret, just tall enough to see over the roof next door.",
        hint: "The building next door looks like it could do with a turret. A few stones would do it.",
    },
    Plan {
        id: "leafy_porch",
        name: "leafy porch",
        icon: "🌿",
        needs: &[(Any(Kind::Leaf), 3)],
        secs: 120,
        extension: true,
        looks: "A shady porch roofed with leaves, with room for two to sit and watch the weather.",
        hint: "A porch would suit the building next door. Three leaves or seeds would roof one.",
    },
    Plan {
        id: "toadstool_chimney",
        name: "toadstool chimney",
        icon: "🍄",
        needs: &[(Any(Kind::Mushroom), 2)],
        secs: 120,
        extension: true,
        looks: "A crooked toadstool chimney. Now and then it puffs out a smoke ring, for no reason at all.",
        hint: "The building next door hasn't got a chimney. A couple of mushrooms would make a toadstool one.",
    },
    Plan {
        id: "shell_bell_tower",
        name: "shell bell tower",
        icon: "🔔",
        needs: &[(Any(Kind::Shell), 2), (Any(Kind::Feather), 1)],
        secs: 180,
        extension: true,
        looks: "A slim bell tower with a big shell for a bell. The wind rings it, very quietly.",
        hint: "Next door could use a bell tower. A shell or two (or anything from the seashore) and a feather would do.",
    },
    Plan {
        id: "flower_arch",
        name: "flower arch",
        icon: "🌼",
        needs: &[(Any(Kind::Flower), 3)],
        secs: 120,
        extension: true,
        looks: "An arch of flowers leading up to the building next door. You can't help walking through it.",
        hint: "A flower arch would make a lovely way in to the building next door.",
    },
];

/// How long a finished building's bar stays up.
const LINGER: i64 = 120;
/// Buildings one person can have going up at once.
const AT_ONCE: usize = 2;
pub const KIND: &str = "building";

pub fn plan_by_id(id: &str) -> Option<&'static Plan> {
    PLANS.iter().find(|p| p.id == id)
}

/// Which plan someone means: "a mushroom house", "Mushroom House", or (when
/// `loose`, for `build`) just "turret".
pub fn plan_named(words: &str, loose: bool) -> Option<&'static Plan> {
    let w = crate::server::world::strip_article(words.trim()).to_lowercase();
    if w.is_empty() {
        return None;
    }
    PLANS
        .iter()
        .find(|p| p.name == w)
        .or_else(|| PLANS.iter().find(|p| w.contains(p.name)))
        .or_else(|| {
            PLANS
                .iter()
                .find(|p| loose && w.len() >= 4 && p.name.contains(w.as_str()))
        })
}

/// A building that's going up, or has gone up.
pub struct Build {
    plan: &'static Plan,
    by: String,
    started: i64,
    ready: i64,
    told: bool,
}

fn now() -> i64 {
    super::boards::now()
}

fn minutes(secs: i64) -> String {
    match (secs + 59) / 60 {
        ..=1 => "about a minute".into(),
        m => format!("about {m} minutes"),
    }
}

const STEPS: [(i64, i64, &str); 8] = [
    (0, -1, "north"),
    (1, -1, "north-east"),
    (1, 0, "east"),
    (1, 1, "south-east"),
    (0, 1, "south"),
    (-1, 1, "south-west"),
    (-1, 0, "west"),
    (-1, -1, "north-west"),
];

impl Game {
    /// A building that's finished going up.
    pub(super) fn built(&self, thing: i64) -> bool {
        self.builds.get(&thing).is_some_and(|b| now() >= b.ready)
    }
}

impl Game {
    pub(super) fn load_buildings(&mut self) {
        match self.world.db.buildings() {
            Ok(rows) => {
                for (thing, plan, by, started, ready, told) in rows {
                    if let Some(plan) = plan_by_id(&plan) {
                        self.builds.insert(
                            thing,
                            Build {
                                plan,
                                by,
                                started,
                                ready,
                                told,
                            },
                        );
                    }
                }
            }
            Err(e) => eprintln!("loading buildings: {e}"),
        }
    }

    /// The building standing at a spot: (thing id, its plan).
    fn building_at(&self, x: i64, y: i64) -> Option<(i64, &'static Plan)> {
        self.world
            .things_at(x, y)
            .find_map(|t| self.builds.get(&t.id).map(|b| (t.id, b.plan)))
    }

    /// Buildings next door to a spot: (plan, which way).
    fn next_door(&self, x: i64, y: i64) -> Vec<(&'static Plan, &'static str)> {
        STEPS
            .iter()
            .filter_map(|&(dx, dy, way)| self.building_at(x + dx, y + dy).map(|(_, p)| (p, way)))
            .collect()
    }

    /// `build` on its own: what you could build, and whether you've the things for it.
    pub(super) fn cmd_blueprints(&mut self, id: u64) -> anyhow::Result<()> {
        let user = self.user(id);
        let satchel = self.world.db.satchel(&user)?;
        let mut lines = vec![Line::new().b(Col::Magenta, "🏗️ Things you could build")];
        for (extensions, heading) in [
            (false, None),
            (true, Some("  …and next door to a building:")),
        ] {
            if let Some(h) = heading {
                lines.push(Line::col(Col::Dim, h));
            }
            for p in PLANS.iter().filter(|p| p.extension == extensions) {
                let ready = plan_needs(p.needs, &satchel).is_some();
                let needs: Vec<String> = p.needs.iter().map(|(n, c)| need_label(*n, *c)).collect();
                lines.push(
                    Line::new()
                        .c(
                            if ready { Col::Green } else { Col::Dim },
                            if ready { "  ✓ " } else { "    " },
                        )
                        .t(format!("{} ", p.icon))
                        .b(if ready { Col::Yellow } else { Col::Plain }, p.name)
                        .c(
                            Col::Dim,
                            format!(
                                "  ← {} · {}",
                                needs.join(" + "),
                                minutes(p.secs).trim_start_matches("about ")
                            ),
                        ),
                );
            }
        }
        lines.push(Line::col(Col::Dim, "  build a mushroom house to put one up right here. Whoever builds one can demolish it."));
        self.send(id, lines);
        Ok(())
    }

    /// `build a mushroom house`: spend the things and start it going up. None
    /// if that isn't something to build (so it's an ordinary `make`).
    pub(super) fn cmd_build(
        &mut self,
        id: u64,
        verb: &str,
        what: &str,
    ) -> Option<anyhow::Result<()>> {
        let plan = plan_named(what, matches!(verb, "build" | "construct"))?;
        Some(self.build(id, plan))
    }

    fn build(&mut self, id: u64, plan: &'static Plan) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else {
            return Ok(());
        };
        if crate::server::world::room_of(x, y).is_some() {
            self.say1(
                id,
                Line::plain("Buildings go up outdoors. (out to go outside)"),
            );
            return Ok(());
        }
        // The same twelve-things-a-spot as making or placing anything.
        if self.world.things_at(x, y).count() >= 12 {
            self.say1(
                id,
                Line::plain("It's getting crowded here. Try somewhere else?"),
            );
            return Ok(());
        }
        let user = self.user(id);
        if !self.world.terrain(x, y).walkable() || self.world.terrain(x, y).is_wet() {
            self.say1(id, Line::plain("You'd want dry ground under it."));
            return Ok(());
        }
        if let Some((_, there)) = self.building_at(x, y) {
            self.say1(
                id,
                Line::plain(format!(
                    "There's already {} here. Build next door to make it bigger.",
                    crate::server::nature::a(there.name)
                )),
            );
            return Ok(());
        }
        if plan.extension && self.next_door(x, y).is_empty() {
            self.say1(
                id,
                Line::plain(format!(
                    "{} {} goes next door to a building. Stand beside one and try again.",
                    capital(&crate::server::nature::a(plan.name)),
                    plan.icon
                )),
            );
            return Ok(());
        }
        let t = now();
        let going_up = self
            .builds
            .iter()
            .filter(|(thing, b)| {
                b.by == user && b.ready > t && self.world.things.iter().any(|x| x.id == **thing)
            })
            .count();
        if going_up >= AT_ONCE {
            self.say1(
                id,
                Line::plain("You've already got two things going up. Let one finish first."),
            );
            return Ok(());
        }
        let satchel = self.world.db.satchel(&user)?;
        let Some(used) = plan_needs(plan.needs, &satchel) else {
            let needs: Vec<String> = plan.needs.iter().map(|(n, c)| need_label(*n, *c)).collect();
            self.say1(
                id,
                Line::plain(format!(
                    "{} {} needs {}. Keep gathering! 🧺 (build lists them all)",
                    capital(&crate::server::nature::a(plan.name)),
                    plan.icon,
                    needs.join(" + ")
                )),
            );
            return Ok(());
        };
        for (sid, n) in &used {
            self.world.db.satchel_add(&user, sid, -n)?;
        }
        let name = crate::server::nature::a(plan.name);
        let thing =
            self.world
                .add_thing(x, y, name.clone(), None, Trigger::Always, &user, Some(KIND))?;
        let ready = t + plan.secs;
        self.world
            .db
            .add_building(thing, plan.id, &user, t, ready)?;
        self.builds.insert(
            thing,
            Build {
                plan,
                by: user,
                started: t,
                ready,
                told: false,
            },
        );
        let place = self.world.place_title(x, y);
        self.say1(
            id,
            Line::new().c(
                Col::Yellow,
                format!(
                    "🏗️ You start building {name} {} here. It'll be ready in {}.",
                    plan.icon,
                    minutes(plan.secs)
                ),
            ),
        );
        self.say1(
            id,
            Line::col(
                Col::Dim,
                "Off you go if you like: the bar in your side panel keeps track.",
            ),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Yellow,
                format!("🏗️ {who} starts building {name} {} at {place}.", plan.icon),
            ),
        );
        self.builds_changed(thing);
        self.status_all();
        Ok(())
    }

    /// The bars for buildings going up (or just finished) that `id` can see:
    /// their own wherever they are, and anyone's nearby.
    pub(super) fn build_bars_for(&self, id: u64) -> Vec<Bar> {
        let Some(p) = self.players.get(&id) else {
            return vec![];
        };
        let t = now();
        self.builds
            .iter()
            .filter(|(_, b)| t < b.ready + LINGER)
            .filter_map(|(thing, b)| {
                let at = self.world.things.iter().find(|x| x.id == *thing)?;
                let near = (at.x - p.x).abs() <= NEAR && (at.y - p.y).abs() <= NEAR;
                (near || b.by == p.user).then(|| {
                    let span = (b.ready - b.started).max(1);
                    let done = ((t - b.started).clamp(0, span) * 100 / span) as u64;
                    Bar {
                        label: format!("{} {}", b.plan.icon, b.plan.name),
                        done,
                        total: 100,
                        place: self.world.place_title(at.x, at.y),
                    }
                })
            })
            .collect()
    }

    /// A building's bar has moved on: freshen its builder's side panel and
    /// those of anyone nearby.
    fn builds_changed(&self, thing: i64) {
        let (Some(b), Some(at)) = (
            self.builds.get(&thing),
            self.world.things.iter().find(|x| x.id == thing),
        ) else {
            return;
        };
        for p in self.players.values() {
            if p.user == b.by || ((at.x - p.x).abs() <= NEAR && (at.y - p.y).abs() <= NEAR) {
                p.status_due.set(true);
            }
        }
    }

    /// Every tick: move the bars along, and say when something's finished.
    pub(super) fn builds_tick(&mut self) {
        let t = now();
        let live: Vec<i64> = self
            .builds
            .iter()
            .filter(|(_, b)| t < b.ready + LINGER + 5)
            .map(|(&k, _)| k)
            .collect();
        for thing in live {
            if !self.world.things.iter().any(|x| x.id == thing) {
                continue;
            }
            self.builds_changed(thing);
            let Some(b) = self.builds.get_mut(&thing) else {
                continue;
            };
            if b.told || t < b.ready {
                continue;
            }
            b.told = true;
            let (plan, by) = (b.plan, b.by.clone());
            let _ = self.world.db.building_told(thing);
            self.finished(thing, plan, &by);
        }
    }

    fn finished(&mut self, thing: i64, plan: &'static Plan, by: &str) {
        let Some((x, y)) = self
            .world
            .things
            .iter()
            .find(|t| t.id == thing)
            .map(|t| (t.x, t.y))
        else {
            return;
        };
        let place = self.world.place_title(x, y);
        let builder = self.world.nick(by);
        self.happened(by, super::since::BUILT, x, y, plan.name);
        let ids: Vec<(u64, bool)> = self
            .players
            .iter()
            .filter(|(_, p)| p.user == by || ((p.x - x).abs() <= NEAR && (p.y - y).abs() <= NEAR))
            .map(|(&id, p)| (id, p.user == by))
            .collect();
        for (id, mine) in ids {
            let line = if mine {
                Line::new()
                    .c(
                        Col::Yellow,
                        format!("{} Your {} at {place} is finished! ", plan.icon, plan.name),
                    )
                    .c(
                        Col::Dim,
                        "The scaffolding comes down with a satisfying clatter.",
                    )
            } else {
                Line::new()
                    .c(
                        Col::Yellow,
                        format!(
                            "{} {builder}'s {} at {place} is finished! ",
                            plan.icon, plan.name
                        ),
                    )
                    .c(
                        Col::Dim,
                        "The scaffolding comes down with a satisfying clatter.",
                    )
            };
            self.say1(id, line);
        }
    }

    /// Looking at a spot with a building on it: what it's like, whether it's
    /// done, and what's next door.
    pub(super) fn building_lines_here(&self, id: u64) -> Vec<Line> {
        let Some(p) = self.players.get(&id) else {
            return vec![];
        };
        let (x, y) = (p.x, p.y);
        let Some((thing, plan)) = self.building_at(x, y) else {
            return vec![];
        };
        let Some(b) = self.builds.get(&thing) else {
            return vec![];
        };
        let t = now();
        let mut lines = Vec::new();
        if t < b.ready {
            lines.push(Line::col(
                Col::Sand,
                format!(
                    "🏗️ Scaffolding stands around a half-built {} {}. Still {} to go.",
                    plan.name,
                    plan.icon,
                    minutes(b.ready - t)
                ),
            ));
        } else {
            lines.push(Line::col(
                Col::Sand,
                format!("{} {}", plan.icon, plan.looks),
            ));
        }
        let doors = self.next_door(x, y);
        if !doors.is_empty() {
            let parts: Vec<String> = doors
                .iter()
                .map(|(p, way)| format!("{} to the {way}", crate::server::nature::a(p.name)))
                .collect();
            lines.push(Line::col(
                Col::Dim,
                format!("Next door: {}.", crate::server::world::join_and(&parts)),
            ));
        }
        lines
    }

    /// For the scenery: a nudge towards something you could build here.
    pub(super) fn build_hints(&self, id: u64) -> Vec<&'static str> {
        let Some(p) = self.players.get(&id) else {
            return vec![];
        };
        let (x, y) = (p.x, p.y);
        let terrain = self.world.terrain(x, y);
        if !terrain.walkable() || terrain.is_wet() || self.building_at(x, y).is_some() {
            return vec![];
        }
        let satchel = self.world.db.satchel(&p.user).unwrap_or_default();
        let beside = !self.next_door(x, y).is_empty();
        PLANS
            .iter()
            .filter(|plan| plan.extension == beside && plan_needs(plan.needs, &satchel).is_some())
            .map(|plan| plan.hint)
            .collect()
    }
}

fn capital(s: &str) -> String {
    crate::server::world::capitalise(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_are_small_and_findable() {
        for p in PLANS {
            let n: i64 = p.needs.iter().map(|(_, c)| c).sum();
            assert!((2..=5).contains(&n), "{} needs {n} things", p.name);
            assert_eq!(
                plan_named(&format!("a {}", p.name), false).map(|f| f.id),
                Some(p.id)
            );
        }
        assert_eq!(
            plan_named("Mushroom House", false).map(|p| p.id),
            Some("mushroom_house")
        );
        assert_eq!(
            plan_named("turret", true).map(|p| p.id),
            Some("pebble_turret")
        );
        assert!(
            plan_named("a fish", false).is_none(),
            "make a fish is just a fish"
        );
        assert!(plan_named("a little blue bench", true).is_none());
        assert!(plan_named("a frog that croaks", true).is_none());
    }

    #[test]
    fn exact_needs_are_real_finds() {
        for p in PLANS {
            for (need, _) in p.needs {
                if let Need::Exactly(id) = need {
                    assert!(crate::server::nature::by_id(id).is_some(), "{id}");
                }
            }
        }
    }

    #[test]
    fn minutes_read_nicely() {
        assert_eq!(minutes(30), "about a minute");
        assert_eq!(minutes(300), "about 5 minutes");
        assert_eq!(minutes(241), "about 5 minutes");
    }
}
