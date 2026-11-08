//! Little things for two (or more): each works alone, and is nicer with
//! company. A picnic, an umbrella in the rain, stargazing, a high five. Not in
//! the help; the scenery hints when there's someone to share them with.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::nature::{self, Kind};
use crate::server::weather::{Clock, Phase};

/// What a constellation looks like tonight.
const CONSTELLATIONS: &[&str] = &[
    "a teapot, pouring",
    "a sleepy cat",
    "a kite with a very long tail",
    "a rabbit holding a carrot",
    "a sailing boat",
    "a snail in a hurry",
    "a heron on one leg",
    "a slice of cake",
    "an owl wearing a hat",
    "a lighthouse, beaming",
    "a dancing frog",
    "a little house with smoke curling from the chimney",
];

impl Game {
    /// The little-things-for-two commands, if this is one.
    pub(super) fn together_command(
        &mut self,
        id: u64,
        verb: &str,
        rest: &str,
    ) -> Option<anyhow::Result<()>> {
        let r = rest.trim().to_lowercase();
        Some(match verb {
            "picnic" => self.cmd_picnic(id),
            "have" | "lay" | "spread" if r.contains("picnic") || r.contains("blanket") => {
                self.cmd_picnic(id)
            }
            "umbrella" | "brolly" => self.cmd_umbrella(id),
            "open" | "share" | "put" if r.contains("umbrella") || r.contains("brolly") => {
                self.cmd_umbrella(id)
            }
            "stargaze" | "stargazing" => self.cmd_stargaze(id),
            "look" | "gaze"
                if r.starts_with("at the stars")
                    || r.starts_with("up at the stars")
                    || r == "up" && self.is_night() =>
            {
                self.cmd_stargaze(id)
            }
            "high" if r.starts_with("five") => {
                self.cmd_high_five(id, r.trim_start_matches("five").trim())
            }
            "highfive" | "high-five" | "hi5" => self.cmd_high_five(id, &r),
            _ => return None,
        })
    }

    fn is_night(&self) -> bool {
        matches!(Clock::phase(), Phase::Night)
    }

    /// Everyone else here who isn't a resident: the company.
    fn company(&self, id: u64) -> Vec<String> {
        let Some(me) = self.players.get(&id) else {
            return vec![];
        };
        self.players
            .iter()
            .filter(|(pid, p)| **pid != id && (p.x, p.y) == (me.x, me.y) && !p.bot)
            .map(|(_, p)| p.name.clone())
            .collect()
    }

    fn cmd_picnic(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else {
            return Ok(());
        };
        if self.world.terrain(x, y).is_wet() {
            self.say1(
                id,
                Line::plain("A picnic in the water? Find somewhere dry and grassy."),
            );
            return Ok(());
        }
        if self.world.weather.sky_at(x, y).wet() {
            self.say1(
                id,
                Line::plain("The rain would get in the sandwiches. Maybe when it clears?"),
            );
            return Ok(());
        }
        let company = self.company(id);
        let user = self.user(id);
        // Something from the satchel to share, if there's food in it.
        let food = self
            .world
            .db
            .satchel(&user)?
            .into_iter()
            .filter_map(|(sid, _)| nature::by_id(&sid))
            .find(|s| s.kind == Kind::Food);
        let (treat, shared) = match food {
            Some(f) => {
                self.world.db.satchel_add(&user, f.id, -1)?;
                let a = nature::a(f.name);
                if company.is_empty() {
                    (
                        format!(" You unpack {a} and enjoy every bite."),
                        String::new(),
                    )
                } else {
                    (
                        format!(" You unpack {a} and share it round."),
                        format!(" {who} unpacks {a} and shares it round."),
                    )
                }
            }
            None => (
                " There's no food, but there's the sky, and the grass, and that's plenty."
                    .to_string(),
                String::new(),
            ),
        };
        let mine = if company.is_empty() {
            format!(
                "🧺 You spread a checked blanket and settle down for a picnic on your own. Peaceful.{treat}"
            )
        } else {
            format!(
                "🧺 You spread a checked blanket, and you and {} settle down for a picnic.{treat}",
                super::cases::and_list(&company)
            )
        };
        self.say1(id, Line::col(Col::Green, mine));
        let theirs = if company.is_empty() {
            format!("🧺 {who} spreads a checked blanket and has a little picnic.")
        } else {
            format!(
                "🧺 {who} spreads a checked blanket, and everyone here settles down for a picnic.{shared} A breeze, a buzzing bee, good company."
            )
        };
        self.tell_here(x, y, id, Line::col(Col::Green, theirs));
        Ok(())
    }

    fn cmd_umbrella(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else {
            return Ok(());
        };
        if !self.world.weather.sky_at(x, y).wet() {
            self.say1(
                id,
                Line::plain(
                    "You'd look a bit odd with an umbrella up on a dry day. Save it for the rain.",
                ),
            );
            return Ok(());
        }
        let company = self.company(id);
        if company.is_empty() {
            self.say1(id, Line::col(Col::Blue, "☂️ You pop open an umbrella and listen to the rain drum on it. There's room under here for one more."));
            self.tell_here(
                x,
                y,
                id,
                Line::col(Col::Blue, format!("☂️ {who} pops open an umbrella.")),
            );
            return Ok(());
        }
        let all = super::cases::and_list(&company);
        self.say1(id, Line::col(Col::Blue, format!("☂️ You pop open an umbrella, and you and {all} huddle underneath, listening to the rain drum on it. Cosy.")));
        self.tell_here(x, y, id, Line::col(Col::Blue, format!("☂️ {who} pops open an umbrella and holds it over everyone. You huddle underneath, listening to the rain drum on it.")));
        Ok(())
    }

    fn cmd_stargaze(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else {
            return Ok(());
        };
        if !self.is_night() {
            self.say1(
                id,
                Line::plain("The stars are hiding until it's properly dark. Come back tonight."),
            );
            return Ok(());
        }
        if self.world.weather.sky_at(x, y) != crate::server::weather::Sky::Clear {
            self.say1(
                id,
                Line::plain(
                    "Clouds have the stars covered tonight. Maybe they'll drift off in a while.",
                ),
            );
            return Ok(());
        }
        let shape = *self.rng.pick(CONSTELLATIONS);
        let company = self.company(id);
        if company.is_empty() {
            self.say1(id, Line::col(Col::DeepBlue, format!("✨ You lie back and gaze up at the stars. After a while, you spot a constellation that looks just like {shape}.")));
            self.tell_here(
                x,
                y,
                id,
                Line::col(
                    Col::DeepBlue,
                    format!("✨ {who} lies back and gazes up at the stars."),
                ),
            );
        } else {
            let all = super::cases::and_list(&company);
            self.say1(id, Line::col(Col::DeepBlue, format!("✨ You and {all} lie back and gaze up at the stars. You point out a constellation that looks just like {shape}.")));
            self.tell_here(x, y, id, Line::col(Col::DeepBlue, format!("✨ {who} lies back beside you, gazing up at the stars, and points out a constellation that looks just like {shape}. Now you can't unsee it.")));
        }
        Ok(())
    }

    fn cmd_high_five(&mut self, id: u64, whom: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else {
            return Ok(());
        };
        let company = self.company(id);
        let other = if whom.is_empty() {
            company.first().cloned()
        } else {
            company
                .iter()
                .find(|n| n.to_lowercase().starts_with(whom))
                .cloned()
        };
        let Some(other) = other else {
            self.say1(id, Line::plain("✋ You hold up a hand for a high five, but there's nobody here to slap it. You high-five the breeze."));
            return Ok(());
        };
        self.say1(
            id,
            Line::col(Col::Yellow, format!("✋ You and {other} high-five. Smack!")),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Yellow,
                format!("✋ {who} and {other} high-five. Smack!"),
            ),
        );
        Ok(())
    }
}
