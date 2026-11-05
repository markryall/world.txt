//! Progress bars on noticeboards. A board's maker can `progress Deploy` for
//! a secret address that an outside tool posts "37/100" to, and everyone
//! who follows the board, or is standing near it, sees the bar fill in
//! their side panel. Nothing is said in anyone's log: it just quietly moves
//! along. `clear Deploy` (or the tool posting "clear") takes it away until
//! the next number comes.

use super::Game;
use crate::protocol::{Bar, Col, Line};
use crate::server::HookReply;
use std::time::{Duration, Instant};

/// How near a board you have to be to see its bars without following it.
pub(super) const NEAR: i64 = 3;
/// The longest label a tool can give its bar.
const MAX_LABEL: usize = 32;
/// A bar is saved and sent out at most this often, however fast a tool
/// posts: anything in between only updates it in memory, and the latest
/// goes out when the time comes. Posts are never turned away, so the last
/// one (100/100) can't be lost to a limit.
const SETTLE: Duration = Duration::from_secs(1);

/// A bar that's showing: which board it's on, and where it's up to.
pub struct Progress {
    board: i64,
    name: String,
    done: u64,
    total: u64,
    label: Option<String>,
    /// Changed since it was last saved and sent out.
    pending: bool,
    /// When it was last saved and sent out.
    sent: Option<Instant>,
}

impl Progress {
    fn bar(&self) -> Bar {
        Bar { label: self.label.clone().unwrap_or_else(|| self.name.clone()), done: self.done, total: self.total }
    }
}

/// What a progress hook was sent.
#[derive(Debug, PartialEq)]
enum Post {
    At { done: u64, total: u64, label: Option<String> },
    Clear,
}

/// "37/100", "37 / 100 copying photos", "37%", "3 of 10 photos", or "clear".
fn parse(text: &str) -> Option<Post> {
    let t = text.trim();
    if matches!(t.to_lowercase().as_str(), "clear" | "reset" | "remove" | "hide") {
        return Some(Post::Clear);
    }
    let digits = t.find(|c: char| !c.is_ascii_digit()).unwrap_or(t.len());
    let done: u64 = t[..digits].parse().ok()?;
    let rest = t[digits..].trim_start();
    let (total, rest) = if let Some(r) = rest.strip_prefix('%') {
        (100, r)
    } else {
        let r = rest.strip_prefix('/').or_else(|| rest.strip_prefix("of ")).map(str::trim_start)?;
        let digits = r.find(|c: char| !c.is_ascii_digit()).unwrap_or(r.len());
        (r[..digits].parse().ok()?, &r[digits..])
    };
    if total == 0 {
        return None;
    }
    let label: String = rest.trim().chars().take(MAX_LABEL).collect();
    Some(Post::At { done: done.min(total), total, label: (!label.is_empty()).then_some(label) })
}

impl Game {
    pub(super) fn load_progress(&mut self) {
        match self.world.db.progress_bars() {
            Ok(bars) => {
                for (hook, board, name, done, total, label) in bars {
                    self.bars.insert(hook, Progress { board, name, done, total, label, pending: false, sent: None });
                }
            }
            Err(e) => eprintln!("loading progress bars: {e}"),
        }
    }

    /// The bars `id` can see: on boards they follow, or near where they stand.
    pub(super) fn bars_for(&self, id: u64) -> Vec<Bar> {
        if self.bars.is_empty() {
            return vec![];
        }
        let Some(p) = self.players.get(&id) else { return vec![] };
        let followed: Vec<i64> = self.world.db.followed(&p.user).unwrap_or_default().into_iter().map(|(b, _)| b).collect();
        self.bars
            .values()
            .filter(|b| {
                followed.contains(&b.board)
                    || self.world.things.iter().any(|t| t.id == b.board && (t.x - p.x).abs() <= NEAR && (t.y - p.y).abs() <= NEAR)
            })
            .map(Progress::bar)
            .collect()
    }

    /// A board's bars or lights have changed: freshen the side panels that show them.
    pub(super) fn bars_changed(&self, board: i64) {
        let Some(t) = self.world.things.iter().find(|t| t.id == board) else { return };
        let followers = self.world.db.followers(board).unwrap_or_default();
        for p in self.players.values() {
            if followers.contains(&p.user) || ((t.x - p.x).abs() <= NEAR && (t.y - p.y).abs() <= NEAR) {
                p.status_due.set(true);
            }
        }
    }

    /// An outside tool saying how far along it is.
    pub(super) fn progress_post(&mut self, hook: i64, board: i64, name: String, text: &str) -> HookReply {
        let Some(post) = parse(text) else { return HookReply::NotProgress };
        match post {
            Post::Clear => {
                if self.bars.remove(&hook).is_some() {
                    let _ = self.world.db.set_progress(hook, None);
                    self.bars_changed(board);
                }
            }
            Post::At { done, total, label } => {
                let bar = self.bars.entry(hook).or_insert(Progress { board, name, done, total, label: label.clone(), pending: true, sent: None });
                if (bar.done, bar.total, &bar.label) != (done, total, &label) {
                    (bar.done, bar.total, bar.label, bar.pending) = (done, total, label, true);
                }
                self.flush_bar(hook);
            }
        }
        HookReply::Progressed
    }

    /// Save a bar and freshen the side panels showing it, if it's changed
    /// and it's been long enough since last time.
    fn flush_bar(&mut self, hook: i64) {
        let Some(b) = self.bars.get_mut(&hook) else { return };
        if !b.pending || b.sent.is_some_and(|t| t.elapsed() < SETTLE) {
            return;
        }
        b.pending = false;
        b.sent = Some(Instant::now());
        let (board, done, total, label) = (b.board, b.done, b.total, b.label.clone());
        let _ = self.world.db.set_progress(hook, Some((done, total, label.as_deref())));
        let _ = self.world.db.touch_hook(hook, super::boards::now());
        self.bars_changed(board);
    }

    /// Every stride: send out bars that changed while they were settling.
    pub(super) fn bars_tick(&mut self) {
        let due: Vec<i64> = self.bars.iter().filter(|(_, b)| b.pending).map(|(&h, _)| h).collect();
        for hook in due {
            self.flush_bar(hook);
        }
    }

    /// A progress hook is gone (unhooked): its bar goes too.
    pub(super) fn drop_bar(&mut self, hook: i64) {
        if let Some(b) = self.bars.remove(&hook) {
            self.bars_changed(b.board);
        }
    }

    /// `clear Deploy`: take a finished bar (or a status light) away,
    /// keeping its hook for next time.
    pub(super) fn cmd_clear_bar(&mut self, id: u64, name: &str) -> anyhow::Result<()> {
        let Some(board) = self.hookable_board(id) else { return Ok(()) };
        let name = name.trim();
        let mut mine: Vec<(i64, String, bool)> = self.bars.iter().filter(|(_, b)| b.board == board).map(|(&h, b)| (h, b.name.clone(), false)).collect();
        mine.extend(self.lights_on(board).into_iter().map(|(h, n)| (h, n, true)));
        let found = match mine.as_slice() {
            [] => {
                self.say1(id, Line::plain("There's no progress bar or status showing on this board."));
                return Ok(());
            }
            [only] if name.is_empty() => only.clone(),
            _ if name.is_empty() => {
                let names: Vec<String> = mine.into_iter().map(|(_, n, _)| n).collect();
                self.say1(id, Line::plain(format!("Clear which one? {}", crate::server::world::join_and(&names))));
                return Ok(());
            }
            _ => match mine.into_iter().find(|(_, n, _)| n.eq_ignore_ascii_case(name)) {
                Some(m) => m,
                None => {
                    self.say1(id, Line::plain(format!("This board has nothing showing called {name}.")));
                    return Ok(());
                }
            },
        };
        let (hook, name, light) = found;
        if light {
            self.world.db.set_light(hook, None)?;
            self.drop_light(hook);
            self.say1(id, Line::col(Col::Yellow, format!("🚦 {name}'s light is cleared. It comes back when it next says up or down.")));
        } else {
            self.world.db.set_progress(hook, None)?;
            self.drop_bar(hook);
            self.say1(id, Line::col(Col::Yellow, format!("📊 {name}'s bar is cleared. It comes back when it next says how far along it is.")));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(done: u64, total: u64, label: Option<&str>) -> Option<Post> {
        Some(Post::At { done, total, label: label.map(str::to_string) })
    }

    #[test]
    fn reads_how_far_along() {
        assert_eq!(parse("37/100"), at(37, 100, None));
        assert_eq!(parse(" 3 / 10 copying photos "), at(3, 10, Some("copying photos")));
        assert_eq!(parse("37%"), at(37, 100, None));
        assert_eq!(parse("45% deploying"), at(45, 100, Some("deploying")));
        assert_eq!(parse("3 of 10 photos"), at(3, 10, Some("photos")));
        assert_eq!(parse("12/10"), at(10, 10, None), "never past full");
        assert_eq!(parse("Clear"), Some(Post::Clear));
        assert_eq!(parse("1/0"), None);
        assert_eq!(parse("halfway"), None);
        assert_eq!(parse("37"), None);
    }

    #[test]
    fn bars_say_how_far() {
        let b = |done, total| Bar { label: "x".into(), done, total };
        assert_eq!(b(37, 100).amount(), "37%");
        assert_eq!(b(3, 10).amount(), "3/10");
        assert_eq!(b(3, 10).percent(), 30);
        assert_eq!(b(5, 0).percent(), 0);
    }
}
