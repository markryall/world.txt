//! Little games for whoever's standing together. Someone starts one and
//! everyone at the spot chooses in secret; when they all have, every choice
//! comes out at once.
//!
//! - `rps` (or `rock paper scissors`): rock, paper or scissors. Not in the
//!   help: the scenery hints at it when there's company.
//! - `ask How many ducks are on the pond?`: everyone types `answer …`, and
//!   the answers tumble out of a hat together, so nobody's swayed by anyone
//!   else's. Numbers come out in order.
//!
//! Anyone who turns up partway can join in just by choosing. Someone who
//! wanders off is out of it, and `reveal` shows what's in so far if someone
//! has stepped away. A game nobody finishes is forgotten after a while.

use super::Game;
use crate::protocol::{Col, Line};
use std::time::{Duration, Instant};

/// A game nobody finished is forgotten after this long.
const STALE: Duration = Duration::from_secs(10 * 60);
/// The longest question, and the longest answer.
const QUESTION_MAX: usize = 200;
const ANSWER_MAX: usize = 60;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Throw {
    Rock,
    Paper,
    Scissors,
}

impl Throw {
    fn parse(word: &str) -> Option<Throw> {
        match word.trim().trim_end_matches(['!', '.']).to_lowercase().as_str() {
            "rock" | "stone" | "🪨" | "✊" => Some(Throw::Rock),
            "paper" | "📄" | "✋" => Some(Throw::Paper),
            "scissors" | "scissor" | "✂️" | "✂" | "✌️" | "✌" => Some(Throw::Scissors),
            _ => None,
        }
    }

    fn beats(self, other: Throw) -> bool {
        matches!((self, other), (Throw::Rock, Throw::Scissors) | (Throw::Paper, Throw::Rock) | (Throw::Scissors, Throw::Paper))
    }

    fn shown(self) -> &'static str {
        match self {
            Throw::Rock => "🪨 rock",
            Throw::Paper => "📄 paper",
            Throw::Scissors => "✂️ scissors",
        }
    }
}

pub enum Kind {
    Rps,
    Ask(String),
}

/// A game going on at one spot.
pub struct Round {
    kind: Kind,
    started: Instant,
    /// Who's playing, by connection, in the order they joined, with what
    /// they've chosen so far.
    players: Vec<(u64, String, Option<String>)>,
}

impl Round {
    fn done(&self) -> bool {
        self.players.len() >= 2 && self.players.iter().all(|p| p.2.is_some())
    }
}

/// Who won a round of rock, paper, scissors: none when it's a draw (all the
/// same, or all three thrown).
pub fn winners(throws: &[Throw]) -> Vec<usize> {
    let kinds: Vec<Throw> = [Throw::Rock, Throw::Paper, Throw::Scissors].into_iter().filter(|t| throws.contains(t)).collect();
    if kinds.len() != 2 {
        return vec![];
    }
    let best = if kinds[0].beats(kinds[1]) { kinds[0] } else { kinds[1] };
    throws.iter().enumerate().filter(|(_, t)| **t == best).map(|(i, _)| i).collect()
}

/// Answers in the order to show them: numbers smallest first when they're
/// all numbers, otherwise as they came.
pub fn in_order(mut answers: Vec<(String, String)>) -> Vec<(String, String)> {
    let num = |a: &str| a.trim().trim_end_matches('%').parse::<f64>().ok();
    if answers.iter().all(|(_, a)| num(a).is_some()) {
        answers.sort_by(|x, y| num(&x.1).partial_cmp(&num(&y.1)).unwrap_or(std::cmp::Ordering::Equal));
    }
    answers
}

fn and_list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

impl Game {
    /// The game commands, if this is one.
    pub(super) fn games_command(&mut self, id: u64, verb: &str, rest: &str) -> Option<anyhow::Result<()>> {
        let r = rest.to_lowercase();
        let rps = |s: &str| s.contains("rock") && s.contains("paper") && s.contains("scissors");
        let whole = format!("{verb} {r}");
        Some(match verb {
            "rps" | "roshambo" | "rochambeau" => self.start_game(id, Kind::Rps),
            "play" | "start" | "game" if rps(&r) || r.trim() == "rps" => self.start_game(id, Kind::Rps),
            "rock" | "paper" | "scissors" if rps(&whole) => self.start_game(id, Kind::Rps),
            "ask" => self.cmd_ask(id, rest),
            "rock" | "paper" | "scissors" | "stone" if r.is_empty() && self.round_here(id).is_some_and(|g| matches!(g.kind, Kind::Rps)) => {
                self.choose(id, verb)
            }
            "answer" | "choose" | "guess" | "vote" if self.round_here(id).is_some() && !r.is_empty() => self.choose(id, rest),
            "answer" if !r.is_empty() => {
                self.say1(id, Line::plain("Nobody's asked anything here. Ask everyone here something with: ask How many ducks are on the pond?"));
                Ok(())
            }
            "reveal" | "shoot" if self.round_here(id).is_some() => self.reveal(id),
            _ => return None,
        })
    }

    /// The game going on where `id` is standing, if there's a fresh one.
    fn round_here(&self, id: u64) -> Option<&Round> {
        let p = self.players.get(&id)?;
        self.rounds.get(&(p.x, p.y)).filter(|g| g.started.elapsed() < STALE)
    }

    fn cmd_ask(&mut self, id: u64, question: &str) -> anyhow::Result<()> {
        let question = question.trim();
        if question.is_empty() {
            self.say1(id, Line::plain("Ask what? Everyone here answers in secret, and all the answers come out together. For example: ask How many ducks are on the pond?"));
            return Ok(());
        }
        let question: String = question.chars().take(QUESTION_MAX).collect();
        self.start_game(id, Kind::Ask(question))
    }

    fn start_game(&mut self, id: u64, kind: Kind) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        if let Some(g) = self.round_here(id) {
            let line = match &g.kind {
                Kind::Rps => "There's a game of rock, paper, scissors going on here already: rock, paper or scissors?".to_string(),
                Kind::Ask(q) => format!("There's a question in the hat here already: \"{q}\" (answer …, or reveal to see what's in)"),
            };
            self.say1(id, Line::plain(line));
            return Ok(());
        }
        let mut players: Vec<(u64, String, Option<String>)> = vec![(id, name.clone(), None)];
        players.extend(self.players.iter().filter(|(pid, p)| **pid != id && (p.x, p.y) == (x, y) && !p.bot).map(|(pid, p)| (*pid, p.name.clone(), None)));
        if players.len() < 2 {
            let line = match kind {
                Kind::Rps => "You hold out a fist, but there's nobody here to play with. It takes two!",
                Kind::Ask(_) => "There's nobody here to ask. Find some company first!",
            };
            self.say1(id, Line::plain(line));
            return Ok(());
        }
        let others: Vec<String> = players[1..].iter().map(|p| p.1.clone()).collect();
        let (mine, theirs) = match &kind {
            Kind::Rps => (
                format!("✊ You start a game of rock, paper, scissors with {}. Choose: rock, paper or scissors.", and_list(&others)),
                format!("✊ {name} starts a game of rock, paper, scissors! Choose: rock, paper or scissors."),
            ),
            Kind::Ask(q) => (
                format!("🎩 You drop a question into a hat: \"{q}\" Everyone here answers in secret (answer …), and the answers all come out together."),
                format!("🎩 {name} drops a question into a hat: \"{q}\" Answer in secret (answer …): they all come out together once everyone has."),
            ),
        };
        let colour = if matches!(kind, Kind::Rps) { Col::Yellow } else { Col::Magenta };
        self.say1(id, Line::col(colour, mine));
        self.tell_here(x, y, id, Line::col(colour, theirs));
        self.rounds.insert((x, y), Round { kind, started: Instant::now(), players });
        Ok(())
    }

    fn choose(&mut self, id: u64, choice: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let Some(g) = self.rounds.get_mut(&(x, y)) else { return Ok(()) };
        let choice = match &g.kind {
            Kind::Rps => match Throw::parse(choice) {
                Some(t) => format!("{t:?}").to_lowercase(),
                None => {
                    self.say1(id, Line::plain("It's rock, paper or scissors, I'm afraid."));
                    return Ok(());
                }
            },
            Kind::Ask(_) => choice.trim().chars().take(ANSWER_MAX).collect(),
        };
        let changed = match g.players.iter_mut().find(|p| p.0 == id) {
            Some(p) => p.2.replace(choice.clone()).is_some(),
            None => {
                g.players.push((id, name.clone(), Some(choice.clone())));
                false
            }
        };
        let waiting: Vec<(u64, String)> = g.players.iter().filter(|p| p.2.is_none()).map(|p| (p.0, p.1.clone())).collect();
        let done = g.done();
        let (verb, has) = if matches!(g.kind, Kind::Ask(_)) { ("answer", "answered") } else { ("choose", "chosen") };
        let others: Vec<u64> = g.players.iter().map(|p| p.0).filter(|p| *p != id).collect();
        // "(waiting for you and Lu)", as it reads to whoever's being told.
        let still = |to: u64| {
            if waiting.is_empty() || done {
                return String::new();
            }
            let names: Vec<String> = waiting.iter().map(|(w, n)| if *w == to { "you".to_string() } else { n.clone() }).collect();
            format!(" (waiting for {})", and_list(&names))
        };
        let mine = if changed { format!("🤫 You change your mind: {choice}.{}", still(id)) } else { format!("🤫 You {verb} {choice}, and keep it to yourself.{}", still(id)) };
        self.say1(id, Line::col(Col::Dim, mine));
        if !done {
            if !changed {
                for o in others {
                    self.say1(o, Line::col(Col::Dim, format!("{name} has {has}.{}", still(o))));
                }
            }
            return Ok(());
        }
        self.show_round(x, y);
        Ok(())
    }

    fn reveal(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        if !self.rounds.get(&(x, y)).is_some_and(|g| g.players.iter().filter(|p| p.2.is_some()).count() >= 2) {
            self.say1(id, Line::plain("Not yet: at least two people need to have chosen first."));
            return Ok(());
        }
        self.show_round(x, y);
        Ok(())
    }

    /// Everything that's been chosen, to everyone at the spot, and the game's over.
    fn show_round(&mut self, x: i64, y: i64) {
        let Some(g) = self.rounds.remove(&(x, y)) else { return };
        let chosen: Vec<(String, String)> = g.players.iter().filter_map(|p| p.2.clone().map(|c| (p.1.clone(), c))).collect();
        let missing: Vec<String> = g.players.iter().filter(|p| p.2.is_none()).map(|p| p.1.clone()).collect();
        let mut lines = Vec::new();
        match &g.kind {
            Kind::Rps => {
                let throws: Vec<Throw> = chosen.iter().filter_map(|c| Throw::parse(&c.1)).collect();
                lines.push(Line::new().b(Col::Yellow, "✊ Rock… paper… scissors… shoot!"));
                let shown: Vec<String> = chosen.iter().zip(&throws).map(|((who, _), t)| format!("{who} {}", t.shown())).collect();
                lines.push(Line::col(Col::Yellow, format!("   {}", shown.join(" · "))));
                let won: Vec<String> = winners(&throws).into_iter().map(|i| chosen[i].0.clone()).collect();
                let result = match won.len() {
                    0 if throws.windows(2).all(|w| w[0] == w[1]) => "Great minds! It's a draw. (rps for another go)".to_string(),
                    0 => "One of everything: it's a draw. (rps for another go)".to_string(),
                    1 => format!("{} wins! 🎉", won[0]),
                    _ => format!("{} win! 🎉", and_list(&won)),
                };
                lines.push(Line::col(Col::Yellow, format!("   {result}")));
            }
            Kind::Ask(q) => {
                lines.push(Line::new().b(Col::Magenta, format!("🎩 Out of the hat, for \"{q}\":")));
                let answers = in_order(chosen);
                let same = answers.windows(2).all(|w| w[0].1.eq_ignore_ascii_case(&w[1].1));
                for (who, a) in &answers {
                    lines.push(Line::new().t("   ").c(Col::Magenta, format!("{a}")).c(Col::Dim, format!("  {who}")));
                }
                if same && answers.len() > 1 {
                    lines.push(Line::col(Col::Magenta, "   Everyone agrees! ✨"));
                }
            }
        }
        if !missing.is_empty() {
            lines.push(Line::col(Col::Dim, format!("   ({} didn't choose)", and_list(&missing))));
        }
        for p in self.players.values() {
            if (p.x, p.y) == (x, y) {
                let _ = p.tx.send(crate::protocol::ToClient::Lines { lines: lines.clone() });
            }
        }
    }

    /// `id` is leaving their spot (walking, sailing, or going to sleep): they're
    /// out of any game there, which might mean everyone left has now chosen.
    pub(super) fn games_walks(&mut self, id: u64) {
        let Some((x, y, name)) = self.here(id) else { return };
        let Some(g) = self.rounds.get_mut(&(x, y)) else { return };
        if g.started.elapsed() >= STALE {
            self.rounds.remove(&(x, y));
            return;
        }
        let before = g.players.len();
        g.players.retain(|p| p.0 != id);
        if g.players.len() == before {
            return;
        }
        if g.done() {
            self.show_round(x, y);
        } else if g.players.len() < 2 {
            let left: Vec<u64> = g.players.iter().map(|p| p.0).collect();
            self.rounds.remove(&(x, y));
            for o in left {
                self.say1(o, Line::col(Col::Dim, format!("{name} has wandered off, so the game fizzles out.")));
            }
        }
    }

    /// For `look`: a game going on here.
    pub(super) fn games_lines_here(&self, id: u64) -> Vec<Line> {
        let Some(g) = self.round_here(id) else { return vec![] };
        let names: Vec<String> = g.players.iter().map(|p| p.1.clone()).collect();
        let line = match &g.kind {
            Kind::Rps => format!("✊ {} are in the middle of rock, paper, scissors. (rock, paper or scissors to join in)", and_list(&names)),
            Kind::Ask(q) => format!("🎩 There's a question in a hat here: \"{q}\" (answer … to join in)"),
        };
        vec![Line::col(Col::Dim, line)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Throw::*;

    #[test]
    fn rock_paper_scissors_has_winners_or_a_draw() {
        assert_eq!(winners(&[Rock, Scissors]), vec![0]);
        assert_eq!(winners(&[Rock, Paper]), vec![1]);
        assert_eq!(winners(&[Scissors, Paper, Scissors]), vec![0, 2]);
        assert_eq!(winners(&[Rock, Rock]), Vec::<usize>::new(), "all the same");
        assert_eq!(winners(&[Rock, Paper, Scissors]), Vec::<usize>::new(), "one of everything");
    }

    #[test]
    fn throws_can_be_typed_lots_of_ways() {
        assert_eq!(Throw::parse("Rock!"), Some(Rock));
        assert_eq!(Throw::parse("stone"), Some(Rock));
        assert_eq!(Throw::parse("✂️"), Some(Scissors));
        assert_eq!(Throw::parse("lizard"), None);
    }

    #[test]
    fn numbers_come_out_in_order() {
        let a = |v: &[(&str, &str)]| v.iter().map(|(w, x)| (w.to_string(), x.to_string())).collect::<Vec<_>>();
        assert_eq!(in_order(a(&[("Poppy", "8"), ("Sam", "3"), ("Lu", "5")])), a(&[("Sam", "3"), ("Lu", "5"), ("Poppy", "8")]));
        assert_eq!(in_order(a(&[("Poppy", "teal"), ("Sam", "3")])), a(&[("Poppy", "teal"), ("Sam", "3")]), "words stay as they came");
    }
}
