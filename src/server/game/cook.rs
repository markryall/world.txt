//! Cooking, the simple way: make a campfire or a clay oven wherever you like,
//! then `cook` something from what you've gathered. While it cooks, it smells
//! (going on, halfway, done), and the smell drifts to everyone nearby. Then
//! there's some for you, and some to `serve` whoever's with you.

use super::Game;
use super::craft::{Need, Recipe, need_label, plan};
use crate::protocol::{Col, Line};
use crate::server::nature::{self, Kind};
use std::time::{Duration, Instant};

pub const FIRE: &str = "campfire";
pub const OVEN: &str = "oven";
/// How far a cooking smell drifts, in steps.
const SMELL: i64 = 6;

/// Which kind of kitchen a made thing is, if any.
pub fn kitchen(name: &str) -> Option<&'static str> {
    let n = name.to_lowercase();
    if n.contains("oven") {
        Some(OVEN)
    } else if [
        "campfire",
        "camp fire",
        "camp stove",
        "stove",
        "fire pit",
        "firepit",
        "bonfire",
        "hearth",
    ]
    .iter()
    .any(|w| n.contains(w))
    {
        Some(FIRE)
    } else {
        None
    }
}

pub struct Dish {
    pub makes: &'static str,
    on: &'static str,
    needs: &'static [(Need, i64)],
    secs: u64,
    serves: i64,
    /// What drifts out: as it goes on, halfway, and when it's done.
    smells: [&'static str; 3],
    eaten: &'static str,
}

use Need::*;

pub const DISHES: &[Dish] = &[
    Dish {
        makes: "mint_tea",
        on: FIRE,
        needs: &[(Exactly("wild_mint"), 1)],
        secs: 15,
        serves: 2,
        smells: [
            "☕ A kettle ticks and hisses as it warms over the fire.",
            "☕ Steam curls from the kettle's spout, bright and green with mint.",
            "☕ The kettle whistles! A cool, clean smell of mint tea drifts all around.",
        ],
        eaten: "You sip the mint tea. It's hot and bright, and tastes like a cool morning.",
    },
    Dish {
        makes: "toasted_acorns",
        on: FIRE,
        needs: &[(Exactly("acorn"), 3)],
        secs: 15,
        serves: 3,
        smells: [
            "🌰 Acorns rattle into a pan over the fire.",
            "🌰 They start to pop and crackle, and a warm, nutty smell drifts about.",
            "🌰 The acorns are toasted dark and shiny, smelling like an autumn afternoon.",
        ],
        eaten: "You crunch a toasted acorn. Warm, nutty, a little smoky.",
    },
    Dish {
        makes: "grilled_fish",
        on: FIRE,
        needs: &[(Any(Kind::Fish), 1)],
        secs: 20,
        serves: 2,
        smells: [
            "🐟 A fish hisses as it hits the hot stones over the coals.",
            "🐟 Woodsmoke and sizzling fish drift on the breeze, and the skin begins to crackle.",
            "🐟 The fish is done: crisp and blistered gold, smelling of smoke and the sea.",
        ],
        eaten: "You eat the grilled fish with your fingers. Smoky, salty, perfect.",
    },
    Dish {
        makes: "mushroom_stew",
        on: FIRE,
        needs: &[(Any(Kind::Mushroom), 2), (Exactly("thyme"), 1)],
        secs: 30,
        serves: 4,
        smells: [
            "🍲 A pot settles on the fire and begins to murmur.",
            "🍲 The stew bubbles away, thick with mushrooms and thyme. The smell is rich, earthy and impossible to ignore.",
            "🍲 The stew is ready, dark and glossy, with steam rising in a fragrant cloud of thyme.",
        ],
        eaten: "You eat the stew slowly. It's warm all the way down, earthy and herby.",
    },
    Dish {
        makes: "berry_crumble",
        on: OVEN,
        needs: &[(Any(Kind::Berry), 3), (Exactly("honeycomb"), 1)],
        secs: 40,
        serves: 6,
        smells: [
            "🥧 The clay oven glows as a crumble goes in.",
            "🥧 A warm smell of baking drifts from the oven: browning crumbs, bubbling fruit, honey.",
            "🥧 The crumble comes out golden, its edges bubbling purple. Everything smells of warm berries and honey.",
        ],
        eaten: "You eat a slice of berry crumble. Crunchy on top, soft and sharp-sweet underneath.",
    },
    Dish {
        makes: "honey_cake",
        on: OVEN,
        needs: &[(Exactly("honeycomb"), 2), (Exactly("acorn"), 1)],
        secs: 35,
        serves: 4,
        smells: [
            "🍯 A honey cake slides into the clay oven.",
            "🍯 The air turns sweet and golden as the cake rises, smelling of honey and toasted nuts.",
            "🍯 Out comes the honey cake, high and golden, filling the air with warmth.",
        ],
        eaten: "You eat a slice of honey cake. Soft, sticky and sweet as summer.",
    },
    Dish {
        makes: "herb_tea",
        on: FIRE,
        needs: &[(Any(Kind::Herb), 2)],
        secs: 15,
        serves: 2,
        smells: [
            "🍵 A kettle settles over the fire with a handful of herbs.",
            "🍵 The kettle starts to sing, and a green, leafy smell drifts out.",
            "🍵 The herb tea is ready, steaming and fragrant.",
        ],
        eaten: "You sip the herb tea. It tastes green and gentle, like a walk in the garden.",
    },
    Dish {
        makes: "elderflower_cordial",
        on: FIRE,
        needs: &[(Exactly("elderflower"), 2)],
        secs: 20,
        serves: 4,
        smells: [
            "🌼 Creamy elderflowers go into a pot of water over the fire.",
            "🌼 A sweet, flowery smell rises with the steam, like a summer hedgerow.",
            "🌼 The cordial is ready: pale gold, cooling in the pot, smelling of June.",
        ],
        eaten: "You drink a glass of elderflower cordial. Sweet and flowery, and tastes like a long summer evening.",
    },
    Dish {
        makes: "cloudberry_jam",
        on: FIRE,
        needs: &[(Exactly("cloudberry"), 2), (Exactly("honeycomb"), 1)],
        secs: 30,
        serves: 4,
        smells: [
            "🍯 Cloudberries and honey tumble into a pan over the fire.",
            "🍯 The jam bubbles and thickens, glowing amber, smelling of honey and sunshine.",
            "🍯 The cloudberry jam is set, golden as a sunset in a jar.",
        ],
        eaten: "You eat a spoonful of cloudberry jam. Tart and honeyed, like nothing on any other island.",
    },
    Dish {
        makes: "samphire_supper",
        on: FIRE,
        needs: &[(Exactly("flying_fish"), 1), (Exactly("samphire"), 1)],
        secs: 25,
        serves: 2,
        smells: [
            "🐟 A flying fish and a bundle of samphire go onto the hot stones.",
            "🐟 The fish crackles and the samphire hisses, smelling of salt and smoke.",
            "🐟 Supper's ready: crisp fish on a bed of bright green samphire, smelling of the sea.",
        ],
        eaten: "You eat the samphire fish supper with your fingers. Salty, smoky, and somehow tastes of the whole beach.",
    },
    Dish {
        makes: "herby_mushrooms",
        on: FIRE,
        needs: &[(Any(Kind::Mushroom), 2), (Exactly("wild_oregano"), 1)],
        secs: 20,
        serves: 3,
        smells: [
            "🍄 Mushrooms sizzle in a pan with a scatter of wild oregano.",
            "🍄 A warm, peppery smell drifts off the pan as the mushrooms turn golden.",
            "🍄 The herby mushrooms are done, glossy and fragrant.",
        ],
        eaten: "You eat the herby mushrooms. Earthy, peppery, and warm as a sunny rock.",
    },
    Dish {
        makes: "hazelnut_cake",
        on: OVEN,
        needs: &[(Exactly("hazelnut"), 3), (Exactly("honeycomb"), 1)],
        secs: 35,
        serves: 4,
        smells: [
            "🌰 A hazelnut honey cake slides into the clay oven.",
            "🌰 The air fills with toasted nuts and warm honey.",
            "🌰 Out comes the hazelnut honey cake, nutty and golden and still warm.",
        ],
        eaten: "You eat a slice of hazelnut honey cake. Crunchy, sticky, and very hard to stop at one slice.",
    },
];

fn recipe(d: &Dish) -> Recipe {
    Recipe {
        makes: d.makes,
        needs: d.needs,
    }
}

/// Something on the go at a campfire or oven.
pub struct Cooking {
    dish: &'static Dish,
    user: String,
    /// What went in, so it can come back if the kitchen's unmade.
    used: Vec<(&'static str, i64)>,
    started: Instant,
    halfway: bool,
}

impl Game {
    /// The campfire or oven here, if there is one: (thing id, which kind).
    pub(super) fn kitchen_here(&self, x: i64, y: i64) -> Option<(i64, &'static str)> {
        self.world
            .things
            .iter()
            .filter(|t| t.x == x && t.y == y)
            .find_map(|t| {
                t.kind
                    .as_deref()
                    .and_then(|k| [FIRE, OVEN].into_iter().find(|w| *w == k))
                    .map(|k| (t.id, k))
            })
    }

    /// Everyone within smelling distance of (x, y).
    fn waft(&self, x: i64, y: i64, line: Line) {
        for p in self.players.values() {
            if (p.x - x).abs().max((p.y - y).abs()) <= SMELL {
                let _ = p.tx.send(crate::protocol::ToClient::Lines {
                    lines: vec![line.clone()],
                });
            }
        }
    }

    /// `cook` on its own: what you could cook here, and with what.
    fn menu(&mut self, id: u64, here: Option<&'static str>) -> anyhow::Result<()> {
        let satchel = self.world.db.satchel(&self.user(id))?;
        let mut lines = vec![Line::new().b(Col::Magenta, "🍳 Things to cook")];
        for d in DISHES {
            let Some(s) = nature::by_id(d.makes) else {
                continue;
            };
            let ready = plan(&recipe(d), &satchel).is_some();
            let needs: Vec<String> = d.needs.iter().map(|(n, c)| need_label(*n, *c)).collect();
            let place = if d.on == OVEN {
                "clay oven"
            } else {
                "campfire"
            };
            lines.push(
                Line::new()
                    .c(
                        if ready { Col::Green } else { Col::Dim },
                        if ready { "  ✓ " } else { "    " },
                    )
                    .b(if ready { Col::Yellow } else { Col::Plain }, s.name)
                    .c(Col::Dim, format!("  ← {} · {place}", needs.join(" + "))),
            );
        }
        lines.push(Line::col(
            Col::Dim,
            match here {
                Some(_) => "  cook <name> to start. It takes a few moments, and smells wonderful.",
                None => "  Make a campfire or a clay oven to cook at: make a campfire",
            },
        ));
        self.send(id, lines);
        Ok(())
    }

    /// `cook mint tea`, at a campfire or clay oven.
    pub(super) fn cmd_cook(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else {
            return Ok(());
        };
        let kitchen = self.kitchen_here(x, y);
        let w = crate::server::world::strip_article(what.trim()).to_lowercase();
        if w.is_empty() {
            return self.menu(id, kitchen.map(|k| k.1));
        }
        let Some(dish) = DISHES.iter().find(|d| {
            nature::by_id(d.makes).is_some_and(|s| {
                s.name.contains(&w)
                    || s.id == w.replace(' ', "_")
                    || s.id.replace('_', " ").contains(&w)
            })
        }) else {
            self.say1(
                id,
                Line::plain(format!(
                    "You don't know how to cook '{w}'. Type cook to see what you can make."
                )),
            );
            return Ok(());
        };
        let s = nature::by_id(dish.makes).expect("a real dish");
        let Some((thing, kind)) = kitchen else {
            let place = if dish.on == OVEN {
                "a clay oven"
            } else {
                "a campfire"
            };
            self.say1(
                id,
                Line::plain(format!(
                    "You'll need {place} for that. Make one right here: make {place}"
                )),
            );
            return Ok(());
        };
        if kind != dish.on {
            let place = if dish.on == OVEN {
                "a clay oven"
            } else {
                "a campfire"
            };
            self.say1(
                id,
                Line::plain(format!(
                    "That needs {place}, not this. Make one nearby: make {place}"
                )),
            );
            return Ok(());
        }
        if self.cooking.contains_key(&thing) {
            self.say1(
                id,
                Line::plain(
                    "Something's already cooking here. Give it a moment, it'll be worth it.",
                ),
            );
            return Ok(());
        }
        let user = self.user(id);
        let satchel = self.world.db.satchel(&user)?;
        let Some(used) = plan(&recipe(dish), &satchel) else {
            let needs: Vec<String> = dish.needs.iter().map(|(n, c)| need_label(*n, *c)).collect();
            self.say1(
                id,
                Line::plain(format!(
                    "For {} you'll need {}. Keep an eye out while you wander 🧺",
                    nature::a(s.name),
                    needs.join(" + ")
                )),
            );
            return Ok(());
        };
        for (sid, n) in &used {
            self.world.db.satchel_add(&user, sid, -n)?;
        }
        self.cooking.insert(
            thing,
            Cooking {
                dish,
                user,
                used,
                started: Instant::now(),
                halfway: false,
            },
        );
        self.say1(
            id,
            Line::col(
                Col::Yellow,
                format!("🍳 You start cooking {}.", nature::a(s.name)),
            ),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(Col::Yellow, format!("🍳 {who} starts cooking.")),
        );
        self.waft(x, y, Line::col(Col::Pink, dish.smells[0]));
        Ok(())
    }

    /// Cooking carries on: smells halfway, and the dish when it's done.
    pub(super) fn cooking_tick(&mut self) {
        let mut done = Vec::new();
        for (thing, c) in self.cooking.iter_mut() {
            let left = Duration::from_secs(c.dish.secs).saturating_sub(c.started.elapsed());
            if left.is_zero() {
                done.push(*thing);
            } else if !c.halfway && c.started.elapsed() >= Duration::from_secs(c.dish.secs / 2) {
                c.halfway = true;
                if let Some(t) = self.world.things.iter().find(|t| t.id == *thing) {
                    let (x, y) = (t.x, t.y);
                    let line = Line::col(Col::Pink, c.dish.smells[1]);
                    for p in self.players.values() {
                        if (p.x - x).abs().max((p.y - y).abs()) <= SMELL {
                            let _ = p.tx.send(crate::protocol::ToClient::Lines {
                                lines: vec![line.clone()],
                            });
                        }
                    }
                }
            }
        }
        for thing in done {
            let Some(c) = self.cooking.remove(&thing) else {
                continue;
            };
            let Some(s) = nature::by_id(c.dish.makes) else {
                continue;
            };
            let at = self
                .world
                .things
                .iter()
                .find(|t| t.id == thing)
                .map(|t| (t.x, t.y));
            if let Some((x, y)) = at {
                self.waft(x, y, Line::col(Col::Pink, c.dish.smells[2]));
            }
            // What fits goes in the cook's satchel; the rest is left by the
            // fire for whoever comes by.
            let fits = self.satchel_room(&c.user).min(c.dish.serves);
            let left = c.dish.serves - fits;
            if fits > 0 && self.world.db.satchel_add(&c.user, s.id, fits).is_err() {
                continue;
            }
            if let Some((x, y)) = at.filter(|_| left > 0) {
                for _ in 0..left
                    .min(12 - self.world.things_at(x, y).count() as i64)
                    .max(0)
                {
                    let _ = self.world.add_thing(
                        x,
                        y,
                        nature::a(s.name),
                        Some(s.here.to_string()),
                        crate::server::world::Trigger::Always,
                        &c.user,
                        None,
                    );
                }
            }
            let first = self.world.db.journal_add(&c.user, s.id).unwrap_or(false);
            let cook = self
                .players
                .iter()
                .find(|(_, p)| p.user == c.user)
                .map(|(id, _)| *id);
            if let Some(id) = cook {
                let line = match (fits, left) {
                    (_, 0) => {
                        let some = if fits == 1 { nature::a(s.name) } else { format!("{fits} × {}", s.name) };
                        format!("🍽 Ready! {some} in your satchel. eat it, or serve it to whoever's here.")
                    }
                    (0, _) => "🍽 Ready! Your satchel's too full to take any, so it's left by the fire for whoever comes by.".to_string(),
                    _ => format!("🍽 Ready! {fits} × {} in your satchel, which is all that fits: the rest is left by the fire for whoever comes by.", s.name),
                };
                self.say1(id, Line::col(Col::Green, line));
                if first {
                    self.say1(
                        id,
                        Line::new()
                            .b(Col::Yellow, "📖 New in your journal: ")
                            .b(Col::White, crate::server::world::capitalise(s.name)),
                    );
                }
            }
        }
    }

    /// `eat stew`
    pub(super) fn cmd_eat(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let Some(s) = self.food_in_satchel(&user, what)? else {
            self.say1(
                id,
                Line::plain("You've nothing like that to eat. Cook something! (cook)"),
            );
            return Ok(());
        };
        self.world.db.satchel_add(&user, s.id, -1)?;
        let eaten = DISHES
            .iter()
            .find(|d| d.makes == s.id)
            .map_or("Delicious.", |d| d.eaten);
        self.say1(id, Line::col(Col::Pink, format!("😋 {eaten}")));
        self.tell_here(
            x,
            y,
            id,
            Line::col(Col::Pink, format!("😋 {who} enjoys {}.", nature::a(s.name))),
        );
        Ok(())
    }

    /// `serve crumble`: a helping for everyone standing with you.
    pub(super) fn cmd_serve(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let Some(s) = self.food_in_satchel(&user, what)? else {
            self.say1(
                id,
                Line::plain("You've nothing like that to serve. Cook something first! (cook)"),
            );
            return Ok(());
        };
        let have = self
            .world
            .db
            .satchel(&user)?
            .iter()
            .find(|(i, _)| i == s.id)
            .map_or(0, |(_, n)| *n);
        let guests: Vec<(u64, String, String)> = self
            .players
            .iter()
            .filter(|(pid, p)| **pid != id && p.x == x && p.y == y && !p.bot)
            .map(|(pid, p)| (*pid, p.user.clone(), p.name.clone()))
            .collect();
        if guests.is_empty() {
            self.say1(id, Line::plain("There's nobody here to serve. Eat it yourself, or save it for when friends come by!"));
            return Ok(());
        }
        // Only whoever has room in their satchel can take a portion.
        let (guests, full): (Vec<_>, Vec<_>) = guests
            .into_iter()
            .partition(|(_, u, _)| self.satchel_room(u) > 0);
        for (gid, _, _) in &full {
            self.say1(
                *gid,
                Line::plain(format!(
                    "{who} offers you {}, but your satchel's too full to take it.",
                    nature::a(s.name)
                )),
            );
        }
        let guests: Vec<_> = guests.into_iter().take(have as usize).collect();
        if guests.is_empty() {
            self.say1(
                id,
                Line::plain("Everyone here has a satchel too full to take any. Maybe later!"),
            );
            return Ok(());
        }
        for (gid, guser, _) in &guests {
            self.world.db.satchel_add(&user, s.id, -1)?;
            self.world.db.satchel_add(guser, s.id, 1)?;
            self.say1(
                *gid,
                Line::col(
                    Col::Pink,
                    format!(
                        "🍽 {who} hands you {}. It smells wonderful. (eat it whenever you like)",
                        nature::a(s.name)
                    ),
                ),
            );
        }
        let names: Vec<&str> = guests.iter().map(|(_, _, n)| n.as_str()).collect();
        let who_gets = match names.as_slice() {
            [one] => format!("{one} gets {}", nature::a(s.name)),
            [rest @ .., last] => format!(
                "{} and {last} each get {}",
                rest.join(", "),
                nature::a(s.name)
            ),
            [] => unreachable!("there are guests"),
        };
        self.say1(
            id,
            Line::col(Col::Pink, format!("🍽 You serve up: {who_gets}.")),
        );
        Ok(())
    }

    fn food_in_satchel(
        &self,
        user: &str,
        what: &str,
    ) -> anyhow::Result<Option<&'static nature::Species>> {
        Ok(self
            .world
            .db
            .satchel(user)?
            .iter()
            .filter_map(|(i, _)| nature::by_id(i))
            .find(|s| s.kind == Kind::Food && super::gather::matches(s, what)))
    }

    /// A campfire or oven was unmade: whatever was cooking goes back in the satchel.
    pub(super) fn forget_kitchen(&mut self, thing: i64) {
        if let Some(c) = self.cooking.remove(&thing) {
            for (id, n) in &c.used {
                let _ = self.world.db.satchel_add(&c.user, id, *n);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_dish_is_real_and_cooks_somewhere() {
        for d in DISHES {
            let s = nature::by_id(d.makes).unwrap_or_else(|| panic!("{} isn't a species", d.makes));
            assert_eq!(s.kind, Kind::Food, "{}", d.makes);
            assert!(d.secs > 0 && d.secs <= 60 && d.serves >= 1);
            for (need, _) in d.needs {
                if let Need::Exactly(id) = need {
                    assert!(nature::by_id(id).is_some(), "{id} isn't a species");
                }
            }
        }
        assert_eq!(kitchen("a crackling campfire"), Some(FIRE));
        assert_eq!(kitchen("a little camp stove"), Some(FIRE));
        assert_eq!(kitchen("a clay oven"), Some(OVEN));
        assert_eq!(kitchen("a bench"), None);
    }

    #[test]
    fn cooking_uses_what_you_gathered() {
        let bag = |items: &[(&str, i64)]| {
            items
                .iter()
                .map(|(i, n)| (i.to_string(), *n))
                .collect::<Vec<_>>()
        };
        let tea = DISHES.iter().find(|d| d.makes == "mint_tea").unwrap();
        assert!(plan(&recipe(tea), &bag(&[("wild_mint", 1)])).is_some());
        assert!(plan(&recipe(tea), &bag(&[("daisy", 3)])).is_none());
        let stew = DISHES.iter().find(|d| d.makes == "mushroom_stew").unwrap();
        assert!(
            plan(
                &recipe(stew),
                &bag(&[("chanterelle", 1), ("button", 1), ("thyme", 1)])
            )
            .is_some()
        );
    }
}
