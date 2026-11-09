//! `dark mode` and `light mode`. Dark mode doesn't touch the colours: it
//! makes the island quietly wrong, for you alone (the lines themselves are
//! changed on the way out, in `server::dark`). Everyone else's fox stays a
//! fox. It's remembered, so it's still dark next time, until `light mode`.

use super::Game;
use crate::protocol::{Col, Line};

fn key(user: &str) -> String {
    format!("dark:{user}")
}

impl Game {
    pub(super) fn cursed_command(
        &mut self,
        id: u64,
        verb: &str,
        rest: &str,
    ) -> Option<anyhow::Result<()>> {
        let r = rest.trim().to_lowercase();
        let mode = matches!(r.as_str(), "mode" | "theme");
        let on = match verb {
            "darkmode" => true,
            "lightmode" => false,
            "dark" if mode => true,
            "dark" if r == "mode off" => false,
            "light" if mode => false,
            _ => return None,
        };
        Some(self.cmd_dark(id, on))
    }

    fn cmd_dark(&mut self, id: u64, on: bool) -> anyhow::Result<()> {
        let Some(p) = self.players.get(&id) else {
            return Ok(());
        };
        let user = p.user.clone();
        let was = p.tx.dark();
        let line = match (was, on) {
            (true, true) => Line::col(
                Col::Dim,
                "🌚 It's already dark mode. You'd noticed, surely. (light mode puts it all back)",
            ),
            (false, false) => Line::col(
                Col::Dim,
                "🌞 It's already light mode. The island's just as it should be. Isn't it?",
            ),
            (false, true) => Line::new()
                .b(Col::Magenta, "🌚 You switch to dark mode. ")
                .c(
                    Col::Magenta,
                    "The colours stay exactly the same. Everything else... doesn't.",
                )
                .c(Col::Dim, " (light mode puts it all back)"),
            (true, false) => Line::new().b(Col::Yellow, "🌞 You switch back to light mode. ").c(
                Col::Yellow,
                "Colour seeps back into everything, and the animals look like animals again. Probably.",
            ),
        };
        if !on {
            p.tx.set_dark(false);
        }
        self.say1(id, line);
        if on {
            p.tx.set_dark(true);
        }
        if was != on {
            self.world
                .db
                .set_meta(&key(&user), if on { "1" } else { "" })?;
        }
        Ok(())
    }

    /// Still in dark mode from last time?
    pub(super) fn restore_dark(&self, id: u64) {
        if let Some(p) = self.players.get(&id) {
            let dark = self.world.db.meta(&key(&p.user)).is_some_and(|v| v == "1");
            p.tx.set_dark(dark);
        }
    }

    pub(super) fn is_dark(&self, id: u64) -> bool {
        self.players.get(&id).is_some_and(|p| p.tx.dark())
    }
}
