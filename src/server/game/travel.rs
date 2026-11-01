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
        if let Some(p) = self.players.get_mut(&id) {
            p.travel = Some(target_user);
        }
        self.say1(id, Line::col(Col::Cyan, format!("🚶 You set off to find {target}. (type stop to stop)")));
        if self.players[&tid].hears(&self.user(id)) {
            self.say1(tid, Line::col(Col::Cyan, format!("🚶 {name} is coming to find you!")));
        }
        self.step_travellers();
        Ok(())
    }

    pub(super) fn stop_travelling(&mut self, id: u64, say: bool) {
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
                self.say1(id, Line::col(Col::Cyan, format!("🎉 You've arrived at {place}!")));
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
