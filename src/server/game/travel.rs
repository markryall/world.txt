//! `join <Name>`: walk to a friend a step at a time, finding a way around
//! the water, without a description at every step.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::world::{DIRS, SIZE, in_bounds};
use std::collections::VecDeque;

impl Game {
    /// The first step on a shortest walkable path from `from` to `to`.
    fn first_step(&self, from: (i64, i64), to: (i64, i64)) -> Option<(i64, i64)> {
        let idx = |(x, y): (i64, i64)| (y * SIZE + x) as usize;
        let mut first: Vec<Option<(i64, i64)>> = vec![None; (SIZE * SIZE) as usize];
        let mut seen = vec![false; (SIZE * SIZE) as usize];
        let mut queue = VecDeque::new();
        seen[idx(from)] = true;
        queue.push_back(from);
        while let Some(p) = queue.pop_front() {
            if p == to {
                return first[idx(p)];
            }
            for &(_, _, dx, dy) in &DIRS {
                let n = (p.0 + dx, p.1 + dy);
                if !in_bounds(n.0, n.1) || seen[idx(n)] || !self.world.terrain(n.0, n.1).walkable() {
                    continue;
                }
                seen[idx(n)] = true;
                first[idx(n)] = first[idx(p)].or(Some((dx, dy)));
                queue.push_back(n);
            }
        }
        None
    }

    pub(super) fn cmd_join(&mut self, id: u64, who: &str) -> anyhow::Result<()> {
        let Some((_, _, name)) = self.here(id) else { return Ok(()) };
        let Some(tid) = self.find_player(who) else {
            self.say1(id, Line::plain(format!("Nobody called {} is in the world right now. Try 'who'.", who.trim())));
            return Ok(());
        };
        let target = self.players[&tid].name.clone();
        let target_user = self.players[&tid].user.clone();
        if tid == id {
            self.say1(id, Line::plain("You're already with yourself. 💅"));
            return Ok(());
        }
        if self.players[&tid].hears(&self.user(id)) {
            self.say1(tid, Line::col(Col::Cyan, format!("🚶 {name} is coming to find you!")));
        }
        // Over the water too, if need be: to the dock, across, then find them.
        let (tx, ty) = (self.players[&tid].x, self.players[&tid].y);
        self.set_off(id, tx, ty, &target, Some(target_user))
    }

    /// `places`: every named spot and home, on every island, numbered.
    pub(super) fn cmd_places(&mut self, id: u64) -> anyhow::Result<()> {
        let places = self.world.places();
        let mut lines = vec![Line::new().b(Col::Magenta, "🗺  Places to go")];
        if places.is_empty() {
            lines.push(Line::col(Col::Dim, "  Nowhere has a name yet. Stand somewhere lovely and name it: name The Willow Bank"));
        }
        let mut island = None;
        for (i, (_, _, name, isl)) in places.iter().enumerate() {
            if island != Some(isl.id) {
                island = Some(isl.id);
                lines.push(Line::col(Col::Pink, format!("  {}", isl.name)));
            }
            lines.push(Line::new().c(Col::Yellow, format!("   {:>2}. ", i + 1)).t(name.clone()));
        }
        if !places.is_empty() {
            lines.push(Line::col(Col::Dim, "  go to The Greenhouse (part of a name will do) or go 3 walks you there, rowing across if need be"));
        }
        self.send(id, lines);
        Ok(())
    }

    /// `go [to] The Greenhouse` or `go 3`: walk there, by boat if it's on
    /// another island.
    pub(super) fn cmd_go_place(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        let want = rest.trim();
        let want = want.strip_prefix("to ").unwrap_or(want).trim();
        if want.is_empty() {
            self.say1(id, Line::plain("Go where? A direction (n, se…), or a place: go to The Greenhouse ('places' lists them)."));
            return Ok(());
        }
        let places = self.world.places();
        let Some((tx, ty, name, _)) = pick_place(&places, want) else {
            self.say1(id, Line::plain(format!("There's nowhere called {want}. 'places' lists everywhere with a name.")));
            return Ok(());
        };
        if (tx, ty) == (x, y) {
            self.say1(id, Line::plain(format!("You're already at {name}. 💅")));
            return Ok(());
        }
        self.set_off(id, tx, ty, &name, None)
    }

    /// Head for (tx, ty): walking, or down to the dock and rowing across if
    /// it's on another island, then walking on. `join`: then go and find
    /// that person (by username) once across.
    pub(super) fn set_off(&mut self, id: u64, tx: i64, ty: i64, name: &str, join: Option<String>) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        let here = crate::server::islands::island_at(x, y);
        let island = crate::server::islands::island_at(tx, ty);
        if here.id == island.id {
            if let Some(p) = self.players.get_mut(&id) {
                p.heading = None;
                p.join_after = None;
                match join {
                    Some(u) => {
                        p.walk_to = None;
                        p.travel = Some(u);
                    }
                    None => {
                        p.travel = None;
                        p.walk_to = Some((tx, ty, name.to_string()));
                    }
                }
            }
            self.say1(id, Line::col(Col::Cyan, format!("🚶 You set off for {name}. (stop to stop)")));
        } else {
            // Over the water: down to the dock, row across, then walk on.
            let Some((dx, dy)) = self.dock_on(here) else {
                self.say1(id, Line::plain("There's no dock on this island to row from."));
                return Ok(());
            };
            let dock = self.world.place_title(dx, dy);
            if let Some(p) = self.players.get_mut(&id) {
                p.travel = None;
                p.heading = Some((tx, ty, name.to_string()));
                p.join_after = join;
                p.walk_to = ((dx, dy) != (x, y)).then(|| (dx, dy, dock.clone()));
            }
            self.say1(id, Line::col(Col::Cyan, format!("🚶 You set off for {name}, over on {}: down to {dock} first, then a row across. (stop to stop)", island.name)));
            if (dx, dy) == (x, y) {
                self.carry_on(id);
            }
        }
        self.step_travellers();
        Ok(())
    }

    /// At a dock on the way somewhere over the water: row on across.
    fn carry_on(&mut self, id: u64) {
        let Some((tx, ty, _)) = self.players.get(&id).and_then(|p| p.heading.clone()) else { return };
        let dest = crate::server::islands::island_at(tx, ty);
        let word = dest.name.split(' ').next().unwrap_or(dest.name).to_string();
        let _ = self.cmd_sail(id, &word);
    }

    pub(super) fn stop_travelling(&mut self, id: u64, say: bool) {
        if let Some(p) = self.players.get_mut(&id) {
            p.heading = None;
            p.join_after = None;
        }
        let place = self.players.get_mut(&id).and_then(|p| p.walk_to.take());
        let was = self.players.get_mut(&id).and_then(|p| p.travel.take()).map(|_| ()).or(place.map(|_| ()));
        if was.is_some() && say {
            self.say1(id, Line::col(Col::Dim, "You stop and look around."));
            self.look(id);
        }
    }

    /// Everyone on their way somewhere takes one more step.
    pub(super) fn step_travellers(&mut self) {
        // Walking to a place.
        let walkers: Vec<(u64, (i64, i64, String))> =
            self.players.iter().filter_map(|(&id, p)| p.walk_to.clone().map(|w| (id, w))).collect();
        for (id, (tx, ty, place)) in walkers {
            let me = (self.players[&id].x, self.players[&id].y);
            if me != (tx, ty) {
                let Some((dx, dy)) = self.first_step(me, (tx, ty)) else {
                    self.say1(id, Line::plain(format!("You can't find a way to {place} over land.")));
                    self.stop_travelling(id, true);
                    continue;
                };
                // walk() stays quiet while walk_to is set.
                let _ = self.walk(id, dx, dy);
            }
            if (self.players[&id].x, self.players[&id].y) == (tx, ty) {
                if let Some(p) = self.players.get_mut(&id) {
                    p.walk_to = None;
                }
                if self.players.get(&id).is_some_and(|p| p.heading.is_some()) {
                    // Only the dock so far: on across the water.
                    self.carry_on(id);
                    continue;
                }
                self.say1(id, Line::col(Col::Cyan, arrived(&place)));
                self.look(id);
            }
        }
        let travellers: Vec<(u64, String)> =
            self.players.iter().filter_map(|(&id, p)| p.travel.clone().map(|t| (id, t))).collect();
        for (id, target_user) in travellers {
            let target = self.world.nick(&target_user);
            let Some(tid) = self.find_player(&target_user).filter(|t| self.players[t].user == target_user) else {
                self.say1(id, Line::plain(format!("{target} has drifted off to sleep, so you stop.")));
                self.stop_travelling(id, true);
                continue;
            };
            let (me, them) = ((self.players[&id].x, self.players[&id].y), (self.players[&tid].x, self.players[&tid].y));
            if me != them {
                let island = |p: (i64, i64)| crate::server::islands::island_at(p.0, p.1).id;
                if island(me) != island(them) {
                    // They've sailed off: follow them over the water.
                    let _ = self.set_off(id, them.0, them.1, &target, Some(target_user.clone()));
                    continue;
                }
                let Some((dx, dy)) = self.first_step(me, them) else {
                    self.say1(id, Line::plain(format!("You can't find a way to {target} over land.")));
                    self.stop_travelling(id, true);
                    continue;
                };
                let _ = self.walk(id, dx, dy);
            }
            let me = (self.players[&id].x, self.players[&id].y);
            if me == them {
                if let Some(p) = self.players.get_mut(&id) {
                    p.travel = None;
                }
                let name = self.players[&id].name.clone();
                self.say1(id, Line::col(Col::Cyan, format!("🎉 You find {target}!")));
                self.look(id);
                if self.players[&tid].hears(&self.user(id)) {
                    self.say1(tid, Line::col(Col::Cyan, format!("🎉 {name} found you!")));
                }
            }
        }
    }
}

/// Getting there: home is home.
pub fn arrived(place: &str) -> String {
    if place == "home" { "🏡 You're home.".to_string() } else { format!("🎉 You've arrived at {place}!") }
}

/// The place someone means: its number from `places`, or its name (whole,
/// then the start of it, then any part), with or without "the".
fn pick_place<'a>(places: &'a [(i64, i64, String, &'static crate::server::islands::Island)], want: &str) -> Option<(i64, i64, String, &'static crate::server::islands::Island)> {
    if let Ok(n) = want.parse::<usize>() {
        return places.get(n.checked_sub(1)?).cloned();
    }
    let norm = |s: &str| {
        let s = s.trim().to_lowercase();
        s.strip_prefix("the ").map(str::to_string).unwrap_or(s)
    };
    let w = norm(want);
    let by = |f: &dyn Fn(&str) -> bool| places.iter().find(|p| f(&norm(&p.2))).cloned();
    by(&|n| n == w).or_else(|| by(&|n| n.starts_with(&w))).or_else(|| by(&|n| n.contains(&w)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::islands::ISLANDS;

    #[test]
    fn finds_a_place_by_number_or_name() {
        let places = vec![
            (1, 1, "The Greenhouse".to_string(), &ISLANDS[0]),
            (2, 2, "The Farmhouse".to_string(), &ISLANDS[0]),
            (70, 3, "Lighthouse Point".to_string(), &ISLANDS[1]),
            (4, 4, "Greenhouse Annex".to_string(), &ISLANDS[0]),
        ];
        let name = |w: &str| pick_place(&places, w).map(|p| p.2);
        assert_eq!(name("2").as_deref(), Some("The Farmhouse"));
        assert_eq!(name("greenhouse").as_deref(), Some("The Greenhouse"), "the whole name wins, 'the' or not");
        assert_eq!(name("the farm").as_deref(), Some("The Farmhouse"));
        assert_eq!(name("lighthouse").as_deref(), Some("Lighthouse Point"));
        assert_eq!(name("annex").as_deref(), Some("Greenhouse Annex"));
        assert_eq!(name("9"), None);
        assert_eq!(name("0"), None);
        assert_eq!(name("narnia"), None);
    }
}
