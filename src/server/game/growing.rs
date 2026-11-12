//! Planting, and hearing how it's grown: straight away if you're on the
//! island, or as "while you were away" when you next arrive.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::gardens::{self, Kind};
use crate::server::terrain::Terrain;

impl Game {
    pub(super) fn cmd_plant(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else {
            return Ok(());
        };
        let here = self.world.terrain(x, y);
        use Terrain::*;
        if matches!(here, Water | Pond | DeepWater | Stone) {
            self.say1(id, Line::plain("Nothing would grow here."));
            return Ok(());
        }
        let w = super::strip_article(&what.to_lowercase()).to_string();
        let Some((kind, chosen)) = gardens::wanted(&w) else {
            if w.contains("grass") || w.is_empty() {
                return self.reshape(
                    id,
                    Grass,
                    "You sow grass seed. A soft green carpet spreads out. 🌱",
                    "sows grass, and green spreads around.",
                );
            }
            self.say1(id, Line::plain("You can plant a tree (oak, birch, willow, apple, cherry, pine, rowan), flowers (bluebells, poppies, sunflowers, lavender, foxgloves) or grass."));
            return Ok(());
        };
        let plant =
            chosen.unwrap_or_else(|| gardens::choose(kind, self.near_water(x, y), self.rng.next()));
        let first = gardens::label(plant, 0);
        let (to, mine, theirs) = match kind {
            Kind::Tree => (
                Forest,
                format!(
                    "You plant {first} and firm the earth around it. It'll take its time growing. 🌱"
                ),
                format!("plants {first}."),
            ),
            Kind::Flowers => (
                Meadow,
                format!(
                    "You plant {}. Little green shoots are already peeping up. 🌱",
                    plant.name
                ),
                format!("plants {}.", plant.name),
            ),
        };
        self.reshape(id, to, &mine, &theirs)?;
        let user = self.user(id);
        self.world
            .plant(x, y, plant.id, &user, super::boards::now())?;
        Ok(())
    }

    /// For `look`: what's growing here, and who planted it.
    pub(super) fn garden_lines_here(&self, id: u64) -> Vec<Line> {
        let Some(p) = self.players.get(&id) else {
            return vec![];
        };
        let Some(row) = self.world.plantings.get(&(p.x, p.y)) else {
            return vec![];
        };
        let now = super::boards::now();
        let Some((plant, stage)) = gardens::stage_of(row, now) else {
            return vec![];
        };
        let who = if row.by == p.user {
            "You".to_string()
        } else {
            self.world.nick(&row.by)
        };
        // Someone's own words for the spot hide how it looks: say what it is.
        let unseen = self.world.descriptions.contains_key(&(p.x, p.y));
        vec![Line::col(
            Col::Green,
            format!(
                "🌱 {}",
                gardens::planted(plant, stage, &who, &gardens::ago(now - row.at), unseen)
            ),
        )]
    }

    /// Tell planters who are here when something of theirs grows a stage.
    pub(super) fn growing_tick(&mut self) {
        let now = super::boards::now();
        let grown: Vec<(i64, i64, String, &'static gardens::Plant, i64)> = self
            .world
            .plantings
            .values()
            .filter_map(|r| {
                gardens::stage_of(r, now)
                    .filter(|(_, s)| *s > r.told)
                    .map(|(p, s)| (r.x, r.y, r.by.clone(), p, s))
            })
            .collect();
        for (x, y, by, plant, stage) in grown {
            let Some((&pid, _)) = self.players.iter().find(|(_, p)| p.user == by) else {
                continue;
            };
            let place = self.world.place_title(x, y);
            let line = Line::col(
                Col::Green,
                format!(
                    "{} The {} you planted at {place} {}.",
                    icon(plant.kind),
                    plant.name,
                    gardens::grew(plant, stage)
                ),
            );
            self.say1(pid, line);
            if let Err(e) = self.world.told_planter(x, y, stage) {
                eprintln!("garden at {x},{y}: {e}");
            }
        }
    }

    /// On arriving: how things they planted have grown since.
    pub(super) fn garden_news(&mut self, id: u64) {
        let Some(user) = self.players.get(&id).map(|p| p.user.clone()) else {
            return;
        };
        let now = super::boards::now();
        let mut grown: Vec<(i64, i64, &'static gardens::Plant, i64, i64)> = self
            .world
            .plantings
            .values()
            .filter(|r| r.by == user)
            .filter_map(|r| {
                gardens::stage_of(r, now)
                    .filter(|(_, s)| *s > r.told)
                    .map(|(p, s)| (r.x, r.y, p, s, r.at))
            })
            .collect();
        if grown.is_empty() {
            return;
        }
        grown.sort_by_key(|g| g.4);
        let mut lines = vec![Line::new().b(Col::Green, "🌿 While you were away:")];
        for (x, y, plant, stage, _) in &grown {
            let place = self.world.place_title(*x, *y);
            lines.push(Line::col(
                Col::Green,
                format!(
                    "  {} the {} you planted at {place} {}.",
                    icon(plant.kind),
                    plant.name,
                    gardens::grew(plant, *stage)
                ),
            ));
        }
        self.send(id, lines);
        for (x, y, _, stage, _) in grown {
            let _ = self.world.told_planter(x, y, stage);
        }
    }
}

fn icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Tree => "🌳",
        Kind::Flowers => "🌼",
    }
}
