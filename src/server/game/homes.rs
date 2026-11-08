//! Homes: a 3x3 patch of island that's yours. Anyone can visit; only you and
//! the friends you invite can build there. `home` takes you back from anywhere.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::paint;
use crate::server::weather::{Clock, Phase};
use std::time::Duration;

/// An owner hears about the same visitor at most this often.
const DOORBELL_EVERY: Duration = Duration::from_secs(30 * 60);
use crate::server::world::{HOME_RADIUS, Home};

/// "home", "my house": painting your own home, which works from anywhere.
pub fn means_home(what: &str) -> bool {
    ["home", "house", "place", "garden"].contains(&what.trim().to_lowercase().as_str())
}

/// `paint home lilac` (and the like) changes your own home, not the place
/// you're standing, so where you are doesn't matter.
pub fn paints_own_home(verb: &str, rest: &str) -> bool {
    matches!(verb, "paint" | "repaint" | "colour" | "color")
        && paint::parse(rest).is_some_and(|(what, _)| means_home(&what))
}

/// Commands that change a place, which a home's owner gets a say over.
pub const BUILDING: &[&str] = &[
    "dig",
    "fill",
    "plant",
    "raise",
    "pile",
    "path",
    "pave",
    "name",
    "call-this",
    "make",
    "build",
    "create",
    "construct",
    "place",
    "drop",
    "put",
    "describe",
    "paint",
    "repaint",
    "colour",
    "color",
];

impl Game {
    /// May this player change things where they're standing? Says why not.
    pub(super) fn may_build_here(&self, id: u64) -> bool {
        let Some(p) = self.players.get(&id) else {
            return false;
        };
        // Inside a building: the home it's at decides, or else its builder.
        if let Some(b) = self.world.room_building(p.x, p.y) {
            let ok = self
                .world
                .home_at(b.x, b.y)
                .map_or(b.maker.eq_ignore_ascii_case(&p.user), |h| {
                    h.welcomes(&p.user)
                })
                || self.is_keeper(&p.user);
            if !ok {
                let owner = self
                    .world
                    .home_at(b.x, b.y)
                    .map_or(b.maker.clone(), |h| h.owner.clone());
                let owner = self.world.nick(&owner);
                self.say1(id, Line::plain(format!("This is {owner}'s room, so it's theirs to change. You're welcome to look around!")));
            }
            return ok;
        }
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
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        if self.players.get(&id).is_some_and(|p| p.bot) {
            return Ok(());
        }
        if self.world.terrain(x, y).is_wet() {
            self.say1(
                id,
                Line::plain("You can't build a home on water. Find dry land!"),
            );
            return Ok(());
        }
        let (sx, sy) = self.world.spawn_point();
        if (x - sx).abs() <= HOME_RADIUS + 1 && (y - sy).abs() <= HOME_RADIUS + 1 {
            self.say1(id, Line::plain("This spot is where newcomers arrive, so it stays open to everyone. Try a little further away."));
            return Ok(());
        }
        let gap = 2 * HOME_RADIUS;
        if let Some(h) = self
            .world
            .homes
            .iter()
            .find(|h| h.owner != user && (h.x - x).abs() <= gap && (h.y - y).abs() <= gap)
        {
            self.say1(
                id,
                Line::plain(format!(
                    "That's too close to {}'s home. Try a few steps away.",
                    self.world.nick(&h.owner)
                )),
            );
            return Ok(());
        }
        let moved = self.world.homes.iter().any(|h| h.owner == user);
        self.world.db.set_home(&user, x, y)?;
        match self.world.homes.iter_mut().find(|h| h.owner == user) {
            Some(h) => {
                h.x = x;
                h.y = y;
            }
            None => self.world.homes.push(Home {
                owner: user.clone(),
                x,
                y,
                guests: vec![],
                paint: None,
            }),
        }
        let msg = if moved {
            "🏡 You move your home here."
        } else {
            "🏡 This is your home now! The patch around you is yours to decorate."
        };
        self.say1(id, Line::new().b(Col::Magenta, msg));
        self.happened(&user, super::since::HOME, x, y, "home");
        self.say1(id, Line::col(Col::Dim, "Type home to come back from anywhere · share it with a friend to build together · name it something special"));
        self.tell_here(
            x,
            y,
            id,
            Line::col(Col::Magenta, format!("🏡 {name} makes a home here.")),
        );
        self.status_all();
        Ok(())
    }

    pub(super) fn cmd_home(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let Some((hx, hy)) = self
            .world
            .homes
            .iter()
            .find(|h| h.owner == user)
            .map(|h| (h.x, h.y))
        else {
            self.say1(
                id,
                Line::plain("You don't have a home yet. Find a spot you love and type claim!"),
            );
            return Ok(());
        };
        if (x, y) == (hx, hy) {
            self.say1(id, Line::plain("You're already home. 🏡"));
            return Ok(());
        }
        // Walked, not whisked: there's the scenery on the way.
        self.tell_here(
            x,
            y,
            id,
            Line::col(Col::Cyan, format!("{name} heads off home.")),
        );
        if let Some(p) = self.players.get_mut(&id) {
            p.fishing = None;
        }
        self.set_off(id, hx, hy, "home", None)
    }

    pub(super) fn cmd_invite(&mut self, id: u64, who: &str, on: bool) -> anyhow::Result<()> {
        let Some((_, _, name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let guest = who.trim();
        if guest.is_empty() {
            self.say1(
                id,
                Line::plain(if on {
                    "Share your home with who? e.g. 'share Lu'"
                } else {
                    "Stop sharing with who?"
                }),
            );
            return Ok(());
        }
        let Some(_home) = self.world.homes.iter_mut().find(|h| h.owner == user) else {
            self.say1(
                id,
                Line::plain("You need a home first. Type claim where you'd like it."),
            );
            return Ok(());
        };
        let Some(guest_user) = self.world.db.resolve_user(guest) else {
            self.say1(
                id,
                Line::plain(format!("There's nobody called {guest} on the island.")),
            );
            return Ok(());
        };
        let guest = crate::server::names::full(&self.world.nick(&guest_user), &guest_user);
        let Some(home) = self.world.homes.iter_mut().find(|h| h.owner == user) else {
            return Ok(());
        };
        if on {
            if !home.guests.contains(&guest_user) {
                home.guests.push(guest_user.clone());
            }
        } else {
            home.guests.retain(|g| *g != guest_user);
        }
        self.world.db.set_guest(&user, &guest_user, on)?;
        if on {
            self.say1(
                id,
                Line::col(
                    Col::Magenta,
                    format!("💌 {guest} can build at your home now."),
                ),
            );
            if let Some(gid) = self.find_player(&guest_user) {
                self.say1(gid, Line::col(Col::Magenta, format!("💌 {name} shared their home with you: you can build there now! (find {name} to visit)")));
            }
        } else {
            self.say1(
                id,
                Line::col(
                    Col::Dim,
                    format!("{guest} can still visit, but can't build at your home any more."),
                ),
            );
        }
        Ok(())
    }

    pub(super) fn cmd_guests(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((_, _, _name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let msg = match self.world.homes.iter().find(|h| h.owner == user) {
            None => "You don't have a home yet. Type claim where you'd like it.".to_string(),
            Some(h) if h.guests.is_empty() => {
                "Nobody else builds at your home yet. 'share Lu' to build together.".to_string()
            }
            Some(h) => format!(
                "💌 Invited to build at your home: {}",
                h.guests
                    .iter()
                    .map(|g| crate::server::names::full(&self.world.nick(g), g))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        self.say1(id, Line::col(Col::Dim, msg));
        Ok(())
    }

    /// `paint the bench pink`, `paint home lilac`, `paint the bench plain`.
    /// Things are painted by whoever made them (or at their home, its
    /// owner); a home by its owner, from anywhere.
    pub(super) fn cmd_paint(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else {
            return Ok(());
        };
        let Some((what, word)) = paint::parse(rest) else {
            self.say1(id, Line::plain("Paint what, and what colour? For example: paint the bench pink, or paint home lilac."));
            return Ok(());
        };
        let user = self.user(id);
        let colour = (!paint::PLAIN.contains(&word.as_str())).then(|| word.clone());
        let thing = if what.is_empty() {
            None
        } else {
            self.world.find_thing(x, y, &what)
        };
        let Some(i) = thing else {
            if !(what.is_empty() || means_home(&what)) {
                self.say1(
                    id,
                    Line::plain(format!("You don't see any {what} here to paint.")),
                );
                return Ok(());
            }
            if !self.world.homes.iter().any(|h| h.owner == user) {
                let line = if what.is_empty() {
                    "Paint what? For example: paint the bench pink."
                } else {
                    "You don't have a home to paint yet. Find a spot you love and claim it!"
                };
                self.say1(id, Line::plain(line));
                return Ok(());
            }
            self.world.paint_home(&user, colour.clone())?;
            if let Some(c) = &colour {
                self.happened(&user, super::since::PAINT, x, y, c);
            }
            let at_home = self.world.home_at(x, y).is_some_and(|h| h.owner == user);
            match &colour {
                Some(c) => {
                    self.say1(
                        id,
                        paint::painted(
                            Line::plain("🎨 You paint your home "),
                            c,
                            Some(c),
                            Col::Plain,
                        )
                        .t(". It shows on the map now."),
                    );
                    if at_home {
                        self.tell_here(
                            x,
                            y,
                            id,
                            paint::painted(
                                Line::plain(format!("🎨 {who} paints their home ")),
                                c,
                                Some(c),
                                Col::Plain,
                            )
                            .t("."),
                        );
                    }
                }
                None => self.say1(id, Line::plain("🧽 You scrub the paint off your home.")),
            }
            self.status_all();
            return Ok(());
        };
        if self.world.things[i].is_fixed() {
            self.say1(
                id,
                Line::plain("That belongs to the island, just as it is."),
            );
            return Ok(());
        }
        let host = self.world.home_at(x, y).is_some_and(|h| h.owner == user);
        if !self.may_change(&user, i) && !host {
            let t = &self.world.things[i];
            let maker = self.world.nick(&t.maker);
            self.say1(
                id,
                Line::plain(format!(
                    "That's {maker}'s {}, so it's theirs to paint. (Ask them: @{maker} ...)",
                    crate::server::world::strip_article(&t.name)
                )),
            );
            return Ok(());
        }
        let name = super::the(&self.world.things[i].name);
        self.world.paint_thing(i, colour.clone())?;
        match &colour {
            Some(c) => {
                self.say1(
                    id,
                    paint::painted(
                        Line::plain(format!("🎨 You paint {name} ")),
                        c,
                        Some(c),
                        Col::Plain,
                    )
                    .t("."),
                );
                self.tell_here(
                    x,
                    y,
                    id,
                    paint::painted(
                        Line::plain(format!("🎨 {who} paints {name} ")),
                        c,
                        Some(c),
                        Col::Plain,
                    )
                    .t("."),
                );
            }
            None => {
                self.say1(
                    id,
                    Line::plain(format!("🧽 You scrub the paint off {name}.")),
                );
                self.tell_here(
                    x,
                    y,
                    id,
                    Line::plain(format!("🧽 {who} scrubs the paint off {name}.")),
                );
            }
        }
        self.status_all();
        Ok(())
    }

    /// For `look`: a home that's lived in. Windows glow at night while its
    /// owner's in the world, and a chimney smokes while something cooks.
    pub(super) fn home_life_lines(&self, id: u64) -> Vec<Line> {
        let Some(p) = self.players.get(&id) else {
            return vec![];
        };
        let Some(h) = self.world.home_at(p.x, p.y) else {
            return vec![];
        };
        let buildings: Vec<String> = self
            .world
            .things_at(p.x, p.y)
            .filter(|t| t.kind.as_deref() == Some(super::builds::KIND) && self.built(t.id))
            .map(|t| super::the(&t.name))
            .collect();
        let Some(building) = buildings.first() else {
            return vec![];
        };
        let mut lines = Vec::new();
        let owner_about = self.players.values().any(|o| o.user == h.owner);
        if owner_about && matches!(Clock::phase(), Phase::Dusk | Phase::Night) {
            lines.push(Line::col(
                Col::Yellow,
                format!("🪟 Warm light glows in the windows of {building}."),
            ));
        }
        let cooking = self.cooking.keys().any(|k| {
            self.world
                .things
                .iter()
                .find(|t| t.id == *k)
                .is_some_and(|t| {
                    self.world
                        .home_at(t.x, t.y)
                        .is_some_and(|kh| kh.owner == h.owner)
                })
        });
        if cooking {
            lines.push(Line::col(Col::Dim, format!("🌫 A curl of smoke rises from the chimney of {building}, smelling of something good.")));
        }
        lines
    }

    /// Someone's wandered into a home: if its owner is out and about
    /// elsewhere, they hear about it (now and then, not every step).
    pub(super) fn visit_home(&mut self, id: u64, from: (i64, i64), to: (i64, i64)) {
        let Some(h) = self.world.home_at(to.0, to.1) else {
            return;
        };
        if self
            .world
            .home_at(from.0, from.1)
            .is_some_and(|f| f.owner == h.owner)
        {
            return;
        }
        let Some(p) = self.players.get(&id) else {
            return;
        };
        if p.bot || h.welcomes(&p.user) {
            return;
        }
        let (visitor, user, owner) = (p.name.clone(), p.user.clone(), h.owner.clone());
        // Now and then, not every step.
        let key = (owner.clone(), user.clone());
        if self
            .doorbells
            .get(&key)
            .is_some_and(|t| t.elapsed() < DOORBELL_EVERY)
        {
            return;
        }
        self.doorbells.insert(key, std::time::Instant::now());
        // Kept for the recap, in case they're away (see docs/rules.md).
        self.happened(&user, super::since::VISIT, to.0, to.1, &owner);
        let Some((&oid, o)) = self.players.iter().find(|(_, o)| o.user == owner) else {
            return;
        };
        // At home, they'll see them arrive anyway.
        if self
            .world
            .home_at(o.x, o.y)
            .is_some_and(|oh| oh.owner == owner)
            || !o.hears(&user)
        {
            return;
        }
        self.say1(
            oid,
            Line::col(
                Col::Cyan,
                format!("🔔 {visitor} is visiting your home. (home to wander back)"),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn painting_your_own_home_from_anywhere() {
        assert!(
            paints_own_home("paint", "home lilac") && paints_own_home("paint", "my house sky blue")
        );
        assert!(!paints_own_home("paint", "the bench pink") && !paints_own_home("dig", "home"));
    }
}
