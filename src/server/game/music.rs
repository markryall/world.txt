//! Music boxes you can teach a tune, and humming together (humming along
//! in time is in circle.rs).

use super::Game;
use crate::protocol::{Col, Line};
use std::time::{Duration, Instant};

const MUSIC_KIND: &str = "music";
const JAM_WINDOW: Duration = Duration::from_secs(12);
const MAX_NOTES: usize = 48;
const MIN_TEMPO: u32 = 40;
const MAX_TEMPO: u32 = 240;

pub fn is_music_box(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("music box") || n.contains("musicbox") || n.contains("music-box")
}

pub fn music_kind() -> &'static str {
    MUSIC_KIND
}

/// One word of a tune, tidied: a note (A–G, with # or b and an octave 2–6)
/// or a rest (`_`), then how long it lasts: each `-` holds it a beat longer,
/// each `/` halves it, and a `.` makes it half as long again. Or the tempo,
/// `@90`, in beats a minute. Anything else isn't music.
///
/// The browser reads tunes the same way (`parseTune` in web/index.html).
pub fn clean_word(w: &str) -> Option<String> {
    if let Some(n) = w.strip_prefix('@') {
        let bpm: u32 = n.parse().ok()?;
        return Some(format!("@{}", bpm.clamp(MIN_TEMPO, MAX_TEMPO)));
    }
    let mut c = w.chars().peekable();
    let first = c.next()?;
    let mut out = String::new();
    if first == '_' {
        out.push('_');
    } else if "ABCDEFGabcdefg".contains(first) {
        out.push(first.to_ascii_uppercase());
        if let Some(&a) = c.peek().filter(|a| matches!(a, '#' | 'b')) {
            out.push(a);
            c.next();
        }
        if let Some(&o) = c.peek().filter(|o| ('2'..='6').contains(*o)) {
            out.push(o);
            c.next();
        }
    } else {
        return None;
    }
    let rest: String = c.collect();
    let holds = rest.chars().take_while(|&ch| ch == '-').count();
    let halves = rest[holds..].chars().take_while(|&ch| ch == '/').count();
    let dot = &rest[holds + halves..];
    if !matches!(dot, "" | ".") || holds > 7 || halves > 3 {
        return None;
    }
    out.push_str(&rest);
    Some(out)
}

/// Keep only the playable words of a tune, however long (a learned song;
/// sections are split before this).
pub fn clean_tune_long(raw: &str) -> String {
    let words: Vec<String> = raw.split(|c: char| c.is_whitespace() || c == ',').filter_map(clean_word).collect();
    let tempo = words.iter().rev().find(|w| w.starts_with('@')).cloned();
    let notes = words.into_iter().filter(|w| !w.starts_with('@'));
    tempo.into_iter().chain(notes).collect::<Vec<_>>().join(" ")
}

/// Keep only the playable words of a tune, so it's something the browser can
/// actually play.
pub fn clean_tune(raw: &str) -> String {
    let words: Vec<String> = raw.split(|c: char| c.is_whitespace() || c == ',').filter_map(clean_word).collect();
    // The tempo goes first (the last one given wins), then the notes.
    let tempo = words.iter().rev().find(|w| w.starts_with('@')).cloned();
    let notes = words.into_iter().filter(|w| !w.starts_with('@')).take(MAX_NOTES);
    tempo.into_iter().chain(notes).collect::<Vec<_>>().join(" ")
}

impl Game {
    pub(super) fn has_music_box_here(&self, id: u64) -> bool {
        self.players.get(&id).is_some_and(|p| self.music_box_here(p.x, p.y).is_some())
    }

    fn music_box_here(&self, x: i64, y: i64) -> Option<usize> {
        self.world.things.iter().position(|t| t.x == x && t.y == y && t.kind.as_deref() == Some(MUSIC_KIND))
    }

    pub(super) fn cmd_teach(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let Some(i) = self.music_box_here(x, y) else {
            self.say1(id, Line::plain("There's no music box here to teach. Try 'make a music box'!"));
            return Ok(());
        };
        let user = self.user(id);
        if self.world.things[i].maker != user && !self.is_keeper(&user) {
            let maker = self.world.nick(&self.world.things[i].maker);
            self.say1(id, Line::plain(format!("That's {maker}'s music box, so only {maker} can teach it a tune. You can 'play' it though!")));
            return Ok(());
        }
        let tune = clean_tune(rest);
        if tune.is_empty() {
            self.say1(id, Line::plain("Teach it which notes? e.g. 'teach @100 C C G G A A G-' (A to G, with # or b, 2–6 for higher or lower; - holds a note longer, / makes it shorter, _ is a rest, @100 is the tempo)"));
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
    /// `user` is a username, so whoever's muted either of them doesn't hear
    /// about it.
    pub(super) fn notice_jam(&mut self, x: i64, y: i64, user: &str) {
        let now = Instant::now();
        let earlier = self.last_hum.get(&(x, y)).filter(|(who, at)| who != user && now.duration_since(*at) < JAM_WINDOW).map(|(w, _)| w.clone());
        self.last_hum.insert((x, y), (user.to_string(), now));
        if let Some(other) = earlier {
            let line = Line::new().b(Col::Pink, format!("🎶 {} and {} are making music together!", self.world.nick(&other), self.world.nick(user)));
            // Unless they're humming along already, a nudge towards keeping time.
            let hint = (!self.circles.contains_key(&(x, y))).then(|| Line::col(Col::Dim, "(To keep in time, 'hum along': the world counts you both in.)"));
            for p in self.players.values() {
                if (p.x, p.y) == (x, y) && p.hears(&other) && p.hears(user) {
                    let lines = std::iter::once(line.clone()).chain(hint.clone()).collect();
                    let _ = p.tx.send(crate::protocol::ToClient::Lines { lines });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunes_keep_only_playable_notes() {
        assert_eq!(clean_tune("c e g, E5 C# bb3 hello Z 7"), "C E G E5 C# Bb3");
        assert_eq!(clean_tune("c- e/ g// a. b/. _ _- C#4---"), "C- E/ G// A. B/. _ _- C#4---");
        assert_eq!(clean_tune("C D @90 E @300"), "@240 C D E");
        assert_eq!(clean_tune("@20 C"), "@40 C");
        assert_eq!(clean_tune("C-/- C.. C--------- @fast _x"), "");
        assert_eq!(clean_tune("la la la"), "");
        assert!(is_music_box("a tiny Music Box"));
    }
}
