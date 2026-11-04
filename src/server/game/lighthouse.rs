//! The lighthouse: one long story the whole island works on together. An old
//! ruin on Driftwood Isle's outermost point is mended a stage at a time from
//! things people gather and 'mend' it with. Progress is only ever told in
//! words, everyone who helped is named, nobody is ranked, and nothing is
//! ever lost. Once it's lit, its beam sweeps the water at night.

use super::Game;
use crate::protocol::{Col, Line, ToClient};
use crate::server::terrain::Terrain;
use crate::server::weather::{Clock, Phase};
use crate::server::world::join_and;

const PROJECT: &str = "lighthouse";
pub const KIND: &str = "lighthouse";
const STAGE_KEY: &str = "lighthouse_stage";

struct Stage {
    /// What it takes (species ids), and how many in all.
    wants: &'static [&'static str],
    need: i64,
    /// How it looks while this stage is under way.
    looks: &'static str,
    /// What it's missing, in words.
    asks: &'static str,
    /// Where to look for it.
    hint: &'static str,
    /// Where the gifts so far are, for "…about half what's needed".
    pile: &'static str,
    /// Said to everyone when it's done: "… thanks to Poppy and Sam!"
    done: &'static str,
    /// A thank-you afterwards: "… brought by Poppy and Sam."
    credit: &'static str,
}

const STAGES: [Stage; 4] = [
    Stage {
        wants: &["driftwood"],
        need: 12,
        looks: "An old lighthouse stands on the point, its white paint flaking, its doorway an empty hole the wind whistles through.",
        asks: "A good armful of driftwood would make it a door.",
        hint: "Driftwood washes up on Driftwood Isle's beaches.",
        pile: "Driftwood is stacked by the doorway",
        done: "The old lighthouse has a door again, made from driftwood",
        credit: "The door was made from driftwood brought by",
    },
    Stage {
        wants: &["pebble"],
        need: 20,
        looks: "An old lighthouse stands on the point, with a sturdy driftwood door. Inside, its spiral stairs are broken and gappy.",
        asks: "Smooth pebbles would mend the stairs, a step at a time.",
        hint: "Smooth pebbles turn up on stony ground, paths and beaches.",
        pile: "Pebbles are heaped at the foot of the stairs",
        done: "The lighthouse stairs are mended, all the way up, with pebbles",
        credit: "The stairs were mended with pebbles brought by",
    },
    Stage {
        wants: &["quartz"],
        need: 6,
        looks: "An old lighthouse stands on the point. Its stairs climb all the way up to the lamp room, but the lamp room is empty and dusty: there's no lens.",
        asks: "Quartz crystals could be ground into a lens.",
        hint: "Quartz crystals sparkle high up on hills and rocks.",
        pile: "Quartz crystals sit in a little row in the lamp room",
        done: "The lighthouse has a lens of quartz crystal",
        credit: "The lens was ground from quartz brought by",
    },
    Stage {
        wants: &["fire_opal", "glowcap", "glow_worm"],
        need: 4,
        looks: "An old lighthouse stands on the point, mended from door to lamp room, where a crystal lens waits in the dark.",
        asks: "Something that glows (a fire opal, a glowcap, a glow-worm) might coax it alight.",
        hint: "Fire opals hide on Ember Rock, glowcaps come out in the woods at night, and glow-worms live on Mossy Holm.",
        pile: "Little glowing things are gathered around the lens",
        done: "The lighthouse is lit",
        credit: "The light was kindled with glowing things brought by",
    },
];

/// How far along a stage is, in words.
fn progress(given: i64, need: i64) -> Option<&'static str> {
    let f = given as f64 / need as f64;
    match f {
        f if f <= 0.0 => None,
        f if f < 0.34 => Some("a start has been made"),
        f if f < 0.67 => Some("about half what's needed"),
        _ => Some("nearly enough"),
    }
}

impl Game {
    fn lighthouse_stage(&self) -> usize {
        self.world.db.meta(STAGE_KEY).and_then(|s| s.parse().ok()).unwrap_or(0)
    }

    pub(super) fn lighthouse_lit(&self) -> bool {
        self.lighthouse_stage() >= STAGES.len()
    }

    fn lighthouse_here(&self, x: i64, y: i64) -> bool {
        self.world.things_at(x, y).any(|t| t.kind.as_deref() == Some(KIND))
    }

    fn helpers(&self, stage: usize) -> Vec<String> {
        let gifts = self.world.db.project_gifts(PROJECT, stage as i64).unwrap_or_default();
        gifts.into_iter().map(|(who, _)| self.world.nick(&who)).collect()
    }

    /// For `look`: the lighthouse as it is now, and what it's missing.
    pub(super) fn lighthouse_lines_here(&self, id: u64) -> Vec<Line> {
        let Some(p) = self.players.get(&id) else { return vec![] };
        if !self.lighthouse_here(p.x, p.y) {
            return vec![];
        }
        let stage = self.lighthouse_stage();
        let mut lines = Vec::new();
        match STAGES.get(stage) {
            Some(s) => {
                lines.push(Line::col(Col::Yellow, format!("🏮 {}", s.looks)));
                let given: i64 = self.world.db.project_gifts(PROJECT, stage as i64).unwrap_or_default().iter().map(|g| g.1).sum();
                let ask = match progress(given, s.need) {
                    Some(how) => format!("   {} ({how}). {}", s.pile, s.asks),
                    None => format!("   {}", s.asks),
                };
                lines.push(Line::plain(ask));
                lines.push(Line::col(Col::Dim, "   Anyone can help: mend lighthouse"));
            }
            None => {
                let lamp = match Clock::phase() {
                    Phase::Night | Phase::Dusk => "Up top, its lamp turns slowly, sweeping a beam of light out across the dark water.",
                    Phase::Dawn => "Up top, its lamp is fading as the sky lightens.",
                    Phase::Day => "Up top, its lens catches the sun and throws little rainbows on the rocks.",
                };
                lines.push(Line::col(Col::Yellow, format!("🏮 The lighthouse stands proud on the point, white and mended. {lamp}")));
            }
        }
        // Everyone who's helped, stage by stage: a thank-you, not a ranking.
        for (i, s) in STAGES.iter().enumerate().take(stage) {
            let who = self.helpers(i);
            if !who.is_empty() {
                lines.push(Line::col(Col::Dim, format!("   ✧ {} {}.", s.credit, join_and(&who))));
            }
        }
        lines
    }

    /// `mend lighthouse`: give it what it needs next from your satchel.
    pub(super) fn cmd_mend(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        if !self.lighthouse_here(x, y) {
            let msg = if self.lighthouse_lit() {
                "There's nothing here to mend. The lighthouse on Driftwood Isle is already lit. ✨"
            } else {
                "There's nothing here to mend. An old lighthouse on Driftwood Isle could do with some help, though: sail there and find Lighthouse Point."
            };
            self.say1(id, Line::plain(msg));
            return Ok(());
        }
        let stage = self.lighthouse_stage();
        let Some(s) = STAGES.get(stage) else {
            self.say1(id, Line::col(Col::Yellow, "🏮 The lighthouse is whole and lit. There's nothing left to mend: just enjoy it. ✨"));
            return Ok(());
        };
        let user = self.user(id);
        let given: i64 = self.world.db.project_gifts(PROJECT, stage as i64)?.iter().map(|g| g.1).sum();
        let mut still = (s.need - given).max(0);
        let satchel = self.world.db.satchel(&user)?;
        let mut gave: Vec<String> = Vec::new();
        let mut total = 0;
        for want in s.wants {
            let have = satchel.iter().find(|(sp, _)| sp == want).map_or(0, |(_, n)| *n);
            let n = have.min(still);
            if n == 0 {
                continue;
            }
            self.world.db.satchel_add(&user, want, -n)?;
            still -= n;
            total += n;
            let sp = crate::server::nature::by_id(want).map_or(*want, |sp| sp.name);
            gave.push(if n == 1 { crate::server::nature::a(sp) } else { format!("some {}", plural(sp)) });
        }
        if total == 0 {
            self.send(id, vec![Line::plain(format!("🏮 {}", s.asks)), Line::col(Col::Dim, format!("   You haven't any with you. {}", s.hint))]);
            return Ok(());
        }
        self.world.db.add_project_gift(PROJECT, stage as i64, &user, total, super::boards::now())?;
        self.say1(id, Line::col(Col::Yellow, format!("🏮 You give the lighthouse {}. Thank you!", join_and(&gave))));
        self.tell_here_from(x, y, id, &user, Line::col(Col::Yellow, format!("🏮 {name} helps mend the lighthouse with {}.", join_and(&gave))));
        if still > 0 {
            if let Some(how) = progress(s.need - still, s.need) {
                self.say1(id, Line::col(Col::Dim, format!("   {} now: {how}.", s.pile)));
            }
            return Ok(());
        }
        // That's this stage done: tell everyone, and write it in the journal.
        self.world.db.set_meta(STAGE_KEY, &(stage + 1).to_string())?;
        let thanks = format!("{}, thanks to {}!", s.done, join_and(&self.helpers(stage)));
        let line = Line::new().b(Col::Yellow, format!("🏮 {thanks}"));
        for p in self.players.values() {
            let _ = p.tx.send(ToClient::Lines { lines: vec![line.clone()] });
        }
        if let Some(board) = self.journal_board {
            let now = super::boards::now();
            self.world.db.add_note(board, "the island", &format!("🏮 {thanks}"), now, &super::boards::week_key(now), None)?;
        }
        if stage + 1 >= STAGES.len() {
            self.say1(id, Line::col(Col::Yellow, "   Look out for its beam across the water at night. ✨"));
        } else {
            self.say1(id, Line::col(Col::Dim, format!("   Next: {}", STAGES[stage + 1].asks)));
        }
        self.status_all();
        Ok(())
    }

    /// Now and then at night, people by the sea see the beam sweep round.
    pub(super) fn lighthouse_tick(&mut self) {
        if Clock::phase() != Phase::Night || !self.lighthouse_lit() {
            return;
        }
        let ids: Vec<u64> = self.players.iter().filter(|(_, p)| !p.bot && p.ambient_quiet < super::AMBIENT_WHILE_IDLE).map(|(&id, _)| id).collect();
        for id in ids {
            let Some(p) = self.players.get(&id) else { continue };
            let (x, y) = (p.x, p.y);
            let by_sea = p.voyage.is_some() || matches!(self.world.terrain(x, y), Terrain::Sand | Terrain::Water | Terrain::DeepWater);
            if !by_sea || self.lighthouse_here(x, y) || !self.rng.chance(1.0 / 60.0) {
                continue;
            }
            let line = if p.voyage.is_some() {
                "✨ The lighthouse beam sweeps over your little boat, and away across the water."
            } else {
                "✨ Far off across the water, the lighthouse beam sweeps round, and is gone, and comes round again."
            };
            self.say1(id, Line::col(Col::Yellow, line));
            if let Some(p) = self.players.get_mut(&id) {
                p.ambient_quiet += 1;
            }
        }
    }
}

/// "piece of driftwood" → "pieces of driftwood", "glowcap" → "glowcaps"
fn plural(name: &str) -> String {
    match name.split_once(" of ") {
        Some((one, rest)) => format!("{one}s of {rest}"),
        None if name.ends_with('y') && !name.ends_with("ey") => format!("{}ies", &name[..name.len() - 1]),
        None => format!("{name}s"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_words_not_numbers() {
        assert_eq!(progress(0, 12), None);
        assert_eq!(progress(2, 12), Some("a start has been made"));
        assert_eq!(progress(6, 12), Some("about half what's needed"));
        assert_eq!(progress(11, 12), Some("nearly enough"));
        for s in &STAGES {
            for text in [s.looks, s.asks, s.hint, s.pile, s.done, s.credit] {
                assert!(!text.chars().any(|c| c.is_ascii_digit()), "no numbers: {text}");
            }
            for w in s.wants {
                assert!(crate::server::nature::by_id(w).is_some(), "{w} is something you can find");
            }
        }
    }

    #[test]
    fn plurals() {
        assert_eq!(plural("piece of driftwood"), "pieces of driftwood");
        assert_eq!(plural("smooth pebble"), "smooth pebbles");
        assert_eq!(plural("quartz crystal"), "quartz crystals");
        assert_eq!(plural("glow-worm"), "glow-worms");
    }
}
