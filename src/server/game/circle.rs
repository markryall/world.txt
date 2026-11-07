//! Humming along: a circle of people at one spot, each humming a part, kept
//! in time by the world.
//!
//! The first `hum along` at a spot starts a circle, and its first section
//! (up to the first `|`, or the whole tune) says how long a turn is, in
//! beats, and how fast. Everyone's part is a loop of numbered sections, one
//! a turn: `hum along` adds sections, `hum along 3 …` puts new notes in
//! section 3 (for fixing a wrong note, or changing your mind), `hush 3`
//! takes section 3 out, and `hush` stops. At the start of every turn the
//! world sends each person at the spot all the parts together, in one
//! message, so they arrive (and play) together however far away anyone's
//! browser is.
//!
//! A ground bass is one section going round; a round (Frère Jacques) is the
//! same few sections for everyone, each person starting a turn after the last.

use super::Game;
use super::music::clean_word;
use super::songbook::{self, Found, SONGS, Song};
use crate::protocol::{CirclePart, Col, Line, ToClient};
use std::collections::HashMap;
use std::ops::RangeInclusive;
use std::time::{Duration, Instant};

/// How many sections one person's part can have.
pub const MAX_SECTIONS: usize = 16;
/// How many people can hum in one circle.
pub const MAX_PARTS: usize = 8;
/// The most notes a browser plays of one tune (`playTune` in web/index.html).
pub const MAX_NOTES: usize = 64;
/// The longest a turn can be, in beats.
pub const MAX_BEATS: f64 = 64.0;
const EPS: f64 = 1e-9;

/// How many beats a tidied word of a tune lasts.
fn beats(word: &str) -> f64 {
    let holds = word.matches('-').count();
    let halves = word.matches('/').count();
    let dot = if word.ends_with('.') { 1.5 } else { 1.0 };
    (1 + holds) as f64 / 2f64.powi(halves as i32) * dot
}

/// The tempo a tune asks for (the last one given), if any.
fn tempo(raw: &str) -> Option<u32> {
    raw.split(|c: char| c.is_whitespace() || c == ',' || c == '|')
        .filter_map(clean_word)
        .filter_map(|w| w.strip_prefix('@').and_then(|n| n.parse().ok()))
        .next_back()
}

/// The notes and rests of a bit of tune, tidied (no tempo).
fn notes(raw: &str) -> Vec<String> {
    raw.split(|c: char| c.is_whitespace() || c == ',').filter_map(clean_word).filter(|w| !w.starts_with('@')).collect()
}

fn has_a_note(words: &[String]) -> bool {
    words.iter().any(|w| !w.starts_with('_'))
}

#[derive(Debug, PartialEq)]
pub enum Misfit {
    /// No notes in it at all.
    Empty,
    /// A turn would be longer than a circle allows: how many beats.
    TooLong(f64),
    /// A note runs over the end of a turn: which of the new sections.
    Straddles(usize),
    /// More notes in one section than a browser will play.
    TooMany,
}

pub struct Part {
    pub id: u64,
    pub user: String,
    pub name: String,
    /// Tunes ready to send, tempo first.
    sections: Vec<String>,
    /// The section that plays next.
    at: usize,
}

impl Part {
    pub fn sections(&self) -> &[String] {
        &self.sections
    }

    /// Which section plays next, counting from 1.
    pub fn next_up(&self) -> usize {
        self.at + 1
    }
}

/// One part of one turn, as it's heard.
pub struct Heard {
    pub id: u64,
    pub user: String,
    pub name: String,
    pub tune: String,
    /// Which section, and how many there are.
    pub section: (usize, usize),
}

pub struct Circle {
    start: Instant,
    pub bpm: u32,
    pub beats: f64,
    /// The turn that plays next, counting from 0 at the start.
    turn: u32,
    parts: Vec<Part>,
}

impl Circle {
    /// A circle started by this tune: a turn is as long as its first section
    /// (up to the first `|`), at its tempo (120 if it doesn't say).
    pub fn new(now: Instant, raw: &str) -> Result<Circle, Misfit> {
        if !has_a_note(&notes(&raw.replace('|', " "))) {
            return Err(Misfit::Empty);
        }
        let first = raw.split('|').map(notes).find(|w| !w.is_empty()).unwrap_or_default();
        let beats: f64 = first.iter().map(|w| beats(w)).sum();
        if beats > MAX_BEATS + EPS {
            return Err(Misfit::TooLong(beats));
        }
        Ok(Circle { start: now, bpm: tempo(raw).unwrap_or(120), beats, turn: 0, parts: Vec::new() })
    }

    pub fn turn_length(&self) -> Duration {
        Duration::from_secs_f64(self.beats * 60.0 / self.bpm as f64)
    }

    /// When the next turn starts.
    pub fn due(&self) -> Instant {
        self.start + self.turn_length() * self.turn
    }

    /// A tune cut into sections, a turn each, at the circle's tempo (whatever
    /// it asked for). A `|` starts a new section, and so does filling a turn.
    /// A short section is fine: the rest of its turn is quiet.
    pub fn split(&self, raw: &str) -> Result<Vec<String>, Misfit> {
        let mut out: Vec<Vec<String>> = Vec::new();
        for bar in raw.split('|') {
            let mut cur: Vec<String> = Vec::new();
            let mut sum = 0.0;
            for w in notes(bar) {
                if sum >= self.beats - EPS {
                    out.push(std::mem::take(&mut cur));
                    sum = 0.0;
                }
                let b = beats(&w);
                if sum + b > self.beats + EPS {
                    return Err(Misfit::Straddles(out.len() + 1));
                }
                sum += b;
                cur.push(w);
            }
            if !cur.is_empty() {
                out.push(cur);
            }
        }
        if !out.iter().any(|s| has_a_note(s)) {
            return Err(Misfit::Empty);
        }
        if out.iter().any(|s| s.len() > MAX_NOTES) {
            return Err(Misfit::TooMany);
        }
        Ok(out.into_iter().map(|s| format!("@{} {}", self.bpm, s.join(" "))).collect())
    }

    /// Put sections into someone's part (they join if they're new): at the
    /// end, or from section `from` (counting from 1) on, replacing what's
    /// there. Says which sections they went in as, or `None` if there's no
    /// room.
    pub fn set(&mut self, id: u64, user: &str, name: &str, from: Option<usize>, new: Vec<String>) -> Option<RangeInclusive<usize>> {
        let i = match self.parts.iter().position(|p| p.id == id) {
            Some(i) => i,
            None if self.parts.len() >= MAX_PARTS => return None,
            None => {
                self.parts.push(Part { id, user: user.to_string(), name: name.to_string(), sections: Vec::new(), at: 0 });
                self.parts.len() - 1
            }
        };
        let p = &mut self.parts[i];
        let from = from.unwrap_or(p.sections.len() + 1).clamp(1, p.sections.len() + 1) - 1;
        if from + new.len() > MAX_SECTIONS {
            if p.sections.is_empty() {
                self.parts.remove(i);
            }
            return None;
        }
        let n = new.len();
        for (k, s) in new.into_iter().enumerate() {
            match p.sections.get_mut(from + k) {
                Some(old) => *old = s,
                None => p.sections.push(s),
            }
        }
        Some(from + 1..=from + n)
    }

    pub fn part(&self, id: u64) -> Option<&Part> {
        self.parts.iter().find(|p| p.id == id)
    }

    /// How many sections someone's part has.
    pub fn sections_of(&self, id: u64) -> usize {
        self.part(id).map_or(0, |p| p.sections.len())
    }

    /// Someone stops humming. Whether they were.
    pub fn hush(&mut self, id: u64) -> bool {
        let before = self.parts.len();
        self.parts.retain(|p| p.id != id);
        self.parts.len() != before
    }

    /// Take one section (counting from 1) out of someone's part; taking the
    /// last one out stops them. Whether there was one.
    pub fn unset(&mut self, id: u64, n: usize) -> bool {
        let Some(i) = self.parts.iter().position(|p| p.id == id) else { return false };
        let p = &mut self.parts[i];
        if n == 0 || n > p.sections.len() {
            return false;
        }
        p.sections.remove(n - 1);
        if n - 1 < p.at {
            p.at -= 1;
        }
        if p.at >= p.sections.len() {
            p.at = 0;
        }
        if p.sections.is_empty() {
            self.parts.remove(i);
        }
        true
    }

    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Play a turn: anyone who's no longer `here` leaves the circle (their ids
    /// come back as the second list), and everyone else hums their next
    /// section, going back to the first after the last. If the world's fallen
    /// behind, it carries on from now rather than rushing to catch up.
    pub fn play(&mut self, now: Instant, here: impl Fn(u64) -> bool) -> (Vec<Heard>, Vec<u64>) {
        let gone: Vec<u64> = self.parts.iter().filter(|p| !here(p.id)).map(|p| p.id).collect();
        self.parts.retain(|p| here(p.id));
        let mut heard = Vec::new();
        for p in &mut self.parts {
            let n = p.sections.len();
            heard.push(Heard { id: p.id, user: p.user.clone(), name: p.name.clone(), tune: p.sections[p.at].clone(), section: (p.at + 1, n) });
            p.at = (p.at + 1) % n;
        }
        let len = self.turn_length().as_secs_f64();
        let since = now.saturating_duration_since(self.start).as_secs_f64();
        self.turn = (self.turn + 1).max((since / len) as u32 + 1);
        (heard, gone)
    }
}

/// How a beat count reads: "8", or "2.5".
pub fn show_beats(b: f64) -> String {
    if b.fract().abs() < EPS { format!("{}", b as i64) } else { format!("{b}") }
}

/// "section 3" or "sections 3–5".
fn show_range(r: &RangeInclusive<usize>) -> String {
    if r.start() == r.end() { format!("section {}", r.start()) } else { format!("sections {}–{}", r.start(), r.end()) }
}

/// `hum along 3 C E G` → (Some(3), "C E G"). A tune never starts with a
/// bare number, so a number first is always a section.
pub fn section_and_tune(rest: &str) -> (Option<usize>, &str) {
    let rest = rest.trim();
    let (first, tail) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    match first.parse::<usize>() {
        Ok(n) => (Some(n), tail.trim()),
        Err(_) => (None, rest),
    }
}

impl Game {
    /// When the next turn of any circle is due.
    pub(super) fn next_circle_turn(&self) -> Option<Instant> {
        self.circles.values().map(|c| c.due()).min()
    }

    /// Play every circle whose turn has come.
    pub(super) fn circles_turn(&mut self) {
        let now = Instant::now();
        let players = &self.players;
        let mut ended = Vec::new();
        for (&(x, y), c) in self.circles.iter_mut() {
            if c.due() > now {
                continue;
            }
            let (heard, gone) = c.play(now, |id| players.get(&id).is_some_and(|p| (p.x, p.y) == (x, y)));
            for id in gone {
                if let Some(p) = players.get(&id) {
                    let _ = p.tx.send(ToClient::Lines { lines: vec![Line::col(Col::Pink, "🎶 You leave the humming circle behind.")] });
                    let _ = p.tx.send(ToClient::Circle { ms: 0, parts: Vec::new() });
                }
            }
            let ms = c.turn_length().as_millis() as u64;
            for (&pid, p) in players {
                // Residents don't need the tune, and every turn would wake them.
                if (p.x, p.y) != (x, y) || p.bot {
                    continue;
                }
                // The parts, not lines of notes: the browser shows who's humming
                // in a strip and plays the tunes, so the log stays readable.
                let parts: Vec<CirclePart> = heard
                    .iter()
                    .filter(|h| p.hears(&h.user))
                    .map(|h| CirclePart { name: h.name.clone(), you: h.id == pid, section: h.section.0, of: h.section.1, tune: h.tune.clone() })
                    .collect();
                if !parts.is_empty() {
                    let _ = p.tx.send(ToClient::Circle { ms, parts });
                }
            }
            if c.is_empty() {
                ended.push((x, y));
            }
        }
        for (x, y) in ended {
            self.end_circle(x, y);
            self.tell_here(x, y, 0, Line::col(Col::Pink, "🎶 The humming circle comes gently to an end."));
        }
    }

    /// A circle's over: everyone at its spot stops showing it.
    fn end_circle(&mut self, x: i64, y: i64) {
        self.circles.remove(&(x, y));
        for p in self.players.values().filter(|p| (p.x, p.y) == (x, y)) {
            let _ = p.tx.send(ToClient::Circle { ms: 0, parts: Vec::new() });
        }
    }

    /// `hum along [section] <tune>`: start a circle here, or add to (or
    /// change) your part in the one that's going. Bare, it shows your part.
    pub(super) fn cmd_hum_along(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some(p) = self.players.get(&id) else { return Ok(()) };
        let (x, y, name, user) = (p.x, p.y, p.name.clone(), p.user.clone());
        let (from, tune) = section_and_tune(rest);
        if tune.is_empty() {
            self.show_part(id, x, y);
            return Ok(());
        }
        // A song from the songbook, or notes.
        let from_book;
        let tune = match songbook::find(tune) {
            Some(Found::Part(part)) => {
                from_book = (part.tune)();
                from_book.as_str()
            }
            Some(Found::Song(song)) => {
                let mut lines = vec![Line::col(Col::Pink, format!("🎶 Which part of {}?", song.title))];
                lines.extend(song_lines(song));
                self.send(id, lines);
                return Ok(());
            }
            None => tune,
        };
        let now = Instant::now();
        let started = !self.circles.contains_key(&(x, y));
        if started {
            match Circle::new(now, tune) {
                Ok(c) => {
                    self.circles.insert((x, y), c);
                }
                Err(m) => {
                    self.say1(id, Line::plain(misfit(&m, None)));
                    return Ok(());
                }
            }
        }
        let c = self.circles.get_mut(&(x, y)).expect("just made sure");
        let sections = match c.split(tune) {
            Ok(s) => s,
            Err(m) => {
                let msg = misfit(&m, Some(c.beats));
                if started {
                    self.circles.remove(&(x, y));
                }
                self.say1(id, Line::plain(msg));
                return Ok(());
            }
        };
        let had = c.sections_of(id);
        let joining = c.part(id).is_none();
        let Some(placed) = c.set(id, &user, &name, from, sections) else {
            let msg = if joining {
                format!("🎶 The circle's full: {MAX_PARTS} people are humming in it already.")
            } else {
                format!("🎶 A part can have {MAX_SECTIONS} sections at most. 'hush 3' takes section 3 out.")
            };
            self.say1(id, Line::plain(msg));
            return Ok(());
        };
        let (beats, bpm) = (c.beats, c.bpm);
        let total = c.sections_of(id);
        let wait = c.due().saturating_duration_since(now).as_secs_f64().ceil() as u64;
        let secs = if wait == 1 { "1 second".to_string() } else { format!("{wait} seconds") };
        let what = show_range(&placed);
        let mine = if started {
            let goes = if total == 1 { "Your part goes".to_string() } else { format!("Your {total} sections go") };
            format!("🎶 You start a humming circle: it goes round every {} beats at @{bpm}. {goes} round and round until you 'hush'.", show_beats(beats))
        } else if joining {
            format!("🎶 You'll come in at the next turn, in {secs}, with {what}. Your part goes round and round until you 'hush'.")
        } else if *placed.start() <= had {
            format!("🎶 You change {what}. It plays the new way next time it comes round.")
        } else {
            format!("🎶 You add {what} to your part ({total} now).")
        };
        self.say1(id, Line::col(Col::Pink, mine));
        if started {
            self.tell_here_from(x, y, id, &user, Line::col(Col::Pink, format!("🎶 {name} starts a humming circle. 'hum along' with a part of your own to join in.")));
        } else if joining {
            self.tell_here_from(x, y, id, &user, Line::col(Col::Pink, format!("🎶 {name} joins the humming circle.")));
        }
        if started || joining {
            for (&pid, o) in &self.players {
                let d = (o.x - x).abs().max((o.y - y).abs());
                if pid != id && (1..=2).contains(&d) && o.hears(&user) {
                    let _ = o.tx.send(ToClient::Lines { lines: vec![Line::col(Col::Pink, "♪ You hear people humming together nearby.")] });
                }
            }
        }
        Ok(())
    }

    /// Bare `hum along`: how the circle here goes, and your part, numbered.
    fn show_part(&self, id: u64, x: i64, y: i64) {
        let Some(c) = self.circles.get(&(x, y)) else {
            let mut lines = vec![Line::col(Col::Pink, "🎶 Hum along with what? Start a humming circle with a song (tap one), or your own notes: 'hum along @60 C E G C5'. Others join in with parts of their own.")];
            lines.extend(songbook_lines());
            self.send(id, lines);
            return;
        };
        let mut lines = vec![Line::col(Col::Pink, format!("🎶 The circle here goes round every {} beats at @{}.", show_beats(c.beats), c.bpm))];
        match c.part(id) {
            None => {
                lines.push(Line::col(Col::Dim, "Join in with a part of your own (it comes in at the next turn): a song, or 'hum along <notes>'."));
                lines.extend(songbook_lines());
            }
            Some(p) => {
                lines.push(Line::col(Col::Dim, format!("Your part (section {} is next):", p.next_up())));
                for (k, s) in p.sections().iter().enumerate() {
                    // Without the ♪s, so the browser doesn't play the list.
                    let notes = s.split_once(' ').map_or("", |(_, n)| n);
                    lines.push(Line::new().b(Col::Pink, format!("  {:>2}  ", k + 1)).c(Col::Dim, notes));
                }
                lines.push(Line::col(Col::Dim, "'hum along 2 …' changes section 2, 'hush 2' takes it out, 'hush' stops."));
            }
        }
        self.send(id, lines);
    }

    /// `hush`: stop humming along. `hush 3`: take section 3 out of your part.
    pub(super) fn cmd_hush(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let Some(c) = self.circles.get_mut(&(x, y)).filter(|c| c.part(id).is_some()) else {
            self.say1(id, Line::plain("You're not humming along with anything. Shh anyway."));
            return Ok(());
        };
        let have = c.sections_of(id);
        if let Ok(n) = rest.trim().parse::<usize>()
            && have > 1
        {
            let msg = if c.unset(id, n) {
                Line::col(Col::Pink, format!("🎶 You take section {n} out. Your part has {} now.", have - 1))
            } else {
                Line::plain(format!("Your part only has sections 1 to {have}. ('hum along' shows them.)"))
            };
            self.say1(id, msg);
            return Ok(());
        }
        c.hush(id);
        if c.is_empty() {
            self.end_circle(x, y);
            self.say1(id, Line::col(Col::Pink, "🎶 You let the last note fade, and the humming circle comes gently to an end."));
            self.tell_here_from(x, y, id, &user, Line::col(Col::Pink, format!("🎶 {name} lets the last note fade, and the humming circle comes gently to an end.")));
        } else {
            self.say1(id, Line::col(Col::Pink, "🎶 You hush. The circle goes on without you."));
            self.tell_here_from(x, y, id, &user, Line::col(Col::Pink, format!("🎶 {name} hushes.")));
        }
        Ok(())
    }
}

/// The songbook, a song a line, each part a tap.
fn songbook_lines() -> Vec<Line> {
    let mut lines: Vec<Line> = SONGS.iter().flat_map(song_lines).collect();
    lines.push(Line::col(Col::Dim, "  (or type it: hum along canon bass, hum along frère jacques, hum along twinkle…)"));
    lines
}

/// One song's parts as taps: "Pachelbel's Canon: bass (the ground…) · tune (…)".
fn song_lines(song: &Song) -> Vec<Line> {
    if song.parts.len() == 1 {
        return vec![Line::new().t("  ").run(Col::Pink, song.title, songbook::command(song, &song.parts[0]))];
    }
    let mut line = Line::new().t("  ").c(Col::Pink, format!("{}: ", song.title));
    for (i, p) in song.parts.iter().enumerate() {
        if i > 0 {
            line = line.c(Col::Dim, " · ");
        }
        line = line.run(Col::Pink, p.name, songbook::command(song, p)).c(Col::Dim, format!(" ({})", p.about));
    }
    vec![line]
}

fn misfit(m: &Misfit, turn: Option<f64>) -> String {
    match (m, turn) {
        (Misfit::Empty, _) => "Hum along which notes? e.g. 'hum along @60 C E G C5' (A to G, - holds a note longer, / makes it shorter, _ is a rest, | starts a new section).".to_string(),
        (Misfit::TooLong(b), _) => format!("That's {} beats: a circle's turn can be {} at most.", show_beats(*b), MAX_BEATS),
        (Misfit::Straddles(k), Some(turn)) => format!(
            "A note in section {k} runs over the end of a turn (this circle goes round in {} beats). Try shortening it, or a | where the section should end.",
            show_beats(turn)
        ),
        (Misfit::Straddles(k), None) => format!("A note in section {k} runs over the end of a turn."),
        (Misfit::TooMany, _) => format!("That's a lot of notes! A section can have {MAX_NOTES} at most."),
    }
}

/// Every circle by spot, for the game.
pub type Circles = HashMap<(i64, i64), Circle>;

#[cfg(test)]
mod tests {
    use super::*;

    const GROUND: &str = "@110 D3- A2- B2- F#2- G2- D2- G2- A2-";
    const FRERE: &str = "@120 C D E C C D E C | E F G- E F G- | G/ A/ G/ F/ E C G/ A/ G/ F/ E C | C G3 C- C G3 C-";

    fn circle(tune: &str) -> Circle {
        Circle::new(Instant::now(), tune).unwrap()
    }

    #[test]
    fn a_circle_goes_round_as_long_as_its_first_section() {
        let c = circle(GROUND);
        assert_eq!((c.bpm, c.beats), (110, 16.0));
        assert_eq!(c.turn_length(), Duration::from_secs_f64(16.0 * 60.0 / 110.0));
        assert_eq!(circle(FRERE).beats, 8.0);
        assert_eq!(circle("C. D/ E// _").beats, 3.25);
        assert_eq!(Circle::new(Instant::now(), "la la").err(), Some(Misfit::Empty));
        assert_eq!(Circle::new(Instant::now(), "_ _-").err(), Some(Misfit::Empty));
        assert_eq!(Circle::new(Instant::now(), &"C------- ".repeat(9)).err(), Some(Misfit::TooLong(72.0)));
    }

    #[test]
    fn tunes_split_into_sections_at_bars_and_full_turns() {
        let c = circle(FRERE);
        assert_eq!(
            c.split(FRERE).unwrap(),
            vec!["@120 C D E C C D E C", "@120 E F G- E F G-", "@120 G/ A/ G/ F/ E C G/ A/ G/ F/ E C", "@120 C G3 C- C G3 C-"]
        );
        // With no bars, a long tune is cut every turn.
        assert_eq!(c.split("@200 C D E C C D E C E F G- E F G-").unwrap(), vec!["@120 C D E C C D E C", "@120 E F G- E F G-"]);
        // Short sections are fine, quiet ones too, as long as something plays.
        assert_eq!(c.split("C | _ | E").unwrap(), vec!["@120 C", "@120 _", "@120 E"]);
        assert_eq!(c.split("C D E F G A B C5-").err(), Some(Misfit::Straddles(1)));
        assert_eq!(c.split("C D E F G A B C5 | C------ C-").err(), Some(Misfit::Straddles(2)));
        assert_eq!(c.split("hello | _").err(), Some(Misfit::Empty));
        let long = circle(&"C--- ".repeat(16));
        assert_eq!(long.split(&"C/// ".repeat(65)).err(), Some(Misfit::TooMany));
    }

    #[test]
    fn parts_loop_their_sections() {
        let t0 = Instant::now();
        let mut c = Circle::new(t0, FRERE).unwrap();
        let len = c.turn_length();
        assert_eq!(c.set(1, "a", "A", None, c.split(FRERE).unwrap()), Some(1..=4));
        assert_eq!(c.due(), t0);
        let section = |c: &mut Circle, k: u32| c.play(t0 + len * k, |_| true).0.iter().map(|h| h.section).collect::<Vec<_>>();
        assert_eq!(section(&mut c, 0), vec![(1, 4)]);
        // The round: the second voice comes in a turn later.
        c.set(2, "b", "B", None, c.split(FRERE).unwrap());
        assert_eq!(section(&mut c, 1), vec![(2, 4), (1, 4)]);
        assert_eq!(section(&mut c, 2), vec![(3, 4), (2, 4)]);
        assert_eq!(section(&mut c, 3), vec![(4, 4), (3, 4)]);
        assert_eq!(section(&mut c, 4), vec![(1, 4), (4, 4)], "round and round");
        assert_eq!(c.due(), t0 + len * 5);
    }

    #[test]
    fn sections_can_be_changed_added_and_taken_out() {
        let t0 = Instant::now();
        let mut c = Circle::new(t0, GROUND).unwrap();
        let s = |c: &Circle, t: &str| c.split(t).unwrap();
        c.set(1, "a", "A", None, s(&c, "C | D | E"));
        // Oops, section 2 should have been an F.
        assert_eq!(c.set(1, "a", "A", Some(2), s(&c, "F")), Some(2..=2));
        assert_eq!(c.part(1).unwrap().sections(), ["@110 C", "@110 F", "@110 E"]);
        // From section 3 on, running over the end adds more.
        assert_eq!(c.set(1, "a", "A", Some(3), s(&c, "G | A")), Some(3..=4));
        // A number past the end just adds.
        assert_eq!(c.set(1, "a", "A", Some(9), s(&c, "B")), Some(5..=5));
        assert_eq!(c.part(1).unwrap().sections(), ["@110 C", "@110 F", "@110 G", "@110 A", "@110 B"]);

        c.play(t0, |_| true);
        c.play(t0 + c.turn_length(), |_| true);
        assert_eq!(c.part(1).unwrap().next_up(), 3);
        assert!(c.unset(1, 1));
        assert_eq!(c.part(1).unwrap().next_up(), 2, "still G next");
        assert!(!c.unset(1, 9));
        for _ in 0..4 {
            c.unset(1, 1);
        }
        assert!(c.is_empty(), "taking the last section out stops them");
    }

    #[test]
    fn people_who_wander_off_or_hush_leave_the_circle() {
        let t0 = Instant::now();
        let mut c = Circle::new(t0, GROUND).unwrap();
        c.set(1, "a", "A", None, vec!["@110 C".into()]);
        c.set(2, "b", "B", None, vec!["@110 E".into()]);
        let (heard, gone) = c.play(t0, |id| id == 1);
        assert_eq!((heard.len(), gone), (1, vec![2]));
        assert!(c.hush(1) && !c.hush(1) && c.is_empty());
    }

    #[test]
    fn a_late_world_picks_up_from_now() {
        let t0 = Instant::now();
        let mut c = Circle::new(t0, GROUND).unwrap();
        let len = c.turn_length();
        c.set(1, "a", "A", None, vec!["@110 C".into()]);
        c.play(t0 + len * 5 + len / 2, |_| true);
        assert_eq!(c.due(), t0 + len * 6);
    }

    #[test]
    fn parts_and_circles_have_room_for_plenty() {
        let mut c = circle(GROUND);
        let one = || vec!["@110 C".to_string()];
        for _ in 0..MAX_SECTIONS {
            assert!(c.set(1, "a", "A", None, one()).is_some());
        }
        assert_eq!(c.set(1, "a", "A", None, one()), None);
        assert!(c.set(1, "a", "A", Some(16), one()).is_some(), "but changing one is fine");
        for id in 2..=MAX_PARTS as u64 {
            assert!(c.set(id, "x", "X", None, one()).is_some());
        }
        assert_eq!(c.set(99, "y", "Y", None, one()), None);
        assert!(c.part(99).is_none());
    }

    #[test]
    fn pachelbels_canon_from_playing_md_fits_a_section_a_turn() {
        let c = circle(GROUND);
        // Sections 5 and 9 from docs/playing.md: the quick run, and the very fast part.
        for sec in ["D5/ C#5/ D5/ D4/ C#4/ A4/ E4/ F#4/ D4/ D5/ C#5/ B4/ C#5/ F#5/ A5/ B5/ G5/ F#5/ E5/ G5/ F#5/ E5/ D5/ C#5/ B4/ A4/ G4/ F#4/ E4/ G4/ F#4/ E4/", "A5/ F#5// G5// A5/ F#5// G5// A5// A4// B4// C#5// D5// E5// F#5// G5// F#5/ D5// E5// F#5/ F#4// G4// A4// B4// A4// G4// A4// F#4// G4// A4// G4/ B4// A4// G4/ F#4// E4// F#4// E4// D4// E4// F#4// G4// A4// B4// G4/ B4// A4// B4/ C#5// D5// A4// B4// C#5// D5// E5// F#5// G5// A5//"] {
            let split = c.split(sec).unwrap();
            assert_eq!(split.len(), 1, "{sec}");
            assert_eq!(read_beats(&split[0]), 16.0);
        }
        let all = ["F#5- E5- D5- C#5- B4- A4- B4- C#5-", "D5- C#5- B4- A4- G4- F#4- G4- E4-", "D4 F#4 A4 G4 F#4 D4 F#4 E4 D4 B3 D4 A4 G4 B4 A4 G4", "F#4 D4 E4 C#5 D5 F#5 A5 A4 B4 G4 A4 F#4 D4 D5 D5. C#5/", "D5/ C#5/ D5/ D4/ C#4/ A4/ E4/ F#4/ D4/ D5/ C#5/ B4/ C#5/ F#5/ A5/ B5/ G5/ F#5/ E5/ G5/ F#5/ E5/ D5/ C#5/ B4/ A4/ G4/ F#4/ E4/ G4/ F#4/ E4/", "D4/ E4/ F#4/ G4/ A4/ E4/ A4/ G4/ F#4/ B4/ A4/ G4/ A4/ G4/ F#4/ E4/ D4/ B3/ B4/ C#5/ D5/ C#5/ B4/ A4/ G4/ F#4/ E4/ B4/ A4/ B4/ A4/ G4/", "F#4 F#5 E5- _ D5 F#5- B5- A5- B5- C#6-", "D6 D5 C#5- _ B4 D5- D5-- D5 D5 G5 E5 A5", "A5/ F#5// G5// A5/ F#5// G5// A5// A4// B4// C#5// D5// E5// F#5// G5// F#5/ D5// E5// F#5/ F#4// G4// A4// B4// A4// G4// A4// F#4// G4// A4// G4/ B4// A4// G4/ F#4// E4// F#4// E4// D4// E4// F#4// G4// A4// B4// G4/ B4// A4// B4/ C#5// D5// A4// B4// C#5// D5// E5// F#5// G5// A5//", "F#5/ D5// E5// F#5/ E5// D5// E5// C#5// D5// E5// F#5// E5// D5// C#5// D5/ B4// C#5// D5/ D4// E4// F#4// G4// F#4// E4// F#4// D5// C#5// D5// B4/ D5// C#5// B4/ A4// G4// A4// G4// F#4// G4// A4// B4// C#5// D5// B4/ D5// C#5// D5/ C#5// B4// C#5// D5// E5// D5// C#5// D5// B4// C#5//"].join(" | ");
        assert_eq!(c.split(&all).unwrap().len(), 10);
    }

    fn read_beats(tune: &str) -> f64 {
        notes(tune).iter().map(|w| beats(w)).sum()
    }

    #[test]
    fn a_number_first_is_a_section() {
        assert_eq!(section_and_tune("3 C E G"), (Some(3), "C E G"));
        assert_eq!(section_and_tune("@100 C E G"), (None, "@100 C E G"));
        assert_eq!(section_and_tune(""), (None, ""));
    }
}
