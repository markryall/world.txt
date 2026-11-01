//! Visiting other worlds. Leaving: at a dock, 'sail to other.world' signs a
//! passport and sends the browser there. Arriving: a checked passport puts
//! the visitor in the waiting room until a keeper admits them, or straight
//! in if they've been admitted before (or their world is trusted).

use super::{ClientTx, Game};
use crate::protocol::{Col, Line, ToClient};
use crate::server::{accounts, federation, names};

impl Game {
    fn here_name(&self) -> String {
        self.url.as_deref().map_or_else(|| "this world".to_string(), federation::host)
    }

    pub(super) fn visit(&mut self, id: u64, p: federation::Passport, tx: ClientTx) {
        let now = accounts::now();
        let bye = |reason: String| {
            let _ = tx.send(ToClient::Bye { reason });
        };
        match self.world.db.use_passport(&p.nonce, p.expires, now) {
            Ok(true) => {}
            _ => return bye("🛂 That passport has already been used. Sail again from your home world!".into()),
        }
        let name = p.visitor_name();
        let home = federation::normalise(&p.home);
        match self.world.db.visitor_status(&name).as_deref() {
            Some("turned away") => {
                bye(format!("🛂 The keepers of {} aren't taking visitors from {} just now.", self.here_name(), federation::host(&home)))
            }
            Some("admitted") => self.admit_now(id, &p, tx),
            _ if self.world.db.is_trusted(&home) => self.admit_now(id, &p, tx),
            _ => {
                let _ = self.world.db.set_visitor(&name, &home, "waiting", now);
                let here = self.here_name();
                let mut lines = vec![
                    Line::new().b(Col::Blue, format!("⛵ Welcome to {here}, {}!", p.nickname)),
                    Line::col(Col::Blue, "🛂 You're waiting on the dock while the island keepers are asked to let you in. Hang on..."),
                ];
                if !self.players.values().any(|o| self.is_keeper(&o.user)) {
                    lines.push(Line::col(Col::Dim, "No keepers are here right now, so it might be a while. Once you're let in, you're in for good."));
                }
                let _ = tx.send(ToClient::Lines { lines });
                self.waiting.insert(id, (p, tx));
                self.tell_keepers_about_visitors();
            }
        }
    }

    /// Let a visitor in: a local account named user@home, and a device token
    /// so next time they walk straight in.
    fn admit_now(&mut self, id: u64, p: &federation::Passport, tx: ClientTx) {
        let name = p.visitor_name();
        let now = accounts::now();
        let made = if self.world.db.player(&name).is_none() {
            self.world.db.create_user(&name, &p.nickname, -1, -1)
        } else {
            self.world.db.set_nickname(&name, &p.nickname)
        };
        let Ok(token) = accounts::new_secret() else { return };
        if made.and_then(|_| self.world.db.set_visitor(&name, &federation::normalise(&p.home), "admitted", now)).and_then(|_| self.world.db.add_device(&name, &names::hash(&token), now)).is_err() {
            let _ = tx.send(ToClient::Bye { reason: "🛂 Something went wobbly at the dock. Try sailing again?".into() });
            return;
        }
        let hash = names::hash(&token);
        self.enter(id, name.clone(), false, Some(token), Some(hash), tx);
        self.say1(id, Line::col(Col::Blue, format!("🛂 You're a visitor here. When you're ready to go back to {}: go dock, then sail home.", federation::host(&p.home))));
    }

    /// Ask keepers online about everyone waiting at the docks.
    pub(super) fn tell_keepers_about_visitors(&self) {
        if self.waiting.is_empty() {
            return;
        }
        let lines: Vec<Line> = self
            .waiting
            .values()
            .map(|(p, _)| {
                let name = p.visitor_name();
                Line::new()
                    .c(Col::Yellow, format!("🛂 {} ({name}) has sailed in from {} and would like to visit: ", p.nickname, federation::host(&p.home)))
                    .b(Col::Yellow, format!("admit {name}"))
                    .c(Col::Dim, format!(" · turn away {name}"))
            })
            .collect();
        for o in self.players.values().filter(|o| self.is_keeper(&o.user)) {
            let _ = o.tx.send(ToClient::Lines { lines: lines.clone() });
        }
    }

    fn waiting_match(&self, who: &str) -> Option<u64> {
        let w = who.trim().to_lowercase();
        self.waiting
            .iter()
            .find(|(_, (p, _))| p.visitor_name().to_lowercase() == w || p.nickname.to_lowercase() == w)
            .map(|(&id, _)| id)
    }

    pub(super) fn cmd_admit(&mut self, id: u64, who: &str) -> anyhow::Result<()> {
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("Only island keepers can let visitors in."));
            return Ok(());
        }
        if let Some(vid) = self.waiting_match(who) {
            let (p, tx) = self.waiting.remove(&vid).expect("just found");
            self.say1(id, Line::col(Col::Green, format!("🛂 You let {} ({}) in. Welcome them!", p.nickname, p.visitor_name())));
            self.admit_now(vid, &p, tx);
            return Ok(());
        }
        let name = who.trim().to_lowercase();
        let Some(home) = self.world.db.visitor_home(&name) else {
            self.say1(id, Line::plain(format!("Nobody called '{}' is waiting. ('visitors' lists them)", who.trim())));
            return Ok(());
        };
        self.world.db.set_visitor(&name, &home, "admitted", accounts::now())?;
        self.say1(id, Line::col(Col::Green, format!("🛂 {name} can come straight in next time they sail here.")));
        Ok(())
    }

    pub(super) fn cmd_turn_away(&mut self, id: u64, who: &str) -> anyhow::Result<()> {
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("Only island keepers can do that."));
            return Ok(());
        }
        let who = who.trim().trim_start_matches("away").trim();
        let (name, home) = match self.waiting_match(who) {
            Some(vid) => {
                let (p, tx) = self.waiting.remove(&vid).expect("just found");
                let _ = tx.send(ToClient::Bye {
                    reason: format!("🛂 The keepers of {} can't have visitors just now. Sail home safely! ⛵", self.here_name()),
                });
                (p.visitor_name(), federation::normalise(&p.home))
            }
            None => {
                let name = who.to_lowercase();
                let Some(home) = self.world.db.visitor_home(&name) else {
                    self.say1(id, Line::plain(format!("No visitor called '{who}'. ('visitors' lists them)")));
                    return Ok(());
                };
                (name, home)
            }
        };
        self.world.db.set_visitor(&name, &home, "turned away", accounts::now())?;
        self.send_off(&name, "🛂 The island keepers have asked you to sail home. ⛵");
        self.say1(id, Line::col(Col::Yellow, format!("🛂 {name} won't be let in. ('admit {name}' if you change your mind)")));
        Ok(())
    }

    pub(super) fn cmd_visitors(&mut self, id: u64) -> anyhow::Result<()> {
        let mut lines = vec![Line::new().b(Col::Magenta, "🛂 Visitors from other worlds")];
        for (p, _) in self.waiting.values() {
            lines.push(Line::new().t("  ").b(Col::Yellow, p.nickname.clone()).c(Col::Dim, format!(" {}", p.visitor_name())).c(Col::Yellow, "  waiting at the dock now"));
        }
        for (name, home, status) in self.world.db.visitors()? {
            if self.waiting.values().any(|(p, _)| p.visitor_name() == name) {
                continue;
            }
            let here = if self.players.values().any(|o| o.user == name) { "  ☺ here" } else { "" };
            lines.push(
                Line::new()
                    .t("  ")
                    .b(Col::Cyan, self.world.nick(&name))
                    .c(Col::Dim, format!(" {name} · from {} · {status}", federation::host(&home)))
                    .c(Col::Green, here),
            );
        }
        if lines.len() == 1 {
            lines.push(Line::col(Col::Dim, "  Nobody has sailed in from another world yet."));
        }
        self.send(id, lines);
        Ok(())
    }

    /// At a dock: `sail to other.world` (another world) or `sail home` (a visitor going back).
    /// Returns false if `dest` isn't about other worlds at all.
    pub(super) fn sail_between_worlds(&mut self, id: u64, dest: &str) -> anyhow::Result<bool> {
        let user = self.user(id);
        let dest = dest.trim();
        if dest.eq_ignore_ascii_case("home") && user.contains('@') {
            let home = self.world.db.visitor_home(&user).unwrap_or_default();
            if home.is_empty() {
                self.say1(id, Line::plain("You can't remember the way home! Ask a keeper."));
                return Ok(true);
            }
            self.say1(id, Line::col(Col::Blue, format!("⛵ You row out for home, to {}...", federation::host(&home))));
            if let Some(p) = self.players.get(&id) {
                let _ = p.tx.send(ToClient::Travel { url: home });
            }
            return Ok(true);
        }
        let looks_like_a_world = dest.contains('.') || dest.contains(':') || dest.starts_with("http");
        if !looks_like_a_world {
            return Ok(false);
        }
        let Some(me) = self.url.clone() else {
            self.say1(id, Line::plain("This world doesn't have an address yet (serve --url), so it can't send anyone to other worlds."));
            return Ok(true);
        };
        if user.contains('@') {
            self.say1(id, Line::plain("You're a visitor here: sail home first, and set off from your own world."));
            return Ok(true);
        }
        let dest = federation::normalise(if dest.contains("://") { dest.to_string() } else { format!("https://{dest}") }.as_str());
        if dest == federation::normalise(&me) {
            self.say1(id, Line::plain("You're already here! 💅"));
            return Ok(true);
        }
        let passport = federation::Passport {
            user: user.clone(),
            nickname: self.world.nick(&user),
            home: federation::normalise(&me),
            dest: dest.clone(),
            expires: accounts::now() + federation::PASSPORT_SECS,
            nonce: accounts::new_secret()?,
        };
        let token = passport.sign(&federation::world_key(&self.world.db)?)?;
        self.say1(id, Line::col(Col::Blue, format!("⛵ You stamp your passport and row out to sea, towards {}...", federation::host(&dest))));
        if let Some(p) = self.players.get(&id) {
            let _ = p.tx.send(ToClient::Travel { url: format!("{dest}/#passport={token}") });
        }
        Ok(true)
    }
}
