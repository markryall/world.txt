//! The village green: the spawn point, where newcomers arrive. The Island
//! Journal stands in the middle, with a village noticeboard and a long table
//! for gatherings beside it, so arriving reads as "a little village", and the
//! island's practical bits (notes, hooks, events, links for calls) have a
//! place to live. The rest of the island is left for pottering.
//!
//! It's set up once, the first time a world starts with this in it, and only
//! where there's room: anything people have built nearby stays as it is, and
//! keepers can rearrange or take it away afterwards.

use super::Game;
use crate::server::world::{DIRS, Trigger};

pub const NAME: &str = "The Village Green";
const DONE: &str = "village_green";
pub const BY: &str = "the island";

impl Game {
    pub(super) fn ensure_green(&mut self) -> anyhow::Result<()> {
        if self.world.db.meta(DONE).is_some() {
            return Ok(());
        }
        let (x, y) = self.world.spawn_point();
        if self.world.tile(x, y).is_some_and(|t| t.name.is_none()) {
            self.world.set_name(x, y, Some(NAME.into()))?;
        }
        // The board and the table each want a dry spot beside the middle,
        // nobody's home, with nothing of their kind there already.
        let spots: Vec<(i64, i64)> = DIRS
            .iter()
            .map(|d| (x + d.2, y + d.3))
            .filter(|&(sx, sy)| {
                let t = self.world.terrain(sx, sy);
                t.walkable() && !t.is_wet() && self.world.home_at(sx, sy).is_none() && self.world.things_at(sx, sy).count() < 6
            })
            .collect();
        let way = |(sx, sy): (i64, i64)| DIRS.iter().find(|d| (x + d.2, y + d.3) == (sx, sy)).map_or("nearby", |d| d.1);
        let mut free = spots.into_iter();
        let mut around = Vec::new();
        if let Some((bx, by)) = free.by_ref().find(|&(sx, sy)| !self.world.things_at(sx, sy).any(|t| t.is_board())) {
            self.world.add_thing(bx, by, "the village noticeboard".into(), None, Trigger::Always, BY, Some(super::boards::board_kind()))?;
            around.push(format!("the village noticeboard just to the {}", way((bx, by))));
        }
        let meeting = super::meetings::kind();
        if let Some((tx, ty)) = free.find(|&(sx, sy)| !self.world.things_at(sx, sy).any(|t| t.kind.as_deref() == Some(meeting))) {
            self.world.add_thing(tx, ty, "a long table for gatherings".into(), None, Trigger::Always, BY, Some(meeting))?;
            around.push(format!("a long table for gatherings to the {}", way((tx, ty))));
        }
        if !self.world.descriptions.contains_key(&(x, y)) {
            let mut text = "A village green, the grass worn into soft paths by everyone passing through. The Island Journal stands in the middle".to_string();
            if !around.is_empty() {
                text.push_str(&format!(", with {}", super::cases::and_list(&around)));
            }
            text.push('.');
            self.world.set_description(x, y, Some(text), BY, super::boards::now())?;
        }
        self.world.db.set_meta(DONE, "1")?;
        println!("🌳 laid out the village green");
        Ok(())
    }
}
