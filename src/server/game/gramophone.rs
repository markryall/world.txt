//! Gramophones: real music from someone's own computer, heard by everyone
//! nearby. The playing browser encodes the songs and sends them up; the world
//! only passes them on to people in earshot, louder the closer they are, and
//! never keeps them.

use super::Game;
use crate::protocol::{Col, Line, ToClient};

const KIND: &str = "gramophone";
/// How far a gramophone (or radio) carries, in steps.
pub const EARSHOT: i64 = 5;
/// Sound makers must be further apart than this, so nowhere is in earshot
/// of two: you'd never want to hear two songs at once.
pub const SPACING: i64 = 2 * EARSHOT;
/// At most this many frames (about a second) in one batch.
const MAX_FRAMES: usize = 60;
/// A frame of encoded music, as base64.
const MAX_FRAME_LEN: usize = 4096;
const MAX_TITLE: usize = 80;

pub fn is_gramophone(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("gramophone") || n.contains("record player") || n.contains("phonograph")
}

pub fn kind() -> &'static str {
    KIND
}

/// Music playing on a gramophone, from someone's browser.
pub struct Spin {
    pub host: u64,
    pub title: Option<String>,
    /// A jukebox turn, rather than someone's gramophone.
    pub jukebox: bool,
}

/// How loud a gramophone sounds this many steps away: 1 beside it, fading out.
pub fn gain_at(steps: i64) -> Option<f32> {
    (steps <= EARSHOT).then(|| 1.0 - steps as f32 / (EARSHOT + 1) as f32)
}

/// A song title fit to show: no control characters, not too long.
pub fn clean_title(raw: &str) -> Option<String> {
    let t: String = raw.chars().filter(|c| !c.is_control()).take(MAX_TITLE).collect();
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

/// Gramophones and radios: things that play real sound.
pub fn makes_sound(kind: Option<&str>) -> bool {
    kind == Some(KIND) || kind == Some(super::radio::kind()) || kind == Some(super::jukebox::kind())
}

fn steps(a: (i64, i64), b: (i64, i64)) -> i64 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

impl Game {
    /// Another gramophone or radio too close to (x, y) for one to go there.
    pub(super) fn sound_too_close(&self, x: i64, y: i64, except: Option<i64>) -> Option<String> {
        self.world
            .things
            .iter()
            .find(|t| makes_sound(t.kind.as_deref()) && Some(t.id) != except && steps((t.x, t.y), (x, y)) <= SPACING)
            .map(|t| t.name.clone())
    }

    /// The one sound someone at (x, y) hears: the nearest gramophone or radio
    /// that's playing, if any is in earshot. Never two at once.
    pub(super) fn sound_heard_at(&self, x: i64, y: i64) -> Option<i64> {
        self.world
            .things
            .iter()
            .filter(|t| self.spins.contains_key(&t.id) || self.radios.contains_key(&t.id))
            .map(|t| (steps((t.x, t.y), (x, y)), t.id))
            .filter(|(d, _)| *d <= EARSHOT)
            .min()
            .map(|(_, id)| id)
    }

    fn gramophone_here(&self, x: i64, y: i64) -> Option<usize> {
        self.world.things.iter().position(|t| t.x == x && t.y == y && t.kind.as_deref() == Some(KIND))
    }

    pub(super) fn has_gramophone_here(&self, id: u64) -> bool {
        self.players.get(&id).is_some_and(|p| self.gramophone_here(p.x, p.y).is_some())
    }

    /// Tell everyone within earshot of a gramophone.
    pub(super) fn tell_earshot(&self, thing: i64, except: u64, line: Line) {
        let Some(t) = self.world.things.iter().find(|t| t.id == thing) else { return };
        for (pid, p) in &self.players {
            if *pid != except && (p.x - t.x).abs().max((p.y - t.y).abs()) <= EARSHOT {
                let _ = p.tx.send(ToClient::Lines { lines: vec![line.clone()] });
            }
        }
    }

    /// `play music`: start playing songs from this browser on the gramophone here.
    pub(super) fn cmd_play_music(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let Some(i) = self.gramophone_here(x, y) else {
            self.say1(id, Line::plain("There's no gramophone here. Try 'make a gramophone'!"));
            return Ok(());
        };
        if self.players[&id].bot {
            self.say1(id, Line::plain("Gramophones play songs from someone's own computer, in the browser."));
            return Ok(());
        }
        let thing = self.world.things[i].id;
        if let Some(s) = self.spins.get(&thing) {
            let msg = if s.host == id {
                "You're already playing music here. 'stop music' to stop.".to_string()
            } else {
                let host = self.players.get(&s.host).map_or("Someone".to_string(), |p| p.name.clone());
                format!("{host} is already playing music on this gramophone. Listen in!")
            };
            self.say1(id, Line::plain(msg));
            return Ok(());
        }
        if self.spins.values().any(|s| s.host == id) {
            self.say1(id, Line::plain("You're playing music on another gramophone. 'stop music' there first."));
            return Ok(());
        }
        self.spins.insert(thing, Spin { host: id, title: None, jukebox: false });
        if let Some(p) = self.players.get(&id) {
            let _ = p.tx.send(ToClient::SpinStart { thing });
        }
        self.say1(id, Line::col(Col::Pink, "🎶 You wind up the gramophone. Choose some songs from your computer, and everyone nearby will hear them."));
        self.tell_here(x, y, id, Line::col(Col::Pink, format!("🎶 {who} winds up the gramophone.")));
        Ok(())
    }

    /// `stop music`: stop what you're playing (keepers can stop anyone's, here).
    pub(super) fn cmd_stop_music(&mut self, id: u64) -> anyhow::Result<()> {
        if let Some((thing, juke)) = self.spins.iter().find(|(_, s)| s.host == id).map(|(t, s)| (*t, s.jukebox)) {
            if juke {
                return self.cmd_skip_song(id);
            }
            self.stop_spin(thing);
            return Ok(());
        }
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        let here = self.gramophone_here(x, y).map(|i| self.world.things[i].id);
        match here.filter(|t| self.spins.contains_key(t)) {
            Some(thing) if self.is_keeper(&self.user(id)) => self.stop_spin(thing),
            Some(thing) => {
                let host = self.players.get(&self.spins[&thing].host).map_or("whoever's playing".into(), |p| p.name.clone());
                self.say1(id, Line::plain(format!("It's {host}'s music, so only {host} can stop it. You can always wander off!")));
            }
            None => self.say1(id, Line::plain("You're not playing any music.")),
        }
        Ok(())
    }

    /// The music stops: the player's browser, the listeners and the room are told.
    pub(super) fn stop_spin(&mut self, thing: i64) {
        let Some(spin) = self.spins.remove(&thing) else { return };
        if let Some(p) = self.players.get(&spin.host) {
            let _ = p.tx.send(ToClient::SpinStop { thing });
        }
        for p in self.players.values() {
            let _ = p.tx.send(ToClient::AudioEnd { thing });
        }
        self.tell_earshot(thing, u64::MAX, Line::col(Col::Pink, "🎶 The gramophone slows, crackles and falls quiet."));
    }

    /// Stop anything this player was playing (they left, or unmade it).
    pub(super) fn stop_spins_by(&mut self, id: u64) {
        let theirs: Vec<i64> = self.spins.iter().filter(|(_, s)| s.host == id).map(|(t, _)| *t).collect();
        for t in theirs {
            self.stop_spin(t);
        }
    }

    /// A batch of encoded music from a playing browser: pass it on to everyone
    /// in earshot, each with how loud it is where they stand.
    pub(super) fn audio(&mut self, id: u64, frames: Vec<String>, title: Option<String>) {
        let Some(thing) = self.spins.iter().find(|(_, s)| s.host == id).map(|(t, _)| *t) else { return };
        if frames.is_empty() || frames.len() > MAX_FRAMES || frames.iter().any(|f| f.len() > MAX_FRAME_LEN) {
            return;
        }
        let Some((tx, ty)) = self.world.things.iter().find(|t| t.id == thing).map(|t| (t.x, t.y)) else {
            self.stop_spin(thing);
            return;
        };
        let title = title.as_deref().and_then(clean_title);
        if self.spins[&thing].jukebox {
            // The jukebox says what's on itself, when each turn starts.
            self.juke_heard(thing);
        } else if title.is_some() && title != self.spins[&thing].title {
            if let Some(s) = self.spins.get_mut(&thing) {
                s.title = title.clone();
            }
            let line = Line::new().c(Col::Pink, "🎶 Now playing on the gramophone: ").b(Col::Pink, title.clone().unwrap_or_default());
            self.tell_earshot(thing, u64::MAX, line);
        }
        let title = self.spins[&thing].title.clone();
        let host = self.user(id);
        for (pid, p) in &self.players {
            // Muting someone silences their music for you too.
            if *pid == id || p.bot || !p.hears(&host) {
                continue;
            }
            if self.sound_heard_at(p.x, p.y) != Some(thing) {
                continue;
            }
            if let Some(gain) = gain_at(steps((p.x, p.y), (tx, ty))) {
                let _ = p.tx.send(ToClient::Audio { thing, frames: frames.clone(), gain, title: title.clone() });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gramophones_fade_with_distance_and_titles_are_tidied() {
        assert!(is_gramophone("a wind-up gramophone") && is_gramophone("an old Record Player"));
        assert!(!is_gramophone("a music box"));
        assert_eq!(gain_at(0), Some(1.0));
        assert!(gain_at(3).unwrap() < gain_at(1).unwrap());
        assert!(gain_at(EARSHOT).unwrap() > 0.0);
        assert_eq!(gain_at(EARSHOT + 1), None);
        assert_eq!(clean_title("  Moon\\nRiver \u{7}  ").as_deref(), Some("Moon\\nRiver"));
        assert_eq!(clean_title("\u{1}\u{2}"), None);
        assert_eq!(clean_title(&"x".repeat(200)).unwrap().len(), MAX_TITLE);
    }
}
