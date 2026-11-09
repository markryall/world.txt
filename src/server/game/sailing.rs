//! Sailing between islands from the docks, in a little rowing boat.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::islands::{self, ISLANDS};
use crate::server::terrain::Terrain;
use crate::server::world::{DIRS, SIZE, World, in_bounds};
use std::collections::VecDeque;

/// Someone out on the water: where they're headed, the way across still to
/// row (a square a stride, like walking), and how far they've come.
pub struct Voyage {
    pub to: (i64, i64),
    pub island: &'static str,
    pub from: &'static str,
    pub path: VecDeque<(i64, i64)>,
    pub rowed: usize,
}

impl Game {
    pub(super) fn dock_here(&self, x: i64, y: i64) -> bool {
        self.world.things_at(x, y).any(|t| t.is_dock())
    }

    pub(super) fn dock_on(&self, island: &islands::Island) -> Option<(i64, i64)> {
        self.world
            .things
            .iter()
            .find(|t| t.is_dock() && island.covers(t.x, t.y))
            .map(|t| (t.x, t.y))
    }

    pub(super) fn cmd_sail(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        if self.at_sea(id) {
            return Ok(());
        }
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let here = islands::island_at(x, y);
        if !self.dock_here(x, y) {
            let hint = match self.dock_on(here) {
                Some((dx, dy)) => format!(
                    "The dock on {} is at {}. (it's ⚓ on the map)",
                    here.name,
                    self.world.place_title(dx, dy)
                ),
                None => "There's no dock on this island.".into(),
            };
            self.say1(
                id,
                Line::plain(format!(
                    "You need a boat! Boats are tied up at the docks. {hint}"
                )),
            );
            return Ok(());
        }
        if self.sail_between_worlds(id, rest)? {
            return Ok(());
        }
        let Some(dest) = islands::by_name(rest).filter(|d| d.id != here.id) else {
            let mut lines = vec![Line::new().b(Col::Blue, "⛵ Where would you like to row to?")];
            for i in ISLANDS.iter().filter(|i| i.id != here.id) {
                lines.push(
                    Line::new()
                        .t("   ")
                        .b(
                            Col::Yellow,
                            format!("sail {}", i.name.split(' ').next().unwrap_or(i.name)),
                        )
                        .c(Col::Dim, format!("  {}: {}", i.name, i.blurb)),
                );
            }
            self.send(id, lines);
            return Ok(());
        };
        let Some(to) = self.dock_on(dest) else {
            self.say1(
                id,
                Line::plain(format!(
                    "There's nowhere to land on {} just now.",
                    dest.name
                )),
            );
            return Ok(());
        };
        let Some(path) = sea_path(&self.world, (x, y), to) else {
            self.say1(
                id,
                Line::plain(format!(
                    "There's no way across the water to {} just now.",
                    dest.name
                )),
            );
            return Ok(());
        };
        // Off the dock: out of any game there, and the kite stays ashore.
        self.games_walks(id);
        self.kite_walks(id);
        if let Some(p) = self.players.get_mut(&id) {
            p.travel = None;
            p.walk_to = None;
            p.fishing = None;
            p.voyage = Some(Voyage {
                to,
                island: dest.name,
                from: here.name,
                path,
                rowed: 0,
            });
        }
        self.say1(
            id,
            Line::col(
                Col::Blue,
                "⛵ You climb into the little rowing boat and push off from the dock.",
            ),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Blue,
                format!("⛵ {name} rows away towards {}.", dest.name),
            ),
        );
        Ok(())
    }

    /// `go dock`: walk to this island's dock.
    pub(super) fn cmd_go_dock(&mut self, id: u64) -> anyhow::Result<()> {
        if self.at_sea(id) {
            return Ok(());
        }
        self.step_out_first(id)?;
        let Some((x, y, _)) = self.here(id) else {
            return Ok(());
        };
        let island = islands::island_at(x, y);
        let Some((dx, dy)) = self.dock_on(island) else {
            self.say1(id, Line::plain("There's no dock on this island."));
            return Ok(());
        };
        let place = self.world.place_title(dx, dy);
        if let Some(p) = self.players.get_mut(&id) {
            p.travel = None;
            p.walk_to = Some((dx, dy, place.clone()));
        }
        self.say1(
            id,
            Line::col(
                Col::Cyan,
                format!("🚶 You head down to {place}. (stop to stop)"),
            ),
        );
        self.step_travellers();
        Ok(())
    }

    /// Everyone out on the water rows a square further.
    pub(super) fn row_boats(&mut self) {
        let sailors: Vec<u64> = self
            .players
            .iter()
            .filter(|(_, p)| p.voyage.is_some())
            .map(|(&id, _)| id)
            .collect();
        for id in sailors {
            let Some(p) = self.players.get_mut(&id) else {
                continue;
            };
            let was = (p.x, p.y);
            let Some(v) = p.voyage.as_mut() else {
                continue;
            };
            let Some(next) = v.path.pop_front() else {
                continue;
            };
            v.rowed += 1;
            let (rowed, left, island, from, to) = (v.rowed, v.path.len(), v.island, v.from, v.to);
            if !v.path.is_empty() {
                p.x = next.0;
                p.y = next.1;
                p.sky = self.world.weather.sky_at(next.0, next.1);
                let line = match (rowed, left) {
                    (2, _) => Some(format!(
                        "🌊 You row out past the shallows. {from} grows smaller behind you."
                    )),
                    (_, 3) => Some(format!("🏝 {island} rises out of the sea ahead.")),
                    _ if rowed == (rowed + left) / 2 && left > 4 => Some(
                        "🐦 Gulls wheel overhead, and the boat rocks gently on the swell."
                            .to_string(),
                    ),
                    _ => None,
                };
                if let Some(l) = line {
                    self.say1(id, Line::col(Col::Blue, l));
                }
                self.status_near(id, &[was, next]);
                continue;
            }
            let name = self.players[&id].name.clone();
            if let Some(p) = self.players.get_mut(&id) {
                p.voyage = None;
                p.x = to.0;
                p.y = to.1;
                p.sky = self.world.weather.sky_at(to.0, to.1);
            }
            self.say1(
                id,
                Line::col(
                    Col::Blue,
                    format!("⚓ You bump gently against the dock at {island}, and hop out."),
                ),
            );
            self.tell_here(
                to.0,
                to.1,
                id,
                Line::col(Col::Blue, format!("⛵ {name} arrives by boat from {from}.")),
            );
            self.look(id);
            self.save_progress(id);
            self.status_near(id, &[was, to]);
            // On the way somewhere further: walk on from the dock.
            if let Some((hx, hy, place)) = self.players.get_mut(&id).and_then(|p| p.heading.take())
            {
                if let Some(u) = self.players.get_mut(&id).and_then(|p| p.join_after.take()) {
                    // Off to find someone: wherever they are now.
                    if let Some(p) = self.players.get_mut(&id) {
                        p.travel = Some(u);
                    }
                    self.say1(
                        id,
                        Line::col(
                            Col::Cyan,
                            format!("🚶 On you go to find {place}. (stop to stop)"),
                        ),
                    );
                } else if (hx, hy) != to {
                    if let Some(p) = self.players.get_mut(&id) {
                        p.walk_to = Some((hx, hy, place.clone()));
                    }
                    let on = if place == "home" {
                        "🚶 On you go home. (stop to stop)".to_string()
                    } else {
                        format!("🚶 On you go to {place}. (stop to stop)")
                    };
                    self.say1(id, Line::col(Col::Cyan, on));
                } else {
                    self.say1(id, Line::col(Col::Cyan, super::travel::arrived(&place)));
                }
            }
        }
    }

    /// Out on the water, walking isn't an option.
    pub(super) fn at_sea(&self, id: u64) -> bool {
        let sailing = self.players.get(&id).is_some_and(|p| p.voyage.is_some());
        if sailing {
            self.say1(
                id,
                Line::col(
                    Col::Blue,
                    "You're out at sea! There's no stopping now: keep rowing, you're nearly there. 🌊",
                ),
            );
        }
        sailing
    }
}

/// The squares of sea between two docks, ending at the far one: a
/// shortest way over open water.
fn sea_path(world: &World, from: (i64, i64), to: (i64, i64)) -> Option<VecDeque<(i64, i64)>> {
    let sea = |x: i64, y: i64| {
        in_bounds(x, y) && matches!(world.terrain(x, y), Terrain::Water | Terrain::DeepWater)
    };
    let idx = |(x, y): (i64, i64)| (y * SIZE + x) as usize;
    let mut came: Vec<Option<(i64, i64)>> = vec![None; (SIZE * SIZE) as usize];
    let mut queue = VecDeque::from([from]);
    came[idx(from)] = Some(from);
    while let Some(p) = queue.pop_front() {
        if p == to {
            let mut path = VecDeque::new();
            let mut at = to;
            while at != from {
                path.push_front(at);
                at = came[idx(at)]?;
            }
            return Some(path);
        }
        for &(_, _, dx, dy) in &DIRS {
            let n = (p.0 + dx, p.1 + dy);
            if !in_bounds(n.0, n.1) || came[idx(n)].is_some() {
                continue;
            }
            // Out over the water, and in to the far dock.
            if n == to || sea(n.0, n.1) {
                came[idx(n)] = Some(p);
                queue.push_back(n);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::db::Db;

    #[test]
    fn every_dock_rows_to_every_other_a_square_at_a_time_over_water() {
        let world = World::load_or_create(Db::open(":memory:").unwrap(), Some(7)).unwrap();
        let docks: Vec<(i64, i64)> = world
            .things
            .iter()
            .filter(|t| t.is_dock())
            .map(|t| (t.x, t.y))
            .collect();
        assert_eq!(docks.len(), ISLANDS.len());
        for &from in &docks {
            for &to in docks.iter().filter(|&&d| d != from) {
                let path = sea_path(&world, from, to).expect("a way across");
                assert_eq!(path.back(), Some(&to), "ends at the far dock");
                let mut at = from;
                for &(x, y) in &path {
                    assert!((x - at.0).abs() <= 1 && (y - at.1).abs() <= 1, "one square");
                    at = (x, y);
                }
                for &(x, y) in path.iter().take(path.len() - 1) {
                    assert!(
                        matches!(world.terrain(x, y), Terrain::Water | Terrain::DeepWater),
                        "rows over the sea, not the land"
                    );
                }
            }
        }
    }
}
