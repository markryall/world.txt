//! Sailing between islands from the docks, in a little rowing boat.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::islands::{self, ISLANDS};

/// Strides (half seconds) a crossing takes.
const CROSSING: u8 = 7;

/// Someone out on the water: where they're headed, and how far along.
pub struct Voyage {
    pub to: (i64, i64),
    pub island: &'static str,
    pub from: &'static str,
    pub stride: u8,
}

impl Game {
    fn dock_here(&self, x: i64, y: i64) -> bool {
        self.world.things_at(x, y).any(|t| t.is_dock())
    }

    pub(super) fn dock_on(&self, island: &islands::Island) -> Option<(i64, i64)> {
        self.world.things.iter().find(|t| t.is_dock() && island.covers(t.x, t.y)).map(|t| (t.x, t.y))
    }

    pub(super) fn cmd_sail(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let here = islands::island_at(x, y);
        if !self.dock_here(x, y) {
            let hint = match self.dock_on(here) {
                Some((dx, dy)) => format!("The dock on {} is at {}. (it's ⚓ on the map)", here.name, self.world.place_title(dx, dy)),
                None => "There's no dock on this island.".into(),
            };
            self.say1(id, Line::plain(format!("You need a boat! Boats are tied up at the docks. {hint}")));
            return Ok(());
        }
        if self.sail_between_worlds(id, rest)? {
            return Ok(());
        }
        let Some(dest) = islands::by_name(rest).filter(|d| d.id != here.id) else {
            let mut lines = vec![Line::new().b(Col::Blue, "⛵ Where would you like to row to?")];
            for i in ISLANDS.iter().filter(|i| i.id != here.id) {
                lines.push(Line::new().t("   ").b(Col::Yellow, format!("sail {}", i.name.split(' ').next().unwrap_or(i.name))).c(Col::Dim, format!("  {}: {}", i.name, i.blurb)));
            }
            self.send(id, lines);
            return Ok(());
        };
        let Some(to) = self.dock_on(dest) else {
            self.say1(id, Line::plain(format!("There's nowhere to land on {} just now.", dest.name)));
            return Ok(());
        };
        if let Some(p) = self.players.get_mut(&id) {
            p.travel = None;
            p.walk_to = None;
            p.fishing = None;
            p.voyage = Some(Voyage { to, island: dest.name, from: here.name, stride: 0 });
        }
        self.say1(id, Line::col(Col::Blue, "⛵ You climb into the little rowing boat and push off from the dock."));
        self.tell_here(x, y, id, Line::col(Col::Blue, format!("⛵ {name} rows away towards {}.", dest.name)));
        Ok(())
    }

    /// `go dock`: walk to this island's dock.
    pub(super) fn cmd_go_dock(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
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
        self.say1(id, Line::col(Col::Cyan, format!("🚶 You head down to {place}. (stop to stop)")));
        self.step_travellers();
        Ok(())
    }

    /// Everyone out on the water rows a little further.
    pub(super) fn row_boats(&mut self) {
        let sailors: Vec<u64> = self.players.iter().filter(|(_, p)| p.voyage.is_some()).map(|(&id, _)| id).collect();
        for id in sailors {
            let Some(v) = self.players.get_mut(&id).and_then(|p| p.voyage.as_mut()) else { continue };
            v.stride += 1;
            let (stride, island, from, to) = (v.stride, v.island, v.from, v.to);
            let line = match stride {
                2 => Some(format!("🌊 You row out past the shallows. {from} grows smaller behind you.")),
                4 => Some("🐦 Gulls wheel overhead, and the boat rocks gently on the swell.".to_string()),
                6 => Some(format!("🏝 {island} rises out of the sea ahead.")),
                _ => None,
            };
            if let Some(l) = line {
                self.say1(id, Line::col(Col::Blue, l));
            }
            if stride < CROSSING {
                continue;
            }
            let name = self.players[&id].name.clone();
            let left = (self.players[&id].x, self.players[&id].y);
            if let Some(p) = self.players.get_mut(&id) {
                p.voyage = None;
                p.x = to.0;
                p.y = to.1;
                p.sky = self.world.weather.sky_at(to.0, to.1);
            }
            self.say1(id, Line::col(Col::Blue, format!("⚓ You bump gently against the dock at {island}, and hop out.")));
            self.tell_here(to.0, to.1, id, Line::col(Col::Blue, format!("⛵ {name} arrives by boat from {from}.")));
            self.look(id);
            self.save_progress(id);
            self.status_near(id, &[left, to]);
            // On the way somewhere further: walk on from the dock.
            if let Some((hx, hy, place)) = self.players.get_mut(&id).and_then(|p| p.heading.take()) {
                if (hx, hy) != to {
                    if let Some(p) = self.players.get_mut(&id) {
                        p.walk_to = Some((hx, hy, place.clone()));
                    }
                    self.say1(id, Line::col(Col::Cyan, format!("🚶 On you go to {place}. (stop to stop)")));
                } else {
                    self.say1(id, Line::col(Col::Cyan, format!("🎉 You've arrived at {place}!")));
                }
            }
        }
    }

    /// Out on the water, walking isn't an option.
    pub(super) fn at_sea(&self, id: u64) -> bool {
        let sailing = self.players.get(&id).is_some_and(|p| p.voyage.is_some());
        if sailing {
            self.say1(id, Line::col(Col::Blue, "You're out at sea! Keep rowing, you're nearly there. 🌊"));
        }
        sailing
    }
}
