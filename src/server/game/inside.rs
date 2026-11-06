//! Going inside: every finished building has a room. `go in` (or `enter`)
//! from beside it, `out` (or any direction) to leave. Rooms are places like
//! any other (people talk, make, place, paint and put things in cases there),
//! just indoors: no weather, and nothing that needs open ground. Whoever may
//! build at the home the building's in (or its builder, elsewhere) may change
//! the room, including `describe`-ing it in their own words.
//!
//! A room lives off the map (see `world::room_for`), so anything that goes by
//! where you're standing keeps working inside without knowing about rooms.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::world::{room_for, room_of};

/// Things that need the great outdoors.
const OUTDOOR: &[&str] = &[
    "dig", "fill", "plant", "raise", "pile", "path", "pave", "claim", "fish", "sail", "row", "boat", "kite", "fly", "reel",
    "untangle", "skim", "jump", "splash", "leap", "stomp", "puddle", "bottle", "throw", "name", "call-this", "roll", "lookout",
    "survey", "picnic", "stargaze", "umbrella",
];

impl Game {
    pub(super) fn in_room(&self, id: u64) -> bool {
        self.players.get(&id).is_some_and(|p| room_of(p.x, p.y).is_some())
    }

    /// Inside, the outdoor things say so rather than half-working.
    pub(super) fn indoors_says_no(&self, id: u64, verb: &str, rest: &str) -> bool {
        if !self.in_room(id) {
            return false;
        }
        let r = rest.to_lowercase();
        let snowy = matches!(verb, "make" | "build" | "create") && (r.contains("snow") || r.contains("sandcastle"));
        if OUTDOOR.contains(&verb) || snowy {
            self.say1(id, Line::plain("That's one for outdoors. (out to go outside)"));
            return true;
        }
        false
    }

    /// `go in`, `enter`, `enter the cottage`: into a finished building here.
    pub(super) fn cmd_enter(&mut self, id: u64, which: &str) -> anyhow::Result<()> {
        if self.in_room(id) {
            self.say1(id, Line::plain("You're already inside. (out to go outside)"));
            return Ok(());
        }
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let which = crate::server::world::strip_article(which.trim()).to_lowercase();
        let buildings: Vec<(i64, String)> = self
            .world
            .things_at(x, y)
            .filter(|t| t.kind.as_deref() == Some(super::builds::KIND))
            .filter(|t| which.is_empty() || t.name.to_lowercase().contains(&which))
            .map(|t| (t.id, t.name.clone()))
            .collect();
        let Some((b, name)) = buildings.iter().find(|(b, _)| self.built(*b)).cloned() else {
            let line = if buildings.is_empty() { "There's nothing here to go into." } else { "It's still going up. You can go in once it's finished." };
            self.say1(id, Line::plain(line));
            return Ok(());
        };
        let room = room_for(b, x, y);
        let the = super::the(&name);
        self.stop_travelling(id, false);
        self.kite_walks(id);
        self.games_walks(id);
        self.tell_here(x, y, id, Line::col(Col::Cyan, format!("{who} goes into {the}.")));
        self.move_player(id, room);
        self.tell_here(room.0, room.1, id, Line::col(Col::Cyan, format!("{who} comes in.")));
        self.look(id);
        self.save_progress(id);
        self.status_near(id, &[(x, y), room]);
        Ok(())
    }

    /// `out`, or any direction, from inside: back out beside the building.
    pub(super) fn cmd_go_out(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        if room_of(x, y).is_none() {
            self.say1(id, Line::plain("You're already out in the fresh air."));
            return Ok(());
        }
        let name = self.world.room_building(x, y).map_or("the building".to_string(), |b| super::the(&b.name));
        let outside = self.world.outside(x, y);
        self.games_walks(id);
        self.tell_here(x, y, id, Line::col(Col::Cyan, format!("{who} heads outside.")));
        self.move_player(id, outside);
        self.tell_here(outside.0, outside.1, id, Line::col(Col::Cyan, format!("{who} comes out of {name}.")));
        self.look(id);
        self.save_progress(id);
        self.status_near(id, &[(x, y), outside]);
        Ok(())
    }

    fn move_player(&mut self, id: u64, to: (i64, i64)) {
        let sky = self.world.weather.sky_at(to.0, to.1);
        if let Some(p) = self.players.get_mut(&id) {
            p.x = to.0;
            p.y = to.1;
            p.sky = sky;
            p.fishing = None;
        }
    }

    /// A building's coming down: whoever's inside steps out first.
    pub(super) fn empty_room(&mut self, building: i64, bx: i64, by: i64) {
        let room = room_for(building, bx, by);
        let inside: Vec<u64> = self.players.iter().filter(|(_, p)| (p.x, p.y) == room).map(|(id, _)| *id).collect();
        for id in inside {
            self.say1(id, Line::col(Col::Cyan, "The walls start coming down around you, so you step outside."));
            self.move_player(id, (bx, by));
            self.look(id);
            self.save_progress(id);
        }
    }
}
