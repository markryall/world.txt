//! Jukeboxes: a gramophone everyone can add to. Songs stay on each person's
//! own computer; only their titles join the jukebox's queue. It takes turns,
//! one song per person, and when it's your turn the world asks your browser
//! to play that song to everyone in earshot, just like a gramophone.

use super::Game;
use super::gramophone::{Spin, clean_title};
use crate::protocol::{Col, Line, Song, ToClient};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const KIND: &str = "jukebox";
/// Songs one person can have waiting on one jukebox.
const MAX_QUEUED: usize = 20;
/// A turn whose browser has sent nothing for this long is skipped.
const QUIET_FOR: Duration = Duration::from_secs(10);

pub fn is_jukebox(name: &str) -> bool {
    let n = name.to_lowercase();
    n.contains("jukebox") || n.contains("juke box")
}

pub fn kind() -> &'static str {
    KIND
}

struct Queued {
    key: String,
    title: String,
}

struct Playing {
    host: u64,
    key: String,
    title: String,
    heard: Instant,
}

/// One jukebox's queue: each person's songs, and whose turn comes next.
#[derive(Default)]
pub struct Jukebox {
    songs: std::collections::HashMap<u64, VecDeque<Queued>>,
    turns: VecDeque<u64>,
    now: Option<Playing>,
}

impl Jukebox {
    /// The next song: whoever's turn it is gives one, then goes to the back
    /// if they have more.
    fn next(&mut self) -> Option<(u64, Queued)> {
        while let Some(who) = self.turns.pop_front() {
            let Some(q) = self.songs.get_mut(&who) else {
                continue;
            };
            let Some(song) = q.pop_front() else { continue };
            if q.is_empty() {
                self.songs.remove(&who);
            } else {
                self.turns.push_back(who);
            }
            return Some((who, song));
        }
        None
    }

    fn add(&mut self, who: u64, songs: Vec<Queued>) -> usize {
        let q = self.songs.entry(who).or_default();
        let room = MAX_QUEUED.saturating_sub(q.len());
        let added = songs.len().min(room);
        q.extend(songs.into_iter().take(room));
        if added > 0 && !self.turns.contains(&who) {
            // Someone new goes before whoever's playing now: they've had
            // their turn this time round.
            let playing = self.now.as_ref().map(|n| n.host);
            match self.turns.iter().position(|w| Some(*w) == playing) {
                Some(i) => self.turns.insert(i, who),
                None => self.turns.push_back(who),
            }
        }
        added
    }

    fn remove(&mut self, who: u64) -> usize {
        self.turns.retain(|w| *w != who);
        self.songs.remove(&who).map_or(0, |q| q.len())
    }
}

impl Game {
    fn jukebox_here(&self, id: u64) -> Option<i64> {
        let p = self.players.get(&id)?;
        self.world
            .things
            .iter()
            .find(|t| t.x == p.x && t.y == p.y && t.kind.as_deref() == Some(KIND))
            .map(|t| t.id)
    }

    pub(super) fn has_jukebox_here(&self, id: u64) -> bool {
        self.jukebox_here(id).is_some()
    }

    /// Would `skip` mean a jukebox song (one playing here, or their own)?
    pub(super) fn juke_skippable(&self, id: u64) -> bool {
        let here = self
            .jukebox_here(id)
            .and_then(|t| self.jukes.get(&t))
            .is_some_and(|j| j.now.is_some());
        here || self
            .jukes
            .values()
            .any(|j| j.now.as_ref().is_some_and(|n| n.host == id))
    }

    /// `add songs`: open the song picker in this browser.
    pub(super) fn cmd_add_songs(&mut self, id: u64) -> anyhow::Result<()> {
        let Some(thing) = self.jukebox_here(id) else {
            self.say1(
                id,
                Line::plain("There's no jukebox here. Try 'make a jukebox'!"),
            );
            return Ok(());
        };
        if self.players[&id].bot {
            self.say1(
                id,
                Line::plain("Jukeboxes play songs from someone's own computer, in the browser."),
            );
            return Ok(());
        }
        if let Some(p) = self.players.get(&id) {
            let _ = p.tx.send(ToClient::JukeAdd { thing });
        }
        self.say1(id, Line::col(Col::Pink, "📀 Choose some songs. They stay on your computer: keep this tab open until your turn comes round."));
        Ok(())
    }

    /// Songs a browser has picked for a jukebox (titles only).
    pub(super) fn queue_songs(&mut self, id: u64, thing: i64, songs: Vec<Song>) {
        let Some(p) = self.players.get(&id) else {
            return;
        };
        let near =
            self.world.things.iter().any(|t| {
                t.id == thing && t.kind.as_deref() == Some(KIND) && t.x == p.x && t.y == p.y
            });
        if !near {
            self.say1(id, Line::plain("Stand at the jukebox to add songs."));
            return;
        }
        let songs: Vec<Queued> = songs
            .into_iter()
            .filter(|s| {
                !s.key.is_empty()
                    && s.key.len() <= 40
                    && s.key.chars().all(|c| c.is_ascii_alphanumeric())
            })
            .filter_map(|s| clean_title(&s.title).map(|title| Queued { key: s.key, title }))
            .collect();
        let added = self.jukes.entry(thing).or_default().add(id, songs);
        let who = p.name.clone();
        let msg = match added {
            0 => "You've already got plenty waiting. Let some play first!".to_string(),
            1 => "📀 One song added. It'll play when your turn comes round.".to_string(),
            n => format!("📀 {n} songs added. One plays each time your turn comes round."),
        };
        self.say1(id, Line::col(Col::Pink, msg));
        if added > 0
            && let Some((x, y)) = self
                .world
                .things
                .iter()
                .find(|t| t.id == thing)
                .map(|t| (t.x, t.y))
        {
            self.tell_here(
                x,
                y,
                id,
                Line::col(
                    Col::Pink,
                    format!("📀 {who} adds some songs to the jukebox."),
                ),
            );
        }
        if self.jukes.get(&thing).is_some_and(|j| j.now.is_none()) {
            self.juke_next(thing);
        }
    }

    /// Play the next song, or fall quiet if there isn't one.
    fn juke_next(&mut self, thing: i64) {
        // Whoever was playing stops.
        if let Some(prev) = self.jukes.get_mut(&thing).and_then(|j| j.now.take()) {
            self.spins.remove(&thing);
            if let Some(p) = self.players.get(&prev.host) {
                let _ = p.tx.send(ToClient::JukeStop { thing });
            }
        }
        // At most one pass through the queue, in case everyone's busy.
        let tries = self.jukes.get(&thing).map_or(0, |j| j.turns.len());
        for _ in 0..=tries {
            let Some(juke) = self.jukes.get_mut(&thing) else {
                return;
            };
            let Some((host, song)) = juke.next() else {
                // Nothing left: the listeners can let go.
                for p in self.players.values() {
                    let _ = p.tx.send(ToClient::AudioEnd { thing });
                }
                return;
            };
            // Someone already playing elsewhere (or gone) waits their turn.
            if !self.players.contains_key(&host) || self.spins.values().any(|s| s.host == host) {
                if self.players.contains_key(&host) {
                    juke.songs.entry(host).or_default().push_front(song);
                    if !juke.turns.contains(&host) {
                        juke.turns.push_back(host);
                    }
                }
                continue;
            }
            juke.now = Some(Playing {
                host,
                key: song.key.clone(),
                title: song.title.clone(),
                heard: Instant::now(),
            });
            self.spins.insert(
                thing,
                Spin {
                    host,
                    title: Some(song.title.clone()),
                    jukebox: true,
                },
            );
            let who = self.players[&host].name.clone();
            if let Some(p) = self.players.get(&host) {
                let _ = p.tx.send(ToClient::JukeTurn {
                    thing,
                    key: song.key,
                });
                let _ = p.tx.send(ToClient::Lines {
                    lines: vec![Line::col(
                        Col::Pink,
                        format!("📀 Your turn! The jukebox is playing {}.", song.title),
                    )],
                });
            }
            let line = Line::new()
                .c(Col::Pink, "🎶 Now playing on the jukebox: ")
                .b(Col::Pink, song.title)
                .c(Col::Pink, format!(" (from {who})"));
            self.tell_earshot(thing, host, line);
            return;
        }
    }

    /// A browser's song finished.
    pub(super) fn track_done(&mut self, id: u64, thing: i64, key: &str) {
        let playing = self
            .jukes
            .get(&thing)
            .and_then(|j| j.now.as_ref())
            .is_some_and(|n| n.host == id && n.key == key);
        if playing {
            self.juke_next(thing);
        }
    }

    /// Music arrived for a jukebox turn: it's still going.
    pub(super) fn juke_heard(&mut self, thing: i64) {
        if let Some(n) = self.jukes.get_mut(&thing).and_then(|j| j.now.as_mut()) {
            n.heard = Instant::now();
        }
    }

    /// Skip a turn whose browser has gone quiet.
    pub(super) fn jukes_tick(&mut self) {
        let quiet: Vec<i64> = self
            .jukes
            .iter()
            .filter(|(_, j)| {
                j.now
                    .as_ref()
                    .is_some_and(|n| n.heard.elapsed() > QUIET_FOR)
            })
            .map(|(t, _)| *t)
            .collect();
        for thing in quiet {
            self.tell_earshot(
                thing,
                u64::MAX,
                Line::col(Col::Dim, "📀 The jukebox skips: that song's gone quiet."),
            );
            self.juke_next(thing);
        }
    }

    /// `jukebox`: what's playing and what's coming.
    pub(super) fn cmd_jukebox(&mut self, id: u64) -> anyhow::Result<()> {
        let Some(thing) = self.jukebox_here(id) else {
            self.say1(
                id,
                Line::plain("There's no jukebox here. Try 'make a jukebox'!"),
            );
            return Ok(());
        };
        let name = |pid: &u64| {
            self.players
                .get(pid)
                .map_or("someone".to_string(), |p| p.name.clone())
        };
        let mut lines = vec![Line::new().b(Col::Magenta, "📀 The jukebox")];
        let juke = self.jukes.get(&thing);
        match juke.and_then(|j| j.now.as_ref()) {
            Some(n) => lines.push(
                Line::new()
                    .c(Col::Pink, "  now: ")
                    .b(Col::Pink, &n.title)
                    .c(Col::Dim, format!(" (from {})", name(&n.host))),
            ),
            None => lines.push(Line::col(
                Col::Dim,
                "  Nothing playing. 'add songs' to start it off!",
            )),
        }
        if let Some(j) = juke {
            // The order they'll come in: one each, round and round.
            let mut queues: Vec<(u64, Vec<&str>)> = j
                .turns
                .iter()
                .map(|w| {
                    (
                        *w,
                        j.songs
                            .get(w)
                            .map(|q| q.iter().map(|s| s.title.as_str()).collect())
                            .unwrap_or_default(),
                    )
                })
                .collect();
            let mut upcoming = Vec::new();
            while upcoming.len() < 8 && queues.iter().any(|(_, q)| !q.is_empty()) {
                for (w, q) in queues.iter_mut() {
                    if !q.is_empty() && upcoming.len() < 8 {
                        upcoming.push((*w, q.remove(0)));
                    }
                }
            }
            for (i, (w, title)) in upcoming.iter().enumerate() {
                lines.push(
                    Line::new()
                        .c(Col::Dim, format!("  {}. ", i + 1))
                        .c(Col::Yellow, *title)
                        .c(Col::Dim, format!(" (from {})", name(w))),
                );
            }
        }
        lines.push(Line::col(Col::Dim, "  add songs · skip · unqueue"));
        self.send(id, lines);
        Ok(())
    }

    /// `skip`: the song's owner, or a keeper.
    pub(super) fn cmd_skip_song(&mut self, id: u64) -> anyhow::Result<()> {
        let Some(thing) = self.jukebox_here(id).or_else(|| {
            self.jukes
                .iter()
                .find(|(_, j)| j.now.as_ref().is_some_and(|n| n.host == id))
                .map(|(t, _)| *t)
        }) else {
            self.say1(id, Line::plain("There's no jukebox here."));
            return Ok(());
        };
        let Some(host) = self
            .jukes
            .get(&thing)
            .and_then(|j| j.now.as_ref())
            .map(|n| n.host)
        else {
            self.say1(id, Line::plain("Nothing's playing."));
            return Ok(());
        };
        if host != id && !self.is_keeper(&self.user(id)) {
            let owner = self
                .players
                .get(&host)
                .map_or("whoever chose it".to_string(), |p| p.name.clone());
            self.say1(
                id,
                Line::plain(format!("It's {owner}'s song, so only {owner} can skip it.")),
            );
            return Ok(());
        }
        let who = self.players[&id].name.clone();
        self.tell_earshot(
            thing,
            u64::MAX,
            Line::col(Col::Dim, format!("📀 {who} skips to the next song.")),
        );
        self.juke_next(thing);
        Ok(())
    }

    /// `unqueue`: take your waiting songs out of every jukebox.
    pub(super) fn cmd_unqueue(&mut self, id: u64) -> anyhow::Result<()> {
        let n: usize = self.jukes.values_mut().map(|j| j.remove(id)).sum();
        let msg = match n {
            0 => "You've no songs waiting.".to_string(),
            1 => "📀 Your waiting song is out of the queue.".to_string(),
            n => format!("📀 Your {n} waiting songs are out of the queue."),
        };
        self.say1(id, Line::col(Col::Pink, msg));
        Ok(())
    }

    /// Someone left: their songs go, and if one was playing, the next comes on.
    pub(super) fn juke_leave(&mut self, id: u64) {
        let playing: Vec<i64> = self
            .jukes
            .iter()
            .filter(|(_, j)| j.now.as_ref().is_some_and(|n| n.host == id))
            .map(|(t, _)| *t)
            .collect();
        for j in self.jukes.values_mut() {
            j.remove(id);
        }
        for thing in playing {
            self.juke_next(thing);
        }
    }

    /// A jukebox was unmade.
    pub(super) fn forget_jukebox(&mut self, thing: i64) {
        if let Some(n) = self.jukes.get_mut(&thing).and_then(|j| j.now.take()) {
            self.spins.remove(&thing);
            if let Some(p) = self.players.get(&n.host) {
                let _ = p.tx.send(ToClient::JukeStop { thing });
            }
            for p in self.players.values() {
                let _ = p.tx.send(ToClient::AudioEnd { thing });
            }
        }
        self.jukes.remove(&thing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn songs(names: &[&str]) -> Vec<Queued> {
        names
            .iter()
            .map(|n| Queued {
                key: n.to_string(),
                title: n.to_string(),
            })
            .collect()
    }

    #[test]
    fn a_jukebox_takes_turns_one_song_each() {
        let mut j = Jukebox::default();
        j.add(1, songs(&["p1", "p2", "p3"]));
        j.add(2, songs(&["s1"]));
        j.add(3, songs(&["t1", "t2"]));
        let order: Vec<String> = std::iter::from_fn(|| j.next().map(|(_, s)| s.title)).collect();
        assert_eq!(order, ["p1", "s1", "t1", "p2", "t2", "p3"]);
    }

    #[test]
    fn someone_arriving_mid_song_goes_before_whoever_is_playing() {
        let mut j = Jukebox::default();
        j.add(1, songs(&["p1", "p2"]));
        let (host, first) = j.next().unwrap();
        j.now = Some(Playing {
            host,
            key: first.key,
            title: first.title,
            heard: Instant::now(),
        });
        j.add(2, songs(&["s1"]));
        let order: Vec<String> = std::iter::from_fn(|| j.next().map(|(_, s)| s.title)).collect();
        assert_eq!(order, ["s1", "p2"]);
    }

    #[test]
    fn nobody_can_flood_it_and_leaving_takes_your_songs() {
        let mut j = Jukebox::default();
        let many: Vec<String> = (0..30).map(|i| format!("x{i}")).collect();
        assert_eq!(
            j.add(
                1,
                songs(&many.iter().map(String::as_str).collect::<Vec<_>>())
            ),
            MAX_QUEUED
        );
        assert_eq!(j.add(1, songs(&["more"])), 0);
        j.add(2, songs(&["s1"]));
        assert_eq!(j.remove(1), MAX_QUEUED);
        assert_eq!(j.next().map(|(w, s)| (w, s.title)), Some((2, "s1".into())));
        assert!(j.next().is_none());
        assert!(is_jukebox("a shiny Jukebox") && !is_jukebox("a gramophone"));
    }
}
