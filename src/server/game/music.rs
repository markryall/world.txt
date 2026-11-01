//! Music boxes you can teach a tune, and humming together.

use super::Game;
use crate::protocol::{Col, Line};
use std::time::{Duration, Instant};

const MUSIC_KIND: &str = "music";
const JAM_WINDOW: Duration = Duration::from_secs(12);
const MAX_NOTES: usize = 24;

pub fn is_music_box(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("music box") || n.contains("musicbox") || n.contains("music-box")
}

pub fn music_kind() -> &'static str {
    MUSIC_KIND
}

/// Keep only playable notes (A–G, with # or b and an octave 2–6), so a tune
/// is something the browser can actually play.
pub fn clean_tune(raw: &str) -> String {
    raw.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| {
            let mut c = w.chars();
            let Some(letter) = c.next() else { return false };
            let rest: String = c.collect();
            "ABCDEFGabcdefg".contains(letter)
                && matches!(rest.as_str(), "" | "#" | "b" | "2" | "3" | "4" | "5" | "6" | "#2" | "#3" | "#4" | "#5" | "#6" | "b2" | "b3" | "b4" | "b5" | "b6")
        })
        .take(MAX_NOTES)
        .map(|w| {
            let mut c = w.chars();
            let first = c.next().unwrap().to_ascii_uppercase();
            format!("{first}{}", c.as_str())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl Game {
    fn music_box_here(&self, x: i64, y: i64) -> Option<usize> {
        self.world.things.iter().position(|t| t.x == x && t.y == y && t.kind.as_deref() == Some(MUSIC_KIND))
    }

    pub(super) fn cmd_teach(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let Some(i) = self.music_box_here(x, y) else {
            self.say1(id, Line::plain("There's no music box here to teach. Try 'make a music box'!"));
            return Ok(());
        };
        if !self.world.things[i].maker.eq_ignore_ascii_case(&who) && !self.is_keeper(&who) {
            let maker = self.world.things[i].maker.clone();
            self.say1(id, Line::plain(format!("That's {maker}'s music box, so only {maker} can teach it a tune. You can 'play' it though!")));
            return Ok(());
        }
        let tune = clean_tune(rest);
        if tune.is_empty() {
            self.say1(id, Line::plain("Teach it which notes? e.g. 'teach C E G E C' (A to G, with # or b, and 2–6 for higher or lower)"));
            return Ok(());
        }
        let t = &mut self.world.things[i];
        t.action = Some(format!("plays ♪ {tune} ♪"));
        let (tid, action) = (t.id, t.action.clone());
        self.world.db.set_thing_action(tid, action.as_deref())?;
        self.say1(id, Line::new().c(Col::Pink, "🎶 The music box learns your tune: ").b(Col::Pink, format!("♪ {tune} ♪")));
        self.tell_here(x, y, id, Line::col(Col::Pink, format!("🎶 {who} teaches the music box a new tune.")));
        Ok(())
    }

    pub(super) fn cmd_play(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let Some(i) = self.music_box_here(x, y) else {
            self.say1(id, Line::plain("There's nothing here to play. Try 'hum C E G', or make a music box!"));
            return Ok(());
        };
        let Some(action) = self.world.things[i].action.clone() else {
            self.say1(id, Line::plain("It doesn't know a tune yet. Its maker can 'teach' it one."));
            return Ok(());
        };
        let line = Line::col(Col::Pink, format!("🎶 {who} winds up the music box. It {action}."));
        self.say1(id, line.clone());
        self.tell_here(x, y, id, line);
        Ok(())
    }

    /// Called after someone hums: two people humming at the same spot close
    /// together are jamming.
    pub(super) fn notice_jam(&mut self, x: i64, y: i64, name: &str) {
        let now = Instant::now();
        let earlier = self.last_hum.get(&(x, y)).filter(|(who, at)| who != name && now.duration_since(*at) < JAM_WINDOW).map(|(w, _)| w.clone());
        self.last_hum.insert((x, y), (name.to_string(), now));
        if let Some(other) = earlier {
            self.tell_here(x, y, u64::MAX, Line::new().b(Col::Pink, format!("🎶 {other} and {name} are making music together!")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunes_keep_only_playable_notes() {
        assert_eq!(clean_tune("c e g, E5 C# bb3 hello Z 7"), "C E G E5 C# Bb3");
        assert_eq!(clean_tune("la la la"), "");
        assert!(is_music_box("a tiny Music Box"));
    }
}
