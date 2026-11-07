//! Telling the keepers things. `suggest …` leaves an idea (or a muddle) for
//! the island's keepers, who read them with `suggestions`. Nothing anyone
//! says to anyone else is ever kept for keepers to read: only what someone
//! chooses to send them.

use super::Game;
use crate::protocol::{Col, Line};

/// The longest suggestion.
const MAX_FEEDBACK: usize = 280;
/// How many a keeper sees at once.
const SHOWN: usize = 20;

impl Game {
    /// `suggest I couldn't work out how to get things out of the jar`
    pub(super) fn cmd_suggest(&mut self, id: u64, text: &str) -> anyhow::Result<()> {
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_FEEDBACK).collect();
        if text.is_empty() {
            self.say1(id, Line::plain("Suggest what? Anything you'd like, or anything that muddled you: suggest a way to sit on benches"));
            return Ok(());
        }
        let Some(p) = self.players.get(&id) else { return Ok(()) };
        let (user, name) = (p.user.clone(), p.name.clone());
        self.world.db.add_feedback("suggestion", &user, None, &text, super::boards::now())?;
        self.say1(id, Line::col(Col::Pink, "💌 Thank you! The island's keepers will read it."));
        self.tell_keepers(id, Line::col(Col::Dim, format!("💌 {name} left a suggestion. (suggestions)")));
        Ok(())
    }

    /// `suggestions`: the latest, for keepers.
    pub(super) fn cmd_suggestions(&mut self, id: u64) -> anyhow::Result<()> {
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("Suggestions go to the island's keepers. Leave one with: suggest …"));
            return Ok(());
        }
        let rows = self.world.db.feedback("suggestion", SHOWN)?;
        let mut lines = vec![Line::new().b(Col::Magenta, "💌 Suggestions")];
        if rows.is_empty() {
            lines.push(Line::col(Col::Dim, "  None yet."));
        }
        for (by, _, text, at) in rows {
            lines.push(Line::new().c(Col::Dim, format!("  {} · ", super::boards::when(at))).b(Col::Cyan, self.world.nick(&by)).c(Col::Plain, format!(": {text}")));
        }
        self.send(id, lines);
        Ok(())
    }

    /// A line for every keeper in the world (but not `except`).
    pub(super) fn tell_keepers(&self, except: u64, line: Line) {
        for (&pid, p) in &self.players {
            if pid != except && !p.bot && self.is_keeper(&p.user) {
                let _ = p.tx.send(crate::protocol::ToClient::Lines { lines: vec![line.clone()] });
            }
        }
    }
}
