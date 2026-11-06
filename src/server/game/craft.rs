//! Crafting: turning gathered things into something new. Crafts go in the
//! satchel like anything else, and come alive when placed.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::nature::{self, Kind, Species};


#[derive(Clone, Copy)]
pub enum Need {
    /// Any find of this kind.
    Any(Kind),
    /// One particular find.
    Exactly(&'static str),
}

pub struct Recipe {
    pub makes: &'static str,
    pub needs: &'static [(Need, i64)],
}

use Need::*;

pub const RECIPES: &[Recipe] = &[
    Recipe { makes: "flower_crown", needs: &[(Any(Kind::Flower), 3)] },
    Recipe { makes: "bouquet", needs: &[(Any(Kind::Flower), 5)] },
    Recipe { makes: "wind_chime", needs: &[(Any(Kind::Shell), 3)] },
    Recipe { makes: "dreamcatcher", needs: &[(Any(Kind::Feather), 2), (Any(Kind::Flower), 1)] },
    Recipe { makes: "crystal_lantern", needs: &[(Exactly("quartz"), 1), (Exactly("pebble"), 2)] },
    Recipe { makes: "mushroom_soup", needs: &[(Any(Kind::Mushroom), 3)] },
    Recipe { makes: "shell_necklace", needs: &[(Any(Kind::Shell), 2), (Exactly("sea_glass"), 1)] },
    Recipe { makes: "cairn", needs: &[(Exactly("pebble"), 5)] },
    Recipe { makes: "conch_horn", needs: &[(Exactly("conch"), 1)] },
    Recipe { makes: "leaf_crown", needs: &[(Any(Kind::Leaf), 3)] },
    Recipe {
        makes: "garland",
        needs: &[(Any(Kind::Flower), 1), (Any(Kind::Feather), 1), (Any(Kind::Shell), 1), (Any(Kind::Stone), 1)],
    },
];

/// Things for a home, made from finds: place one and it's part of the room.
pub const FURNITURE: &[Recipe] = &[
    Recipe { makes: "feather_cushion", needs: &[(Any(Kind::Feather), 3)] },
    Recipe { makes: "toadstool_stool", needs: &[(Exactly("fly_agaric"), 1), (Any(Kind::Leaf), 1)] },
    Recipe { makes: "leaf_rug", needs: &[(Any(Kind::Leaf), 5)] },
    Recipe { makes: "door_wreath", needs: &[(Any(Kind::Flower), 2), (Any(Kind::Leaf), 2)] },
    Recipe { makes: "vase_of_flowers", needs: &[(Exactly("sea_glass"), 1), (Any(Kind::Flower), 3)] },
    Recipe { makes: "shell_mirror", needs: &[(Exactly("sea_glass"), 2), (Any(Kind::Shell), 2)] },
    Recipe { makes: "pinecone_lamp", needs: &[(Exactly("pinecone"), 2), (Exactly("firefly"), 1)] },
    Recipe { makes: "driftwood_table", needs: &[(Exactly("driftwood"), 2), (Any(Kind::Stone), 1)] },
    Recipe { makes: "pebble_doorstop", needs: &[(Exactly("pebble"), 2), (Any(Kind::Feather), 1)] },
    Recipe { makes: "butterfly_bush", needs: &[(Any(Kind::Butterfly), 2), (Any(Kind::Flower), 2)] },
];

/// Every recipe, crafts first, then things for a home.
fn all_recipes() -> impl Iterator<Item = &'static Recipe> {
    RECIPES.iter().chain(FURNITURE)
}

pub(super) fn need_label(need: Need, n: i64) -> String {
    let (one, many) = match need {
        Any(k) => match k {
            Kind::Flower => ("flower", "flowers"),
            Kind::Shell => ("shell", "shells"),
            Kind::Feather => ("feather", "feathers"),
            Kind::Mushroom => ("mushroom", "mushrooms"),
            Kind::Stone => ("stone", "stones"),
            Kind::Leaf => ("leaf", "leaves"),
            Kind::Berry => ("berry", "berries"),
            Kind::Fish => ("fish", "fish"),
            Kind::Herb => ("herb", "herbs"),
            Kind::Butterfly => ("butterfly", "butterflies"),
            Kind::Bug => ("bug", "bugs"),
            _ => ("thing", "things"),
        },
        Exactly(id) => {
            let name = nature::by_id(id).map_or(id, |s| s.name);
            return if n == 1 { name.to_string() } else { format!("{n} × {name}") };
        }
    };
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

fn fits(need: Need, s: &Species) -> bool {
    match need {
        Any(k) => s.kind == k,
        Exactly(id) => s.id == id,
    }
}

/// Work out which satchel items a recipe would use: exact needs first, then
/// "any" needs from the commonest finds, so rare things aren't used up by accident.
pub fn plan(recipe: &Recipe, satchel: &[(String, i64)]) -> Option<Vec<(&'static str, i64)>> {
    plan_needs(recipe.needs, satchel)
}

/// The same, for any list of needs (a building's, say).
pub fn plan_needs(needs: &[(Need, i64)], satchel: &[(String, i64)]) -> Option<Vec<(&'static str, i64)>> {
    let mut left: Vec<(&'static Species, i64)> =
        satchel.iter().filter_map(|(id, n)| nature::by_id(id).map(|s| (s, *n))).collect();
    let mut needs: Vec<(Need, i64)> = needs.to_vec();
    needs.sort_by_key(|(n, _)| matches!(n, Any(_)));
    let mut used: Vec<(&'static str, i64)> = Vec::new();
    for (need, mut n) in needs {
        let rank = |s: &Species| match s.rarity {
            nature::Rarity::Common => 0,
            nature::Rarity::Uncommon => 1,
            nature::Rarity::Rare => 2,
        };
        let mut options: Vec<usize> = (0..left.len()).filter(|&i| fits(need, left[i].0) && left[i].1 > 0).collect();
        options.sort_by_key(|&i| (rank(left[i].0), -left[i].1));
        for i in options {
            let take = n.min(left[i].1);
            left[i].1 -= take;
            n -= take;
            used.push((left[i].0.id, take));
            if n == 0 {
                break;
            }
        }
        if n > 0 {
            return None;
        }
    }
    Some(used)
}

impl Game {
    pub(super) fn cmd_recipes(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((_, _, _name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let satchel = self.world.db.satchel(&user)?;
        let mut lines = vec![Line::new().b(Col::Magenta, "✂️ Things you can craft")];
        for (i, r) in all_recipes().enumerate() {
            if i == RECIPES.len() {
                lines.push(Line::new().b(Col::Magenta, "🪑 For your home"));
            }
            let Some(s) = nature::by_id(r.makes) else { continue };
            let needs: Vec<String> = r.needs.iter().map(|(n, c)| need_label(*n, *c)).collect();
            let ready = plan(r, &satchel).is_some();
            lines.push(
                Line::new()
                    .c(if ready { Col::Green } else { Col::Dim }, if ready { "  ✓ " } else { "    " })
                    .b(if ready { Col::Yellow } else { Col::Plain }, s.name)
                    .c(Col::Dim, format!("  ← {}", needs.join(" + "))),
            );
        }
        lines.push(Line::col(Col::Dim, "  craft <name> to make one. Placed crafts come alive!"));
        lines.push(Line::col(Col::Dim, "  For food, make a campfire and type cook."));
        self.send(id, lines);
        Ok(())
    }

    pub(super) fn cmd_craft(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let w = crate::server::world::strip_article(what.trim()).to_lowercase();
        if w.is_empty() {
            return self.cmd_recipes(id);
        }
        let Some((recipe, s)) = all_recipes()
            .filter_map(|r| nature::by_id(r.makes).map(|s| (r, s)))
            .find(|(_, s)| s.name.contains(&w) || s.id == w.replace(' ', "_"))
        else {
            self.say1(id, Line::plain(format!("You don't know how to craft '{w}'. Type recipes to see what you can make.")));
            return Ok(());
        };
        let satchel = self.world.db.satchel(&user)?;
        let Some(used) = plan(recipe, &satchel) else {
            let needs: Vec<String> = recipe.needs.iter().map(|(n, c)| need_label(*n, *c)).collect();
            self.say1(id, Line::plain(format!("A {} needs {}. Keep gathering! 🧺", s.name, needs.join(" + "))));
            return Ok(());
        };
        for (sid, n) in &used {
            self.world.db.satchel_add(&user, sid, -n)?;
        }
        self.world.db.satchel_add(&user, s.id, 1)?;
        self.say1(id, Line::new().c(Col::Yellow, "✂️ You craft ").b(Col::Yellow, nature::a(s.name)).c(Col::Yellow, "! It's in your satchel: place it, or give it to someone."));
        self.tell_here(x, y, id, Line::col(Col::Yellow, format!("✂️ {name} crafts {}.", nature::a(s.name))));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bag(items: &[(&str, i64)]) -> Vec<(String, i64)> {
        items.iter().map(|(i, n)| (i.to_string(), *n)).collect()
    }

    #[test]
    fn every_recipe_makes_a_real_craft() {
        for r in all_recipes() {
            let s = nature::by_id(r.makes).unwrap_or_else(|| panic!("{} is missing", r.makes));
            assert_eq!(s.kind, Kind::Craft);
            for (need, _) in r.needs {
                if let Exactly(id) = need {
                    assert!(nature::by_id(id).is_some(), "{id}");
                }
            }
        }
    }

    #[test]
    fn spends_common_shells_before_rare_ones() {
        let chime = RECIPES.iter().find(|r| r.makes == "wind_chime").unwrap();
        let used = plan(chime, &bag(&[("conch", 1), ("cockle", 2), ("scallop", 1)])).unwrap();
        assert!(!used.iter().any(|(id, _)| *id == "conch"), "{used:?}");
        assert!(plan(chime, &bag(&[("cockle", 2)])).is_none());
    }

    #[test]
    fn exact_needs_are_kept_for_themselves() {
        let necklace = RECIPES.iter().find(|r| r.makes == "shell_necklace").unwrap();
        // The sea glass must go to the "exactly" slot, not be counted as an "any shell".
        let used = plan(necklace, &bag(&[("sea_glass", 1), ("cockle", 2)])).unwrap();
        assert!(used.contains(&("sea_glass", 1)) && used.contains(&("cockle", 2)), "{used:?}");
    }
}
