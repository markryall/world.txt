//! Homes: a 3x3 patch of island that's yours. Anyone can visit; only you and
//! the friends you invite can build there. `home` takes you back from anywhere.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::world::{HOME_RADIUS, Home};

/// Commands that change a place, which a home's owner gets a say over.
pub const BUILDING: &[&str] = &[
    "dig", "fill", "plant", "raise", "pile", "path", "pave", "name", "call-this", "make", "build", "create",
    "place", "drop", "put",
];

impl Game {
    /// May this player change things where they're standing? Says why not.
    pub(super) fn may_build_here(&self, id: u64) -> bool {
        let Some(p) = self.players.get(&id) else { return false };
        match self.world.home_at(p.x, p.y) {
            Some(h) if !h.welcomes(&p.user) && !self.is_keeper(&p.user) => {
                let owner = self.world.nick(&h.owner);
                self.say1(
                    id,
                    Line::plain(format!("This is {owner}'s home, so it's theirs to change. Ask them to share it with you! (@{owner} ...)")),
                );
                false
            }
            _ => true,
        }
    }

    pub(super) fn cmd_claim(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        if self.players.get(&id).is_some_and(|p| p.bot) {
            return Ok(());
        }
        if self.world.terrain(x, y).is_wet() {
            self.say1(id, Line::plain("You can't build a home on water. Find dry land!"));
            return Ok(());
        }
        let (sx, sy) = self.world.spawn_point();
        if (x - sx).abs() <= HOME_RADIUS + 1 && (y - sy).abs() <= HOME_RADIUS + 1 {
            self.say1(id, Line::plain("This spot is where newcomers arrive, so it stays open to everyone. Try a little further away."));
            return Ok(());
        }
        let gap = 2 * HOME_RADIUS;
        if let Some(h) = self.world.homes.iter().find(|h| {
            h.owner != user && (h.x - x).abs() <= gap && (h.y - y).abs() <= gap
        }) {
            self.say1(id, Line::plain(format!("That's too close to {}'s home. Try a few steps away.", self.world.nick(&h.owner))));
            return Ok(());
        }
        let moved = self.world.homes.iter().any(|h| h.owner == user);
        self.world.db.set_home(&user, x, y)?;
        match self.world.homes.iter_mut().find(|h| h.owner == user) {
            Some(h) => {
                h.x = x;
                h.y = y;
            }
            None => self.world.homes.push(Home { owner: user.clone(), x, y, guests: vec![] }),
        }
        let msg = if moved { "🏡 You move your home here." } else { "🏡 This is your home now! The patch around you is yours to decorate." };
        self.say1(id, Line::new().b(Col::Magenta, msg));
        self.say1(id, Line::col(Col::Dim, "Type home to come back from anywhere · share it with a friend to build together · name it something special"));
        self.tell_here(x, y, id, Line::col(Col::Magenta, format!("🏡 {name} makes a home here.")));
        self.status_all();
        Ok(())
    }

    pub(super) fn cmd_home(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let Some((hx, hy)) = self.world.homes.iter().find(|h| h.owner == user).map(|h| (h.x, h.y)) else {
            self.say1(id, Line::plain("You don't have a home yet. Find a spot you love and type claim!"));
            return Ok(());
        };
        if (x, y) == (hx, hy) {
            self.say1(id, Line::plain("You're already home. 🏡"));
            return Ok(());
        }
        self.tell_here(x, y, id, Line::col(Col::Cyan, format!("{name} heads off home.")));
        if let Some(p) = self.players.get_mut(&id) {
            p.x = hx;
            p.y = hy;
            p.fishing = None;
            p.sky = self.world.weather.sky_at(hx, hy);
        }
        self.say1(id, Line::col(Col::Magenta, "🏡 You wander home along the paths you know by heart."));
        self.tell_here(hx, hy, id, Line::col(Col::Cyan, format!("{name} comes home.")));
        self.look(id);
        self.save_progress(id);
        self.status_near(id, &[(x, y), (hx, hy)]);
        Ok(())
    }

    pub(super) fn cmd_invite(&mut self, id: u64, who: &str, on: bool) -> anyhow::Result<()> {
        let Some((_, _, name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let guest = who.trim();
        if guest.is_empty() {
            self.say1(id, Line::plain(if on { "Share your home with who? e.g. 'share Lu'" } else { "Stop sharing with who?" }));
            return Ok(());
        }
        let Some(_home) = self.world.homes.iter_mut().find(|h| h.owner == user) else {
            self.say1(id, Line::plain("You need a home first. Type claim where you'd like it."));
            return Ok(());
        };
        let Some(guest_user) = self.world.db.resolve_user(guest) else {
            self.say1(id, Line::plain(format!("There's nobody called {guest} on the island.")));
            return Ok(());
        };
        let guest = crate::server::names::full(&self.world.nick(&guest_user), &guest_user);
        let Some(home) = self.world.homes.iter_mut().find(|h| h.owner == user) else { return Ok(()) };
        if on {
            if !home.guests.contains(&guest_user) {
                home.guests.push(guest_user.clone());
            }
        } else {
            home.guests.retain(|g| *g != guest_user);
        }
        self.world.db.set_guest(&user, &guest_user, on)?;
        if on {
            self.say1(id, Line::col(Col::Magenta, format!("💌 {guest} can build at your home now.")));
            if let Some(gid) = self.find_player(&guest_user) {
                self.say1(gid, Line::col(Col::Magenta, format!("💌 {name} shared their home with you: you can build there now! (find {name} to visit)")));
            }
        } else {
            self.say1(id, Line::col(Col::Dim, format!("{guest} can still visit, but can't build at your home any more.")));
        }
        Ok(())
    }

    pub(super) fn cmd_guests(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((_, _, _name)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let msg = match self.world.homes.iter().find(|h| h.owner == user) {
            None => "You don't have a home yet. Type claim where you'd like it.".to_string(),
            Some(h) if h.guests.is_empty() => "Nobody else builds at your home yet. 'share Lu' to build together.".to_string(),
            Some(h) => format!(
                "💌 Invited to build at your home: {}",
                h.guests.iter().map(|g| crate::server::names::full(&self.world.nick(g), g)).collect::<Vec<_>>().join(", ")
            ),
        };
        self.say1(id, Line::col(Col::Dim, msg));
        Ok(())
    }
}
