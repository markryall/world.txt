//! Status lights on noticeboards. `status CI` at a board gives a secret
//! address that an outside tool (a build, an uptime check) posts "up" or
//! "down" to. Everyone who follows the board, or is near it, sees a little
//! light in their side panel, and hears about it when it changes: when it
//! goes down, and when it comes back up. Staying up says nothing.
//!
//! A flapping check can't chatter: a light speaks at most once a minute,
//! and if it's flipped about in between, only where it ended up is said.
//!
//! While something's down, a small crab sits on the noticeboard and sulks.
//! It can be patted.

use super::progress::NEAR;
use super::Game;
use crate::protocol::{Col, Light, Line};
use crate::server::HookReply;
use std::time::{Duration, Instant};

/// The longest note a light keeps ("build 42 failed"), the same as a board
/// note, so there's room for a link: `down [build 42](https://…) failed`.
const MAX_NOTE: usize = 280;
/// Saved and sent out at most this often.
const SETTLE: Duration = Duration::from_secs(1);
/// A light says it's changed at most this often.
const SPEAK_GAP: Duration = Duration::from_secs(60);

pub struct Status {
    board: i64,
    name: String,
    up: bool,
    note: Option<String>,
    /// What everyone was last told (None: nothing yet).
    told: Option<bool>,
    /// When they were last told.
    said: Option<Instant>,
    /// Changed since it was last saved and sent out.
    pending: bool,
    saved: Option<Instant>,
}

#[derive(Debug, PartialEq)]
enum Post {
    Is { up: bool, note: Option<String> },
    Clear,
}

/// "up", "down build 42 failed", "passed", "FAILED: tests", "DOWN timeout", "clear".
fn parse(text: &str) -> Option<Post> {
    let t = text.trim();
    let split = t.find(|c: char| !c.is_alphanumeric()).unwrap_or(t.len());
    let word = t[..split].to_lowercase();
    let rest = t[split..].trim_start_matches(|c: char| c == ':' || c == '-' || c == '—' || c == ',' || c.is_whitespace());
    let up = match word.as_str() {
        "up" | "ok" | "okay" | "pass" | "passed" | "passing" | "green" | "success" | "succeeded" | "healthy" | "fixed" | "resolved" | "good" => true,
        "down" | "fail" | "failed" | "failing" | "failure" | "red" | "error" | "errored" | "broken" | "critical" | "bad" | "unhealthy" => false,
        "clear" | "reset" | "remove" | "hide" if rest.is_empty() => return Some(Post::Clear),
        _ => return None,
    };
    let note: String = rest.trim().chars().take(MAX_NOTE).collect();
    Some(Post::Is { up, note: (!note.is_empty()).then_some(note) })
}

impl Status {
    fn light(&self) -> Light {
        Light { label: self.name.clone(), up: self.up, note: self.note.clone() }
    }
}

impl Game {
    pub(super) fn load_lights(&mut self) {
        match self.world.db.status_lights() {
            Ok(lights) => {
                for (hook, board, name, up, note) in lights {
                    self.lights.insert(hook, Status { board, name, up, note, told: Some(up), said: None, pending: false, saved: None });
                }
            }
            Err(e) => eprintln!("loading status lights: {e}"),
        }
    }

    /// Whether someone at (x, y) who follows `followed` boards keeps an eye on `board`.
    fn watches(&self, x: i64, y: i64, followed: &[i64], board: i64) -> bool {
        followed.contains(&board) || self.world.things.iter().any(|t| t.id == board && (t.x - x).abs() <= NEAR && (t.y - y).abs() <= NEAR)
    }

    pub(super) fn lights_for(&self, id: u64) -> Vec<Light> {
        if self.lights.is_empty() {
            return vec![];
        }
        let Some(p) = self.players.get(&id) else { return vec![] };
        let followed: Vec<i64> = self.world.db.followed(&p.user).unwrap_or_default().into_iter().map(|(b, _)| b).collect();
        self.lights.values().filter(|l| self.watches(p.x, p.y, &followed, l.board)).map(Status::light).collect()
    }

    pub(super) fn lights_on(&self, board: i64) -> Vec<(i64, String)> {
        self.lights.iter().filter(|(_, l)| l.board == board).map(|(&h, l)| (h, l.name.clone())).collect()
    }

    /// An outside tool saying whether something's up or down.
    pub(super) fn status_post(&mut self, hook: i64, board: i64, name: String, text: &str) -> HookReply {
        let Some(post) = parse(text) else { return HookReply::NotStatus };
        match post {
            Post::Clear => {
                if self.lights.remove(&hook).is_some() {
                    let _ = self.world.db.set_light(hook, None);
                    self.bars_changed(board);
                }
            }
            Post::Is { up, note } => {
                // The first word from something that's fine needn't be said.
                let light = self.lights.entry(hook).or_insert(Status { board, name, up, note: None, told: up.then_some(true), said: None, pending: true, saved: None });
                if (light.up, &light.note) != (up, &note) {
                    (light.up, light.note, light.pending) = (up, note, true);
                }
                self.flush_light(hook);
            }
        }
        HookReply::Progressed
    }

    /// Save a light and freshen side panels if it's changed (at most once a
    /// second), and say so if it's gone up or down (at most once a minute).
    fn flush_light(&mut self, hook: i64) {
        let Some(l) = self.lights.get_mut(&hook) else { return };
        if l.pending && !l.saved.is_some_and(|t| t.elapsed() < SETTLE) {
            l.pending = false;
            l.saved = Some(Instant::now());
            let (board, up, note) = (l.board, l.up, l.note.clone());
            let _ = self.world.db.set_light(hook, Some((up, note.as_deref())));
            let _ = self.world.db.touch_hook(hook, super::boards::now());
            self.bars_changed(board);
        }
        let Some(l) = self.lights.get_mut(&hook) else { return };
        if l.told == Some(l.up) || l.said.is_some_and(|t| t.elapsed() < SPEAK_GAP) {
            return;
        }
        l.told = Some(l.up);
        l.said = Some(Instant::now());
        let (board, name, up, note) = (l.board, l.name.clone(), l.up, l.note.clone());
        self.announce_light(board, &name, up, note.as_deref());
    }

    fn announce_light(&mut self, board: i64, name: &str, up: bool, note: Option<&str>) {
        let Some((bx, by)) = self.world.things.iter().find(|t| t.id == board).map(|t| (t.x, t.y)) else { return };
        let place = self.world.place_title(bx, by);
        let followers = self.world.db.followers(board).unwrap_or_default();
        let note = note.map(|n| format!(": {n}")).unwrap_or_default();
        let (line, crab) = if up {
            (
                Line::new().t("🟢 ").b(Col::Green, name).c(Col::Green, format!(" is back up (at {place}){note}")),
                ["The crab on the noticeboard does a happy sideways shuffle and scuttles off.", "The crab stops sulking, clicks its claws twice, and wanders off to the shore.", "The crab looks at the board, decides all is forgiven, and scuttles away."],
            )
        } else {
            (
                Line::new().t("🔴 ").b(Col::Red, name).c(Col::Red, format!(" is down (at {place}){note}")),
                ["A small crab climbs onto the noticeboard and sulks.", "A crab scuttles up the noticeboard's post and settles in to glare.", "A crab arrives from nowhere, sits on the noticeboard and folds its claws crossly."],
            )
        };
        let crab = Line::col(Col::Sand, format!("🦀 {}", self.rng.pick(&crab)));
        let ids: Vec<u64> = self
            .players
            .iter()
            .filter(|(_, p)| followers.contains(&p.user) || ((p.x - bx).abs() <= NEAR && (p.y - by).abs() <= NEAR))
            .map(|(&id, _)| id)
            .collect();
        for id in ids {
            let Some(p) = self.players.get(&id) else { continue };
            let mut lines = vec![line.clone()];
            // Only those right at the board see the crab come and go, and
            // only when it's the first light down (or the last one back up).
            let downs = self.lights.values().filter(|l| l.board == board && !l.up).count();
            if (p.x, p.y) == (bx, by) && downs == usize::from(!up) {
                lines.push(crab.clone());
            }
            self.send(id, lines);
        }
    }

    /// Every stride: lights that changed while settling or keeping quiet.
    pub(super) fn lights_tick(&mut self) {
        let due: Vec<i64> = self.lights.iter().filter(|(_, l)| l.pending || l.told != Some(l.up)).map(|(&h, _)| h).collect();
        for hook in due {
            self.flush_light(hook);
        }
    }

    pub(super) fn drop_light(&mut self, hook: i64) {
        if let Some(l) = self.lights.remove(&hook) {
            self.bars_changed(l.board);
        }
    }

    /// What's down on a board, for the crab.
    fn down_on(&self, board: i64) -> Vec<String> {
        self.lights.values().filter(|l| l.board == board && !l.up).map(|l| l.name.clone()).collect()
    }

    /// Looking at a board where something's down: there's a crab.
    pub(super) fn crab_lines_here(&self, id: u64) -> Vec<Line> {
        let Some(p) = self.players.get(&id) else { return vec![] };
        let Some((board, _)) = self.board_here(p.x, p.y) else { return vec![] };
        let down = self.down_on(board);
        if down.is_empty() {
            return vec![];
        }
        let about = crate::server::world::join_and(&down);
        vec![Line::col(Col::Sand, format!("🦀 A small crab sits on top of the noticeboard, sulking about {about}."))]
    }

    /// `pat crab`, `feed crab`, `cheer up crab`: it's not the crab's fault.
    pub(super) fn crab_command(&mut self, id: u64, verb: &str, rest: &str) -> Option<anyhow::Result<()>> {
        let rest = rest.trim().to_lowercase();
        let rest = rest.strip_prefix("up ").unwrap_or(&rest).trim_start_matches("the ").trim();
        if !rest.starts_with("crab") || !matches!(verb, "pat" | "pet" | "stroke" | "feed" | "cheer" | "comfort" | "hug" | "tickle") {
            return None;
        }
        let p = self.players.get(&id)?;
        let (x, y, name) = (p.x, p.y, p.name.clone());
        let Some((board, _)) = self.board_here(x, y) else {
            self.say1(id, Line::plain("There's no crab here. They only turn up when something's down."));
            return Some(Ok(()));
        };
        let down = self.down_on(board);
        if down.is_empty() {
            self.say1(id, Line::plain("There's no crab here just now: everything's up, so it's gone back to the shore."));
            return Some(Ok(()));
        }
        let about = crate::server::world::join_and(&down);
        let (mine, theirs) = match verb {
            "feed" => (
                format!("You offer the crab a crumb. It takes it with great dignity, and goes back to glaring at {about}."),
                format!("{name} feeds the sulking crab a crumb."),
            ),
            "tickle" => (
                "You tickle the crab. It pinches you, very gently, as a warning.".to_string(),
                format!("{name} tickles the crab and gets a warning pinch."),
            ),
            "hug" => ("You try to hug the crab. It allows this for exactly one second.".to_string(), format!("{name} hugs the crab, briefly.")),
            _ => (
                format!("You pat the crab. It clicks a claw, slightly comforted, but it won't be properly happy until {about} is back up."),
                format!("{name} pats the sulking crab."),
            ),
        };
        self.say1(id, Line::col(Col::Sand, format!("🦀 {mine}")));
        let others: Vec<u64> = self.players.iter().filter(|(o, p)| **o != id && (p.x, p.y) == (x, y)).map(|(&o, _)| o).collect();
        for o in others {
            self.say1(o, Line::col(Col::Sand, format!("🦀 {theirs}")));
        }
        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is(up: bool, note: Option<&str>) -> Option<Post> {
        Some(Post::Is { up, note: note.map(str::to_string) })
    }

    #[test]
    fn reads_up_and_down() {
        assert_eq!(parse("up"), is(true, None));
        assert_eq!(parse("Down build 42 failed"), is(false, Some("build 42 failed")));
        assert_eq!(parse("FAILED: 3 tests"), is(false, Some("3 tests")));
        assert_eq!(parse("passed"), is(true, None));
        assert_eq!(parse("DOWN timeout"), is(false, Some("timeout")));
        assert_eq!(
            parse("down [build 42](https://ci.example.com/builds/42/jobs/7/logs) failed"),
            is(false, Some("[build 42](https://ci.example.com/builds/42/jobs/7/logs) failed"))
        );
        assert_eq!(parse("clear"), Some(Post::Clear));
        assert_eq!(parse("upside"), None);
        assert_eq!(parse("maybe"), None);
    }
}
