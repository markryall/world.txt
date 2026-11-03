//! Radios: tune one to an internet radio stream and everyone nearby hears it,
//! louder the closer they are. The world never touches the sound itself: it
//! tells each nearby browser which stream to play and how loud, and the
//! browser plays it straight from the station.

use super::Game;
use super::gramophone::gain_at;
use crate::protocol::{Col, Line, ToClient};
use std::collections::HashMap;

const KIND: &str = "radio";
const MAX_URL: usize = 400;

pub fn is_radio(name: &str) -> bool {
    name.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|w| w == "radio" || w == "wireless")
}

pub fn kind() -> &'static str {
    KIND
}

fn meta_key(thing: i64) -> String {
    format!("radio:{thing}")
}

/// A stream address the browser can play from a secure page: https only.
pub fn clean_stream(raw: &str) -> Result<String, &'static str> {
    let url = raw.trim();
    if url.starts_with("http://") {
        return Err("That's an http:// stream. Browsers only play https:// streams on a secure page; many stations have one.");
    }
    if !url.starts_with("https://") || url.len() <= "https://".len() || url.len() > MAX_URL || url.contains(char::is_whitespace) {
        return Err("Tune it to a stream's https:// address, e.g. 'tune radio https://stream.example.com/live.mp3'");
    }
    Ok(url.to_string())
}

/// Just the station's address, for saying what's on.
pub fn station(url: &str) -> &str {
    url.trim_start_matches("https://").split(['/', '?', ':']).next().unwrap_or(url)
}

impl Game {
    /// Every radio's stream, from the db (at start).
    pub(super) fn load_radios(&mut self) {
        self.radios = self
            .world
            .things
            .iter()
            .filter(|t| t.kind.as_deref() == Some(KIND))
            .filter_map(|t| self.world.db.meta(&meta_key(t.id)).filter(|u| !u.is_empty()).map(|u| (t.id, u)))
            .collect();
    }

    fn radio_here(&self, x: i64, y: i64) -> Option<usize> {
        self.world.things.iter().position(|t| t.x == x && t.y == y && t.kind.as_deref() == Some(KIND))
    }

    pub(super) fn has_radio_here(&self, id: u64) -> bool {
        self.players.get(&id).is_some_and(|p| self.radio_here(p.x, p.y).is_some())
    }

    /// `tune radio https://…` (its maker, or a keeper).
    pub(super) fn cmd_tune(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let Some(i) = self.radio_here(x, y) else {
            self.say1(id, Line::plain("There's no radio here to tune. Try 'make a radio'!"));
            return Ok(());
        };
        let user = self.user(id);
        if self.world.things[i].maker != user && !self.is_keeper(&user) {
            let maker = self.world.nick(&self.world.things[i].maker);
            self.say1(id, Line::plain(format!("That's {maker}'s radio, so only {maker} can tune it. Have a listen though!")));
            return Ok(());
        }
        let raw = rest.trim().strip_prefix("radio").unwrap_or(rest.trim()).trim().strip_prefix("to").unwrap_or_default().trim();
        let raw = if raw.is_empty() { rest.trim().trim_start_matches("radio").trim() } else { raw };
        let url = match clean_stream(raw) {
            Ok(u) => u,
            Err(why) => {
                self.say1(id, Line::plain(why));
                return Ok(());
            }
        };
        let thing = self.world.things[i].id;
        self.world.db.set_meta(&meta_key(thing), &url)?;
        let on = station(&url).to_string();
        self.radios.insert(thing, url);
        self.say1(id, Line::new().c(Col::Pink, "📻 You tune the radio to ").b(Col::Pink, on.clone()).c(Col::Pink, ". Everyone nearby with sound on can hear it."));
        self.tell_here(x, y, id, Line::col(Col::Pink, format!("📻 {who} tunes the radio to {on}.")));
        Ok(())
    }

    /// `radio off` (its maker, or a keeper).
    pub(super) fn cmd_radio_off(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let Some(i) = self.radio_here(x, y) else {
            self.say1(id, Line::plain("There's no radio here."));
            return Ok(());
        };
        let user = self.user(id);
        if self.world.things[i].maker != user && !self.is_keeper(&user) {
            self.say1(id, Line::plain("Only its maker (or a keeper) can switch it off. You can always wander off!"));
            return Ok(());
        }
        let thing = self.world.things[i].id;
        self.world.db.set_meta(&meta_key(thing), "")?;
        self.radios.remove(&thing);
        self.say1(id, Line::col(Col::Dim, "📻 Click. The radio goes quiet."));
        self.tell_here(x, y, id, Line::col(Col::Dim, format!("📻 {who} switches the radio off.")));
        Ok(())
    }

    /// A radio was unmade.
    pub(super) fn forget_radio(&mut self, thing: i64) {
        if self.radios.remove(&thing).is_some() {
            let _ = self.world.db.set_meta(&meta_key(thing), "");
        }
    }

    /// Tell each browser which radios it can hear and how loud, when that
    /// changes (they moved, or a radio was tuned, moved or switched off).
    pub(super) fn update_radios(&mut self) {
        // Each person hears only the nearest sound, so a radio is only theirs
        // if it's nearer than any other radio or gramophone.
        let heard: HashMap<u64, Option<i64>> =
            self.players.iter().map(|(pid, p)| (*pid, self.sound_heard_at(p.x, p.y))).collect();
        let radios: HashMap<i64, (i64, i64, String)> = self
            .radios
            .iter()
            .filter_map(|(id, url)| self.world.things.iter().find(|t| t.id == *id).map(|t| (*id, (t.x, t.y, url.clone()))))
            .collect();
        for (pid, p) in self.players.iter_mut().filter(|(_, p)| !p.bot) {
            let mut hearing: HashMap<i64, (f32, String)> = HashMap::new();
            if let Some((id, (x, y, url))) = heard[pid].and_then(|id| radios.get(&id).map(|r| (id, r))) {
                if let Some(gain) = gain_at((p.x - x).abs().max((p.y - y).abs())) {
                    hearing.insert(id, (gain, url.clone()));
                }
            }
            for (id, (gain, url)) in &hearing {
                if p.radios.get(id) != Some(&(*gain, url.clone())) {
                    if p.radios.get(id).is_none_or(|(_, u)| u != url) {
                        let line = Line::col(Col::Pink, format!("📻 A radio nearby is playing {}.", station(url)));
                        let _ = p.tx.send(ToClient::Lines { lines: vec![line] });
                    }
                    let _ = p.tx.send(ToClient::Radio { thing: *id, url: Some(url.clone()), gain: *gain });
                }
            }
            for id in p.radios.keys().filter(|id| !hearing.contains_key(id)) {
                let _ = p.tx.send(ToClient::Radio { thing: *id, url: None, gain: 0.0 });
            }
            p.radios = hearing;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radios_take_secure_streams_and_name_the_station() {
        assert!(is_radio("an old radio") && is_radio("a crackly wireless"));
        assert!(!is_radio("a radiator"));
        assert_eq!(clean_stream(" https://stream.example.com/live.mp3 ").unwrap(), "https://stream.example.com/live.mp3");
        assert!(clean_stream("http://stream.example.com/live").unwrap_err().contains("https"));
        assert!(clean_stream("not a url").is_err());
        assert!(clean_stream("https://").is_err());
        assert_eq!(station("https://stream.example.com:8000/live.mp3?x=1"), "stream.example.com");
    }
}
