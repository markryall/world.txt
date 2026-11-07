//! Telling the keepers things. `suggest …` leaves an idea (or a muddle) for
//! the island's keepers, who read them with `suggestions`. `report Sam …` is
//! for when someone's being unkind: the keepers see the report, in the
//! reporter's own words, with `reports`. Nothing anyone says to anyone else is
//! ever kept for keepers to read: only what someone chooses to send them.

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

    /// `report Sam keeps sending me horrible messages`
    pub(super) fn cmd_report(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let (who, text) = rest.trim().split_once(char::is_whitespace).map_or((rest.trim(), ""), |(w, t)| (w, t.trim()));
        let who = who.trim_start_matches('@');
        if who.is_empty() {
            self.say1(id, Line::plain("Report who? Tell the keepers what's happened, in your own words: report Sam keeps saying unkind things"));
            return Ok(());
        }
        let about = self.find_player(who).map(|pid| self.players[&pid].user.clone()).or_else(|| self.world.db.resolve_user(who));
        let Some(about) = about else {
            self.say1(id, Line::plain(format!("There's nobody called {who} on the island.")));
            return Ok(());
        };
        let user = self.user(id);
        if about.eq_ignore_ascii_case(&user) {
            self.say1(id, Line::plain("You can't report yourself, silly. If something's wrong, suggest … tells the keepers."));
            return Ok(());
        }
        let them = self.world.nick(&about);
        let text: String = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_FEEDBACK).collect();
        if text.is_empty() {
            self.say1(id, Line::plain(format!("What's been happening with {them}? The keepers will only see what you write here: report {them} …")));
            return Ok(());
        }
        let name = self.players.get(&id).map_or(user.clone(), |p| p.name.clone());
        self.world.db.add_feedback("report", &user, Some(&about), &text, super::boards::now())?;
        self.say1(id, Line::col(Col::Pink, format!("🚩 Thank you for telling us. Only the island's keepers will see it. To stop hearing from {them} meanwhile: mute {them} (they won't know).")));
        self.tell_keepers(id, Line::col(Col::Yellow, format!("🚩 {name} has reported {them}. (reports)")));
        Ok(())
    }

    /// `reports`: the latest, for keepers.
    pub(super) fn cmd_reports(&mut self, id: u64) -> anyhow::Result<()> {
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("Reports go to the island's keepers. If someone's being unkind: report Sam …"));
            return Ok(());
        }
        let rows = self.world.db.feedback("report", SHOWN)?;
        let mut lines = vec![Line::new().b(Col::Magenta, "🚩 Reports")];
        if rows.is_empty() {
            lines.push(Line::col(Col::Dim, "  None. Lovely."));
        }
        for (by, about, text, at) in rows {
            let about = about.map(|a| self.world.nick(&a)).unwrap_or_default();
            lines.push(
                Line::new()
                    .c(Col::Dim, format!("  {} · ", super::boards::when(at)))
                    .b(Col::Cyan, self.world.nick(&by))
                    .c(Col::Dim, " about ")
                    .b(Col::Cyan, about)
                    .c(Col::Plain, format!(": {text}")),
            );
        }
        lines.push(Line::col(Col::Dim, "  Only what the reporter wrote is kept. kick, ban and signout are there if they're needed."));
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
