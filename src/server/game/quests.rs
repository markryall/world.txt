//! Little quests, never chores. The island folk now and then lose something
//! and mention it to whoever's about: find it, give it back, and they say
//! thank you with something for your home. And now and then something
//! comes apart, its pieces scattered about an island: whoever finds a piece
//! sets it down where it belongs, at any time, and when the last one's in
//! it comes whole, for everyone who helped.
//!
//! No timers to see, no failing, no list to keep up with. An errand quietly
//! goes when the folk who asked rows off; pieces quietly go after a few
//! days. Nothing is said either way. None of it is in the help.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::islands::{ISLANDS, Island, REGION, island_at};
use crate::server::nature::{self, Species};
use crate::server::quest_items::{self as items, Set};
use crate::server::terrain::Terrain as T;
use crate::server::weather::Clock;
use crate::server::world::{DIRS, capitalise, in_room_space, join_and};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const META: &str = "quests";
const HOUR: i64 = 3600;
/// After being asked, a while before anyone asks you again.
const ERRAND_REST: i64 = 4 * HOUR;
/// The folk only ask with long enough left on the island to get to it.
const ERRAND_NEEDS: i64 = 2 * HOUR;
/// How long pieces wait to be found before they fade.
const PIECES_LAST: i64 = 3 * 24 * HOUR;
/// Between one set of pieces and the next.
const PIECES_REST: (f64, f64) = (12.0 * 3600.0, 36.0 * 3600.0);
/// Hidden within this many steps of a named place, so a clue means something.
const HIDE_NEAR: i64 = 8;
/// Close enough to catch a glint.
const GLINT: i64 = 2;
/// Where things get hidden.
const HIDE_ON: &[T] = &[T::Grass, T::Meadow, T::Forest, T::Sand, T::Path, T::Hill];

/// For the recap: something that came together.
pub const WHOLE: &str = "whole";

#[derive(Serialize, Deserialize, Default)]
pub struct Quests {
    errands: Vec<Errand>,
    #[serde(default)]
    pieces: Option<Pieces>,
    #[serde(default)]
    next_pieces: i64,
    /// When each person (by username) was last asked.
    #[serde(default)]
    asked: HashMap<String, i64>,
}

/// Someone's been asked to look for something.
#[derive(Serialize, Deserialize, Clone)]
struct Errand {
    user: String,
    folk: String,
    item: String,
    /// Where it's lying, until it's picked up.
    at: Option<(i64, i64)>,
    /// "among the trees near The Duck Pond"
    clue: String,
    #[serde(default)]
    glinted: bool,
}

/// Something in pieces, and where it belongs.
#[derive(Serialize, Deserialize, Clone)]
struct Pieces {
    set: String,
    x: i64,
    y: i64,
    place: String,
    until: i64,
    pieces: Vec<Piece>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Piece {
    item: String,
    /// Where it's lying, until someone picks it up.
    at: Option<(i64, i64)>,
    /// Who set it down where it belongs.
    by: Option<String>,
    /// Who's caught a glint of it already.
    #[serde(default)]
    glinted: Vec<String>,
}

impl Pieces {
    fn def(&self) -> Option<&'static Set> {
        items::set(&self.set)
    }
    fn placed(&self) -> Vec<&Piece> {
        self.pieces.iter().filter(|p| p.by.is_some()).collect()
    }
}

/// How a hiding spot sounds: "among the trees".
fn ground_words(t: T) -> &'static str {
    match t {
        T::Forest => "among the trees",
        T::Meadow => "among the meadow flowers",
        T::Sand => "out on the sand",
        T::Path => "along the path",
        T::Hill => "up on the hillside",
        _ => "in the long grass",
    }
}

/// "north-east" for a step from one spot towards another.
fn way(dx: i64, dy: i64) -> &'static str {
    DIRS.iter()
        .find(|q| q.2 == dx.signum() && q.3 == dy.signum())
        .map_or("nearby", |q| q.1)
}

fn steps(a: (i64, i64), b: (i64, i64)) -> i64 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// "the silver thimble"
fn the(s: &Species) -> String {
    format!("the {}", s.name)
}

impl Game {
    pub(super) fn load_quests(&mut self) {
        self.quests = self
            .world
            .db
            .meta(META)
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();
    }

    fn save_quests(&self) {
        if let Ok(j) = serde_json::to_string(&self.quests) {
            let _ = self.world.db.set_meta(META, &j);
        }
    }

    /// Named places on an island that aren't anyone's home.
    fn quest_places(&self, island: &Island) -> Vec<(i64, i64, String)> {
        self.world
            .places()
            .into_iter()
            .filter(|p| p.3.id == island.id && self.world.home_at(p.0, p.1).is_none())
            .filter(|p| self.world.tile(p.0, p.1).is_some_and(|t| t.name.is_some()))
            .map(|p| (p.0, p.1, p.2))
            .collect()
    }

    /// Somewhere on an island to hide something, near a named place and not
    /// too near `avoid`, with a clue to where.
    fn quest_hide(
        &mut self,
        island: &Island,
        avoid: Option<(i64, i64)>,
    ) -> Option<((i64, i64), String)> {
        let places = self.quest_places(island);
        for _ in 0..200 {
            let x = island.ox + (self.rng.next() % REGION as u64) as i64;
            let y = island.oy + (self.rng.next() % REGION as u64) as i64;
            let t = self.world.terrain(x, y);
            if !HIDE_ON.contains(&t)
                || self.world.home_at(x, y).is_some()
                || self.world.things_at(x, y).count() >= 8
                || avoid.is_some_and(|a| steps(a, (x, y)) < 3)
            {
                continue;
            }
            let Some(near) = places
                .iter()
                .filter(|p| steps((p.0, p.1), (x, y)) <= HIDE_NEAR)
                .min_by_key(|p| steps((p.0, p.1), (x, y)))
            else {
                continue;
            };
            let clue = if (near.0, near.1) == (x, y) {
                format!("right at {}", near.2)
            } else {
                format!("{} near {}", ground_words(t), near.2)
            };
            return Some(((x, y), clue));
        }
        None
    }

    // ---- errands -------------------------------------------------------

    /// One of the folk, chatting to `user`, might mention something they've
    /// lost. What they'd say, if so.
    pub(super) fn errand_offer(
        &mut self,
        folk: &str,
        island: u8,
        leaves: i64,
        user: &str,
        now: i64,
    ) -> Option<String> {
        if user.contains('@')
            || leaves - now < ERRAND_NEEDS
            || self.quests.errands.iter().any(|e| e.user == user)
            || self
                .quests
                .asked
                .get(user)
                .is_some_and(|&t| now - t < ERRAND_REST)
        {
            return None;
        }
        let season = Clock::season();
        let lost: Vec<&items::Lost> = items::LOST
            .iter()
            .filter(|l| l.season.is_none_or(|s| s == season))
            .filter(|l| !self.quests.errands.iter().any(|e| e.item == l.item.id))
            .collect();
        if lost.is_empty() {
            return None;
        }
        let l = *self.rng.pick(&lost);
        let island = &ISLANDS[island as usize % ISLANDS.len()];
        let (at, clue) = self.quest_hide(island, None)?;
        self.quests.errands.push(Errand {
            user: user.to_string(),
            folk: folk.to_string(),
            item: l.item.id.to_string(),
            at: Some(at),
            clue: clue.clone(),
            glinted: false,
        });
        self.quests.asked.insert(user.to_string(), now);
        self.save_quests();
        Some(format!(
            "I've gone and lost my {}. Last I had it, I was {clue}. If you happen across it, I'd be ever so glad.",
            l.item.name
        ))
    }

    /// What they're still hoping for, for `look` at one of the folk.
    pub(super) fn errand_hoping(&self, user: &str, folk: &str) -> Option<String> {
        let e = self
            .quests
            .errands
            .iter()
            .find(|e| e.user == user && e.folk == folk && e.at.is_some())?;
        let s = nature::by_id(&e.item)?;
        Some(format!(
            " Still hoping someone turns up {}.",
            nature::a(s.name)
        ))
    }

    /// Giving one of the folk the thing they lost. None if it isn't theirs.
    pub(super) fn errand_returned(
        &mut self,
        id: u64,
        folk: &str,
        s: &'static Species,
    ) -> Option<anyhow::Result<()>> {
        let user = self.user(id);
        let i = self
            .quests
            .errands
            .iter()
            .position(|e| e.user == user && e.folk == folk && e.item == s.id)?;
        Some(self.errand_done(id, i, s))
    }

    fn errand_done(&mut self, id: u64, i: usize, s: &'static Species) -> anyhow::Result<()> {
        let Some((x, y, me)) = self.here(id) else {
            return Ok(());
        };
        let e = self.quests.errands.remove(i);
        let user = e.user.clone();
        let thanks: &'static Species = self.rng.pick(items::THANKS);
        let words = *self.rng.pick(&[
            "Oh, you found it! I'd have been lost without it.",
            "Well I never! Where was it? No, don't tell me, I'll only feel silly.",
            "You're a treasure. A real treasure.",
            "Oh, thank you. I'd looked everywhere. Well, nearly everywhere.",
        ]);
        self.world.db.satchel_add(&user, s.id, -1)?;
        self.world.db.satchel_add(&user, thanks.id, 1)?;
        self.save_quests();
        self.say1(
            id,
            Line::new()
                .c(Col::Pink, "🎁 ")
                .b(Col::Cyan, &e.folk)
                .c(Col::Pink, format!(" beams. \"My {}! {words}\"", s.name)),
        );
        self.say1(
            id,
            Line::col(
                Col::Pink,
                format!(
                    "🎁 {} gives you {} to say thank you. It would look lovely at home.",
                    e.folk,
                    nature::a(thanks.name)
                ),
            ),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Pink,
                format!(
                    "🎁 {me} gives {} back {}, and {} is delighted.",
                    e.folk,
                    the(s),
                    e.folk
                ),
            ),
        );
        let _ = self
            .world
            .db
            .discovered(&user, "errands", super::boards::now());
        Ok(())
    }

    /// `errand`: a reminder, if you'd like one.
    pub(super) fn cmd_errand(&mut self, id: u64) -> anyhow::Result<()> {
        let user = self.user(id);
        let Some(e) = self.quests.errands.iter().find(|e| e.user == user) else {
            self.say1(
                id,
                Line::plain("Nobody's asked you to look for anything just now."),
            );
            return Ok(());
        };
        let Some(s) = nature::by_id(&e.item) else {
            return Ok(());
        };
        let text = if e.at.is_some() {
            format!(
                "🧵 {} lost {}, somewhere {}. No hurry.",
                e.folk,
                nature::a(s.name),
                e.clue
            )
        } else {
            let at = self
                .folk
                .iter()
                .find(|f| f.name == e.folk)
                .map(|f| self.world.place_title(f.x, f.y))
                .unwrap_or_default();
            format!(
                "🧵 You've found {}'s {}. {} is at {at} just now. (give {} to {})",
                e.folk, s.name, e.folk, s.name, e.folk
            )
        };
        self.say1(id, Line::col(Col::Pink, text));
        Ok(())
    }

    // ---- pieces --------------------------------------------------------

    /// Something comes apart somewhere, if nothing is in pieces already.
    fn pieces_start(&mut self, now: i64) {
        let season = Clock::season();
        let seasonal: Vec<&'static Set> = items::SETS
            .iter()
            .filter(|s| s.season == Some(season))
            .collect();
        let any: Vec<&'static Set> = items::SETS.iter().filter(|s| s.season.is_none()).collect();
        // The season's own, more often than not: the build-up to its festival.
        let def = if !seasonal.is_empty() && self.rng.chance(0.6) {
            *self.rng.pick(&seasonal)
        } else {
            *self.rng.pick(&any)
        };
        // Mostly Home Island, where everyone arrives.
        let island = if self.rng.chance(0.7) {
            &ISLANDS[0]
        } else {
            self.rng.pick(&ISLANDS[1..])
        };
        let places = self.quest_places(island);
        if places.is_empty() {
            self.quests.next_pieces = now + HOUR;
            return;
        }
        let (x, y, place) = self.rng.pick(&places).clone();
        let mut pieces = Vec::new();
        let mut taken: Vec<(i64, i64)> = vec![(x, y)];
        for p in &def.pieces {
            let mut spot = None;
            for _ in 0..5 {
                if let Some((at, _)) = self.quest_hide(island, Some((x, y)))
                    && !taken.iter().any(|&t| steps(t, at) < 4)
                {
                    spot = Some(at);
                    break;
                }
            }
            let Some(at) = spot else {
                self.quests.next_pieces = now + HOUR;
                return;
            };
            taken.push(at);
            pieces.push(Piece {
                item: p.id.to_string(),
                at: Some(at),
                by: None,
                glinted: vec![],
            });
        }
        self.quests.pieces = Some(Pieces {
            set: def.id.to_string(),
            x,
            y,
            place: place.clone(),
            until: now + PIECES_LAST,
            pieces,
        });
        self.save_quests();
        let line = Line::col(
            Col::Yellow,
            format!(
                "🧩 Word is that {} has come apart, its pieces scattered about {}. They belong at {place}.",
                def.name, island.name
            ),
        );
        for p in self.players.values().filter(|p| !p.bot && p.about()) {
            let _ = p.tx.send(crate::protocol::ToClient::Lines {
                lines: vec![line.clone()],
            });
        }
    }

    /// Every piece is in: it comes whole, for everyone who helped.
    fn pieces_whole(&mut self, finisher: &str) {
        let Some(ps) = self.quests.pieces.take() else {
            return;
        };
        let now = super::boards::now();
        self.quests.next_pieces = now + self.rng.range(PIECES_REST.0, PIECES_REST.1) as i64;
        self.save_quests();
        let Some(def) = ps.def() else {
            return;
        };
        let mut helpers: Vec<String> = Vec::new();
        for p in &ps.pieces {
            if let Some(b) = &p.by
                && !helpers.contains(b)
            {
                helpers.push(b.clone());
            }
        }
        let names: Vec<String> = helpers.iter().map(|u| self.world.nick(u)).collect();
        let whole = def.whole.replace("{place}", &ps.place);
        let line = Line::col(
            Col::Yellow,
            format!("{} {whole} Thanks to {}.", def.icon, join_and(&names)),
        );
        let gift = Line::col(
            Col::Pink,
            format!(
                "🎁 {} is tucked into your satchel, to remember it by.",
                capitalise(&nature::a(def.keepsake.name))
            ),
        );
        for p in self.players.values().filter(|p| !p.bot) {
            let mut lines = vec![line.clone()];
            if helpers.contains(&p.user) {
                lines.push(gift.clone());
            }
            let _ = p.tx.send(crate::protocol::ToClient::Lines { lines });
        }
        for u in &helpers {
            let _ = self.world.db.satchel_add(u, def.keepsake.id, 1);
            let _ = self.world.db.discovered(u, "pieces", now);
        }
        self.happened(
            finisher,
            WHOLE,
            ps.x,
            ps.y,
            &format!("{}\t{}\t{}", def.icon, def.id, helpers.join(",")),
        );
    }

    /// For the recap: "🎶 An old music box came together at The Bandstand,
    /// thanks to you, Sam and Poppy."
    pub(super) fn whole_words(&self, me: &str, what: &str, place: &str) -> Option<String> {
        let mut parts = what.split('\t');
        let (icon, set, helpers) = (parts.next()?, parts.next()?, parts.next()?);
        let def = items::set(set)?;
        let helpers: Vec<&str> = helpers.split(',').filter(|h| !h.is_empty()).collect();
        let mut names: Vec<String> = Vec::new();
        if helpers.contains(&me) {
            names.push("you".into());
        }
        names.extend(
            helpers
                .iter()
                .filter(|h| **h != me)
                .map(|h| self.world.nick(h)),
        );
        Some(format!(
            "{icon} {} came together at {place}, thanks to {}.",
            capitalise(def.name),
            join_and(&names)
        ))
    }

    // ---- out on the island ---------------------------------------------

    /// The world turning, for quests: things fading, things coming apart,
    /// and glints for whoever's close.
    pub(super) fn quests_tick(&mut self) {
        let now = super::boards::now();
        let mut changed = false;
        // An errand goes when the folk who asked does, and what was found
        // goes with them.
        let gone: Vec<Errand> = self
            .quests
            .errands
            .iter()
            .filter(|e| !self.folk.iter().any(|f| f.name == e.folk))
            .cloned()
            .collect();
        for e in gone {
            self.quests.errands.retain(|o| o.user != e.user);
            // Wherever it got to, even a friend's satchel.
            let _ = self.world.db.satchel_take_all(&e.item);
            changed = true;
        }
        // Pieces nobody got to fade, wherever they are.
        if let Some(ps) = self.quests.pieces.as_ref().filter(|p| now >= p.until) {
            for p in &ps.pieces {
                let _ = self.world.db.satchel_take_all(&p.item);
            }
            self.quests.pieces = None;
            self.quests.next_pieces = now + self.rng.range(PIECES_REST.0, PIECES_REST.1) as i64;
            changed = true;
        }
        if self.quests.pieces.is_none()
            && now >= self.quests.next_pieces
            && self.players.values().any(|p| !p.bot && p.about())
        {
            self.pieces_start(now);
        }
        changed |= self.quest_glints();
        if changed {
            self.save_quests();
        }
    }

    /// A glint of something just nearby, once.
    fn quest_glints(&mut self) -> bool {
        let people: Vec<(u64, String, i64, i64)> = self
            .players
            .iter()
            .filter(|(_, p)| !p.bot && p.about() && !in_room_space(p.x, p.y))
            .map(|(&id, p)| (id, p.user.clone(), p.x, p.y))
            .collect();
        let mut changed = false;
        for (id, user, x, y) in people {
            if let Some(e) = self
                .quests
                .errands
                .iter_mut()
                .find(|e| e.user == user && !e.glinted)
                && let Some(at) = e.at
                && (1..=GLINT).contains(&steps(at, (x, y)))
            {
                e.glinted = true;
                changed = true;
                let (folk, item) = (e.folk.clone(), e.item.clone());
                if let Some(s) = nature::by_id(&item) {
                    self.say1(
                        id,
                        Line::col(
                            Col::Pink,
                            format!(
                                "✨ Something glints just to the {}. Could that be {folk}'s {}?",
                                way(at.0 - x, at.1 - y),
                                s.name
                            ),
                        ),
                    );
                }
            }
            let carrying = self.carrying_piece(&user);
            let Some(ps) = self.quests.pieces.as_mut() else {
                continue;
            };
            let Some(def) = items::set(&ps.set) else {
                continue;
            };
            if carrying {
                continue;
            }
            if let Some(p) = ps.pieces.iter_mut().find(|p| {
                !p.glinted.contains(&user)
                    && p.at
                        .is_some_and(|at| (1..=GLINT).contains(&steps(at, (x, y))))
            }) {
                p.glinted.push(user.clone());
                changed = true;
                let at = p.at.unwrap_or((x, y));
                let line = Line::col(
                    Col::Yellow,
                    format!(
                        "🧩 Something catches your eye just to the {}: it looks like part of {}.",
                        way(at.0 - x, at.1 - y),
                        def.name
                    ),
                );
                self.say1(id, line);
            }
        }
        changed
    }

    /// Whether they've a piece of what's in pieces in their satchel already.
    fn carrying_piece(&self, user: &str) -> bool {
        let Some(ps) = &self.quests.pieces else {
            return false;
        };
        let satchel = self.world.db.satchel(user).unwrap_or_default();
        ps.pieces
            .iter()
            .any(|p| satchel.iter().any(|(sid, n)| *sid == p.item && *n > 0))
    }

    /// For `look`: anything quest-ish here.
    pub(super) fn quest_lines(&self, id: u64) -> Vec<Line> {
        let Some(me) = self.players.get(&id) else {
            return vec![];
        };
        let here = (me.x, me.y);
        let mut out = Vec::new();
        for e in self
            .quests
            .errands
            .iter()
            .filter(|e| e.user == me.user && e.at == Some(here))
        {
            if let Some(s) = nature::by_id(&e.item) {
                out.push(Line::col(
                    Col::Pink,
                    format!(
                        "✨ {} {}. Could it be {}'s?",
                        capitalise(&nature::a(s.name)),
                        s.here,
                        e.folk
                    ),
                ));
                out.push(Line::col(Col::Dim, "  (type gather to take it)"));
            }
        }
        if let Some(ps) = &self.quests.pieces
            && let Some(def) = ps.def()
        {
            for p in ps.pieces.iter().filter(|p| p.at == Some(here)) {
                if let Some(s) = nature::by_id(&p.item) {
                    out.push(Line::col(
                        Col::Yellow,
                        format!(
                            "🧩 {} {}. It looks like part of {}.",
                            capitalise(&nature::a(s.name)),
                            s.here,
                            def.name
                        ),
                    ));
                    out.push(Line::col(Col::Dim, "  (type gather to take it)"));
                }
            }
            if (ps.x, ps.y) == here {
                let placed: Vec<String> = ps
                    .placed()
                    .iter()
                    .filter_map(|p| nature::by_id(&p.item))
                    .map(the)
                    .collect();
                let left = ps.pieces.len() - placed.len();
                let so_far = if placed.is_empty() {
                    "None of its pieces has found its way back yet.".to_string()
                } else {
                    format!(
                        "Set down so far: {}. {left} still to find.",
                        join_and(&placed)
                    )
                };
                out.push(Line::col(
                    Col::Yellow,
                    format!(
                        "{} {} belongs here, in pieces. {so_far}",
                        def.icon,
                        capitalise(def.name)
                    ),
                ));
            }
        }
        out
    }

    /// `gather` where something's waiting to be found. None if nothing is.
    pub(super) fn quest_gather(&mut self, id: u64, what: &str) -> Option<anyhow::Result<()>> {
        let p = self.players.get(&id)?;
        let (here, user, me) = ((p.x, p.y), p.user.clone(), p.name.clone());
        let fits = |sid: &str| nature::by_id(sid).filter(|s| super::gather::matches(s, what));
        if let Some(i) = self
            .quests
            .errands
            .iter()
            .position(|e| e.user == user && e.at == Some(here) && fits(&e.item).is_some())
        {
            let s = fits(&self.quests.errands[i].item)?;
            if self.satchel_room(&user) == 0 {
                self.say1(
                    id,
                    Line::plain("🧺 Your satchel is full! Give, place or release something first."),
                );
                return Some(Ok(()));
            }
            let folk = self.quests.errands[i].folk.clone();
            self.quests.errands[i].at = None;
            self.save_quests();
            if let Err(e) = self.world.db.satchel_add(&user, s.id, 1) {
                return Some(Err(e));
            }
            self.say1(
                id,
                Line::col(
                    Col::Pink,
                    format!(
                        "✨ You pick up {}. {folk} will be so pleased. (give {} to {folk})",
                        the(s),
                        s.name
                    ),
                ),
            );
            self.tell_here(
                here.0,
                here.1,
                id,
                Line::col(
                    Col::Pink,
                    format!("✨ {me} picks up {}.", nature::a(s.name)),
                ),
            );
            self.status_near(id, &[here]);
            return Some(Ok(()));
        }
        let ps = self.quests.pieces.as_ref()?;
        let i = ps
            .pieces
            .iter()
            .position(|p| p.at == Some(here) && fits(&p.item).is_some())?;
        let s = fits(&ps.pieces[i].item)?;
        let (place, def) = (ps.place.clone(), ps.def()?);
        if self.carrying_piece(&user) {
            self.say1(
                id,
                Line::plain(format!(
                    "You've a piece of {} already. Set it down at {place} first, and leave this one for someone else to find.",
                    def.name
                )),
            );
            return Some(Ok(()));
        }
        if self.satchel_room(&user) == 0 {
            self.say1(
                id,
                Line::plain("🧺 Your satchel is full! Give, place or release something first."),
            );
            return Some(Ok(()));
        }
        if let Some(ps) = self.quests.pieces.as_mut() {
            ps.pieces[i].at = None;
        }
        self.save_quests();
        if let Err(e) = self.world.db.satchel_add(&user, s.id, 1) {
            return Some(Err(e));
        }
        self.say1(
            id,
            Line::col(
                Col::Yellow,
                format!(
                    "🧩 You pick up {}. It looks like it belongs with something at {place}. (place {} there)",
                    the(s),
                    s.name
                ),
            ),
        );
        self.tell_here(
            here.0,
            here.1,
            id,
            Line::col(
                Col::Yellow,
                format!("🧩 {me} picks up {}.", nature::a(s.name)),
            ),
        );
        self.status_near(id, &[here]);
        Some(Ok(()))
    }

    /// `place` for quest things: a piece goes where it belongs, and what the
    /// folk lost isn't yours to leave about. None for anything else.
    pub(super) fn quest_place(
        &mut self,
        id: u64,
        s: &'static Species,
    ) -> Option<anyhow::Result<()>> {
        let p = self.players.get(&id)?;
        let (here, user, me) = ((p.x, p.y), p.user.clone(), p.name.clone());
        if let Some(e) = self
            .quests
            .errands
            .iter()
            .find(|e| e.user == user && e.item == s.id)
        {
            self.say1(
                id,
                Line::plain(format!(
                    "That's {}'s. Best give it back. (give {} to {})",
                    e.folk, s.name, e.folk
                )),
            );
            return Some(Ok(()));
        }
        let ps = self.quests.pieces.as_ref()?;
        let i = ps
            .pieces
            .iter()
            .position(|p| p.item == s.id && p.by.is_none())?;
        let (place, def) = (ps.place.clone(), ps.def()?);
        if (ps.x, ps.y) != here {
            self.say1(
                id,
                Line::plain(format!(
                    "It doesn't look like it belongs here. Something about it says {place}."
                )),
            );
            return Some(Ok(()));
        }
        if let Err(e) = self.world.db.satchel_add(&user, s.id, -1) {
            return Some(Err(e));
        }
        let left = {
            let ps = self.quests.pieces.as_mut()?;
            ps.pieces[i].by = Some(user.clone());
            ps.pieces.iter().filter(|p| p.by.is_none()).count()
        };
        self.save_quests();
        let after = match left {
            0 => String::new(),
            1 => " Just one piece still to find.".to_string(),
            n => format!(" {n} pieces still to find."),
        };
        self.say1(
            id,
            Line::col(
                Col::Yellow,
                format!("{} You set {} down at {place}.{after}", def.icon, the(s)),
            ),
        );
        self.tell_here(
            here.0,
            here.1,
            id,
            Line::col(
                Col::Yellow,
                format!("{} {me} sets {} down here.{after}", def.icon, the(s)),
            ),
        );
        if left == 0 {
            self.pieces_whole(&user);
        }
        Some(Ok(()))
    }

    /// For the scenery: whether something's in pieces on this island.
    pub(super) fn pieces_on(&self, x: i64, y: i64) -> bool {
        self.quests
            .pieces
            .as_ref()
            .is_some_and(|p| island_at(p.x, p.y).id == island_at(x, y).id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clues_sound_like_places() {
        for t in HIDE_ON {
            assert!(!ground_words(*t).is_empty());
        }
        assert_eq!(way(1, -1), "north-east");
        assert_eq!(steps((0, 0), (2, -1)), 2);
    }

    #[test]
    fn old_saves_still_load() {
        let q: Quests = serde_json::from_str(r#"{"errands":[]}"#).unwrap();
        assert!(q.pieces.is_none() && q.next_pieces == 0);
    }
}
