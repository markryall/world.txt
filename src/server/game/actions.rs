//! What can I do here? Each time the side panel goes out, the world works out
//! the everyday things you could do where you're standing, so the browser
//! can offer them as taps and a phone's keyboard can stay down.
//!
//! Only ordinary, practical things you can plainly see: the playful ones
//! people find through the scenery (see docs/secrets.md) never turn up here.
//! People (and the island folk) first, then creatures, then things, then the ground; the browser
//! shows the first few and keeps the rest under "more…".

use super::Game;
use super::companions;
use super::{builds, cook, gramophone, jukebox, meetings, music, radio};
use crate::protocol::Action;
use crate::server::terrain::Terrain;
use crate::server::world::{Thing, room_of};

/// At most this many people get a chip of their own.
const PEOPLE: usize = 2;
/// A companion that's just been patted doesn't need suggesting again for a while.
const PAT_AGAIN: std::time::Duration = std::time::Duration::from_secs(10 * 60);

fn run(label: impl Into<String>, cmd: impl Into<String>) -> Action {
    Action {
        label: label.into(),
        run: Some(cmd.into()),
        fill: None,
    }
}

fn fill(label: impl Into<String>, text: impl Into<String>) -> Action {
    Action {
        label: label.into(),
        run: None,
        fill: Some(text.into()),
    }
}

impl Game {
    /// Everything there is to do where `id` is standing, most likely first.
    pub(super) fn actions_for(&self, id: u64) -> Vec<Action> {
        let Some(me) = self.players.get(&id) else {
            return vec![];
        };
        let (x, y) = (me.x, me.y);
        let mut out = Vec::new();

        // People.
        let mut here: Vec<&str> = self
            .players
            .iter()
            .filter(|(pid, p)| **pid != id && (p.x, p.y) == (x, y))
            .map(|(_, p)| p.name.as_str())
            .collect();
        here.sort_by_key(|n| n.to_lowercase());
        if !here.is_empty() {
            out.push(run("👋 wave", "wave"));
        }
        for name in here.iter().take(PEOPLE) {
            out.push(fill(format!("💬 {name}…"), format!("@{name} ")));
        }

        // The island folk, if any are about.
        out.extend(self.folk_actions(id));

        // Creatures: your friend (unless they've just had a pat), or a wild
        // one that might become one.
        if let Some((a, n)) = &me.companion
            && let Some(an) = companions::animal(a)
        {
            if self
                .patted
                .get(&me.user)
                .is_none_or(|t| t.elapsed() > PAT_AGAIN)
            {
                let who = n.clone().unwrap_or_else(|| format!("the {}", an.name));
                out.push(run(format!("{} pat {who}", an.icon), "pet"));
            }
        } else if let Some(w) = self.wild.iter().find(|w| (w.x, w.y) == (x, y)) {
            out.push(run(
                format!("🍓 befriend the {}", w.animal.name),
                format!("befriend {}", w.animal.name),
            ));
        }

        // Things, each offering its own (once each: two boards, one read).
        for t in self.world.things_at(x, y) {
            for a in self.thing_actions(id, t) {
                if !out.contains(&a) {
                    out.push(a);
                }
            }
        }
        if self.in_room(id) {
            out.push(run("🚪 out", "out"));
        }

        // The ground.
        if self.finds.iter().any(|f| (f.x, f.y) == (x, y)) {
            out.push(run("🧺 gather", "gather"));
        }
        if room_of(x, y).is_none() {
            if self.water_near(x, y).is_some() {
                out.push(run("🎣 fish", "fish"));
            }
            if !matches!(
                self.world.terrain(x, y),
                Terrain::Water | Terrain::Pond | Terrain::DeepWater | Terrain::Stone
            ) {
                out.push(fill("🌱 plant…", "plant "));
            }
        }
        out
    }

    /// What you could do with one thing where you stand: for the row, and
    /// for the end of looking at it.
    pub(super) fn thing_actions(&self, id: u64, t: &Thing) -> Vec<Action> {
        let Some(me) = self.players.get(&id) else {
            return vec![];
        };
        let kind = t.kind.as_deref().unwrap_or("");
        let mut out = Vec::new();
        if t.is_board() {
            out.push(run("📖 read", "read"));
            out.push(fill("📌 post…", "post "));
        }
        if kind == music::music_kind() {
            out.push(run("🎶 play", "play"));
            if t.maker == me.user {
                out.push(fill("🎼 teach…", "teach "));
            }
        }
        if kind == gramophone::kind() {
            out.push(if self.spins.contains_key(&t.id) {
                run("⏹ stop music", "stop music")
            } else {
                run("🎵 play music", "play music")
            });
        }
        if kind == jukebox::kind() {
            out.push(run("➕ add songs", "add songs"));
        }
        if kind == radio::kind() {
            out.push(fill("📻 tune…", "tune "));
        }
        if kind == cook::FIRE || kind == cook::OVEN {
            out.push(run("🍳 cook", "cook"));
        }
        if kind == builds::KIND && self.built(t.id) && !self.in_room(id) {
            out.push(run("🚪 go in", "enter"));
        }
        if t.is_lighthouse() && !self.lighthouse_lit() {
            out.push(run("🔧 mend", "mend"));
        }
        if t.is_dock() {
            out.push(run("⛵ sail", "sail"));
        }
        if kind == meetings::kind() {
            out.push(run("🔗 meet", "meet"));
        }
        out
    }
}
