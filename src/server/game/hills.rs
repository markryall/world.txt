//! The high ground: hills and stony heights. From up here you can `look
//! out` over the whole island, a `shout` comes echoing back, and a round
//! stone can be set rolling down the slope (`roll stone`). None of it is in
//! the help; the hills drop hints.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::terrain::Terrain;
use crate::server::world::DIRS;

/// How far a rolled stone goes, at most.
const ROLL_MAX: i64 = 5;

pub fn high(t: Terrain) -> bool {
    matches!(t, Terrain::Hill | Terrain::Stone)
}

/// The last word of a shout, for its echo: "hello there!" → "there".
pub fn echo_word(msg: &str) -> Option<String> {
    let w: String = msg
        .split_whitespace()
        .last()?
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '\'')
        .collect();
    (!w.is_empty()).then(|| w.to_lowercase())
}

impl Game {
    /// The hill commands, if this is one.
    pub(super) fn hills_command(
        &mut self,
        id: u64,
        verb: &str,
        rest: &str,
    ) -> Option<anyhow::Result<()>> {
        let r = rest.trim().to_lowercase();
        let r = r.trim_start_matches("a ").trim_start_matches("the ");
        Some(match verb {
            "look" | "gaze" | "peer"
                if matches!(
                    r,
                    "out" | "around" | "far" | "out over the island" | "at the view"
                ) && (r == "out" || self.up_high(id)) =>
            {
                self.cmd_look_out(id)
            }
            "lookout" | "survey" | "view" => self.cmd_look_out(id),
            "roll" | "bowl"
                if ["stone", "rock", "pebble", "boulder"]
                    .iter()
                    .any(|w| r.starts_with(w)) =>
            {
                self.cmd_roll(id)
            }
            _ => return None,
        })
    }

    fn up_high(&self, id: u64) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| high(self.world.terrain(p.x, p.y)))
    }

    fn cmd_look_out(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else {
            return Ok(());
        };
        if !self.up_high(id) {
            self.say1(id, Line::plain("You can't see far from down here. Somewhere high would have a view: the hills, maybe."));
            return Ok(());
        }
        let here = crate::server::islands::island_at(x, y);
        let mut lines = vec![Line::new().b(
            Col::Magenta,
            format!("⛰  From up here you can see the whole of {}.", here.name),
        )];
        let mut seen: Vec<String> = Vec::new();
        let dir_to = |tx: i64, ty: i64| {
            let (dx, dy) = ((tx - x).signum(), (ty - y).signum());
            DIRS.iter()
                .find(|d| (d.2, d.3) == (dx, dy))
                .map_or("right below you", |d| d.1)
        };
        for p in self.players.values() {
            if crate::server::islands::island_at(p.x, p.y).id != here.id || (p.x, p.y) == (x, y) {
                continue;
            }
            if let Some(k) = &p.fun.kite {
                seen.push(format!(
                    "{}'s kite bobbing on the wind to the {}",
                    p.name,
                    dir_to(k.at.0, k.at.1)
                ));
            } else {
                seen.push(format!(
                    "{}, a tiny figure to the {}",
                    p.name,
                    dir_to(p.x, p.y)
                ));
            }
        }
        for t in self
            .cooking
            .keys()
            .filter_map(|k| self.world.things.iter().find(|t| t.id == *k))
        {
            if crate::server::islands::island_at(t.x, t.y).id == here.id {
                seen.push(format!(
                    "a thread of smoke rising to the {}",
                    dir_to(t.x, t.y)
                ));
            }
        }
        seen.dedup();
        seen.truncate(5);
        if !seen.is_empty() {
            lines.push(Line::col(
                Col::Cyan,
                format!("You spot {}.", super::cases::and_list(&seen)),
            ));
        }
        let others: Vec<&str> = crate::server::islands::ISLANDS
            .iter()
            .filter(|i| i.id != here.id)
            .map(|i| i.name)
            .collect();
        lines.push(Line::col(
            Col::Blue,
            format!(
                "Out across the water, {} sit hazy on the horizon.",
                super::cases::and_list(&others.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            ),
        ));
        let people: Vec<(i64, i64)> = self
            .players
            .iter()
            .filter(|(pid, _)| **pid != id)
            .map(|(_, o)| (o.x, o.y))
            .collect();
        lines.extend(self.world.island_map((x, y), &people));
        self.send(id, lines);
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Dim,
                format!("{who} shades their eyes and gazes out over the island."),
            ),
        );
        Ok(())
    }

    /// After a shout from somewhere high, the hills shout back.
    pub(super) fn echo(&self, id: u64, msg: &str) {
        let Some(p) = self.players.get(&id) else {
            return;
        };
        if !high(self.world.terrain(p.x, p.y)) {
            return;
        }
        let Some(w) = echo_word(msg) else { return };
        let line = Line::col(Col::Dim, format!("…{w}… {w}… {w}…  the hills call back."));
        self.say1(id, line.clone());
        self.tell_here(p.x, p.y, id, line);
    }

    fn cmd_roll(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else {
            return Ok(());
        };
        if !self.up_high(id) {
            self.say1(
                id,
                Line::plain(
                    "It just sits there. Stones roll better downhill: try the top of a slope.",
                ),
            );
            return Ok(());
        }
        // Downhill: towards ground that isn't high, any way that has some.
        let ways: Vec<(&str, i64, i64)> = DIRS
            .iter()
            .filter(|d| (1..=ROLL_MAX).any(|n| !high(self.world.terrain(x + d.2 * n, y + d.3 * n))))
            .map(|d| (d.1, d.2, d.3))
            .collect();
        if ways.is_empty() {
            self.say1(
                id,
                Line::plain("It's high ground all around. The stone wobbles, and stays put."),
            );
            return Ok(());
        }
        let (long, dx, dy) = *self.rng.pick(&ways);
        let mut at = (x, y);
        for _ in 0..ROLL_MAX {
            let next = (at.0 + dx, at.1 + dy);
            at = next;
            if !high(self.world.terrain(next.0, next.1)) {
                break;
            }
        }
        let end = match self.world.terrain(at.0, at.1) {
            Terrain::Pond => "and lands in the pond with a tremendous plop",
            t if t.is_wet() => "and splashes into the water",
            Terrain::Forest => "and thuds to a stop among the trees",
            Terrain::Sand => "and buries itself in the sand",
            Terrain::Meadow => "and settles in the long grass, startling a butterfly",
            _ => "and rolls to a stop at the bottom",
        };
        self.say1(id, Line::col(Col::Brown, format!("🪨 You set a round stone rolling. It bumps and bounds away down the slope to the {long}, {end}.")));
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Brown,
                format!("🪨 {who} sets a stone rolling down the hill to the {long}."),
            ),
        );
        if at != (x, y) {
            self.tell_here(at.0, at.1, id, Line::col(Col::Brown, "🪨 A stone comes bouncing down the hill and rolls to a stop right by your feet!"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn echoes_take_the_last_word() {
        assert_eq!(echo_word("hello there!").as_deref(), Some("there"));
        assert_eq!(echo_word("  Coo-ee  ").as_deref(), Some("cooee"));
        assert_eq!(echo_word("!!!"), None);
    }
}
