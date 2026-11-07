//! What can I do here? Each time the side panel goes out, the world works out
//! the everyday things you could do where you're standing, so the browser
//! can offer them as taps and a phone's keyboard can stay down.
//!
//! Only ordinary, practical things you can plainly see: the playful ones
//! people find through the scenery (see docs/secrets.md) never turn up here.
//! People first, then creatures, then things, then the ground; the browser
//! shows the first few and keeps the rest under "more…".

use super::Game;
use super::companions;
use super::{builds, gramophone};
use crate::protocol::Action;
use crate::server::terrain::Terrain;
use crate::server::world::room_of;

/// At most this many people get a chip of their own.
const PEOPLE: usize = 2;

fn run(label: impl Into<String>, cmd: impl Into<String>) -> Action {
    Action { label: label.into(), run: Some(cmd.into()), fill: None }
}

fn fill(label: impl Into<String>, text: impl Into<String>) -> Action {
    Action { label: label.into(), run: None, fill: Some(text.into()) }
}

impl Game {
    /// Everything there is to do where `id` is standing, most likely first.
    pub(super) fn actions_for(&self, id: u64) -> Vec<Action> {
        let Some(me) = self.players.get(&id) else { return vec![] };
        let (x, y) = (me.x, me.y);
        let mut out = Vec::new();

        // People.
        let mut here: Vec<&str> = self.players.iter().filter(|(pid, p)| **pid != id && (p.x, p.y) == (x, y)).map(|(_, p)| p.name.as_str()).collect();
        here.sort_by_key(|n| n.to_lowercase());
        if !here.is_empty() {
            out.push(run("👋 wave", "wave"));
        }
        for name in here.iter().take(PEOPLE) {
            out.push(fill(format!("💬 {name}…"), format!("@{name} ")));
        }

        // Creatures: your friend, or a wild one that might become one.
        if let Some((a, n)) = &me.companion
            && let Some(an) = companions::animal(a)
        {
            let who = n.clone().unwrap_or_else(|| format!("the {}", an.name));
            out.push(run(format!("{} pat {who}", an.icon), "pet"));
        } else if let Some(w) = self.wild.iter().find(|w| (w.x, w.y) == (x, y)) {
            out.push(run(format!("🍓 befriend the {}", w.animal.name), format!("befriend {}", w.animal.name)));
        }

        // Things.
        let things: Vec<_> = self.world.things_at(x, y).collect();
        if things.iter().any(|t| t.is_board()) {
            out.push(run("📖 read", "read"));
            out.push(fill("📌 post…", "post "));
        }
        if let Some(i) = self.music_box_here(x, y) {
            out.push(run("🎶 play", "play"));
            if self.world.things[i].maker == me.user {
                out.push(fill("🎼 teach…", "teach "));
            }
        }
        if let Some(g) = things.iter().find(|t| t.kind.as_deref() == Some(gramophone::kind())) {
            if self.spins.contains_key(&g.id) {
                out.push(run("⏹ stop music", "stop music"));
            } else {
                out.push(run("🎵 play music", "play music"));
            }
        }
        if self.has_jukebox_here(id) {
            out.push(run("➕ add songs", "add songs"));
        }
        if self.has_radio_here(id) {
            out.push(fill("📻 tune…", "tune "));
        }
        if self.kitchen_here(x, y).is_some() {
            out.push(run("🍳 cook", "cook"));
        }
        if self.in_room(id) {
            out.push(run("🚪 out", "out"));
        } else if things.iter().any(|t| t.kind.as_deref() == Some(builds::KIND) && self.built(t.id)) {
            out.push(run("🚪 go in", "enter"));
        }
        if self.lighthouse_here(x, y) && !self.lighthouse_lit() {
            out.push(run("🔧 mend", "mend"));
        }
        if self.dock_here(x, y) {
            out.push(run("⛵ sail", "sail"));
        }
        if self.meeting_here(id) {
            out.push(run("🔗 meet", "meet"));
        }

        // The ground.
        if self.finds.iter().any(|f| (f.x, f.y) == (x, y)) {
            out.push(run("🧺 gather", "gather"));
        }
        if room_of(x, y).is_none() {
            if self.water_near(x, y).is_some() {
                out.push(run("🎣 fish", "fish"));
            }
            if !matches!(self.world.terrain(x, y), Terrain::Water | Terrain::Pond | Terrain::DeepWater | Terrain::Stone) {
                out.push(fill("🌱 plant…", "plant "));
            }
        }
        out
    }
}
