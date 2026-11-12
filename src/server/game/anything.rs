//! Whatever you type, something happens. The island has a fixed set of
//! words, but nobody should have to guess them: a near-miss of a word it
//! knows is taken as that word, a question gets a nudge towards where the
//! answer might be, chatter is said aloud, and anything else becomes
//! something you do ("dance a jig" → "Wren dances a jig."), with the island
//! reacting now and then. Rudeness gets a smarmy word back, for the rude
//! one's ears only, and goes no further.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::world::capitalise;

/// Words the island knows that are safe to guess at: a typo of one of these
/// is taken as meant. Nothing here can lose anyone anything.
const GUESSABLE: &[&str] = &[
    "look",
    "gather",
    "satchel",
    "journal",
    "recipes",
    "craft",
    "cook",
    "plant",
    "fish",
    "sail",
    "help",
    "map",
    "home",
    "read",
    "places",
    "befriend",
    "release",
    "inventory",
    "describe",
    "wave",
    "hum",
    "make",
    "shout",
    "enter",
    "dance",
];

/// The first words of chatter, which is better said aloud than done.
const CHATTY: &[&str] = &[
    "i",
    "i'm",
    "im",
    "ive",
    "i've",
    "id",
    "i'd",
    "we",
    "you",
    "u",
    "lol",
    "haha",
    "hahaha",
    "thanks",
    "thank",
    "ty",
    "yay",
    "yes",
    "yeah",
    "yep",
    "no",
    "nope",
    "ok",
    "okay",
    "hello",
    "hey",
    "hiya",
    "hi",
    "morning",
    "goodnight",
    "night",
    "sorry",
    "oops",
    "wow",
    "omg",
    "aww",
    "ooh",
    "oh",
    "cool",
    "nice",
    "lovely",
    "please",
    "bye",
    "cheers",
    "g'day",
    "gday",
];

/// The first words of a question.
const ASKING: &[&str] = &[
    "what", "where", "how", "why", "when", "which", "can", "could", "is", "are", "do", "does",
    "will", "should", "am",
];

/// The first words of a thing, rather than a doing: mused on, not done.
const THINGS: &[&str] = &[
    "the", "a", "an", "my", "this", "that", "these", "those", "it", "its", "it's", "there", "some",
    "your", "our", "their",
];

/// Strong words, as the start of any word ("fucking" and all).
const SWEARS: &[&str] = &[
    "fuck", "shit", "cunt", "bitch", "bastard", "wank", "twat", "arsehole", "asshole", "motherf",
    "bullshit",
];
/// Rude only as whole words ("ass" is rude, "assemble" isn't).
const RUDE_WORDS: &[&str] = &[
    "ass", "arse", "crap", "crappy", "dick", "prick", "piss", "pissed", "bollocks", "bugger",
    "stupid", "idiot", "idiots", "dumb", "sucks", "suck", "moron", "loser", "useless", "stfu",
    "wtf", "damn", "dammit", "fk", "fkn", "fking", "ffs", "shite", "sod", "frick", "fricking",
];

/// Fighting words, as the first word. Nobody gets hurt on the island: it
/// all turns into something soft instead.
const FIGHTY: &[&str] = &[
    "kill",
    "murder",
    "stab",
    "punch",
    "attack",
    "hit",
    "slap",
    "fight",
    "smack",
    "strangle",
    "bite",
    "slay",
    "maim",
    "assassinate",
    "throttle",
    "wallop",
    "thump",
    "whack",
    "beat",
    "bash",
    "headbutt",
    "choke",
    "poison",
    "behead",
    "decapitate",
    "execute",
    "shank",
    "nuke",
    "obliterate",
];

/// What someone typed, as far as the island can tell.
#[derive(Debug, PartialEq)]
enum Said {
    Rude,
    /// Wanting to hurt something, which comes out as something nice.
    Fighty,
    /// A near-miss of a word the island knows.
    Typo(&'static str),
    Question,
    Chatter,
    /// A thing, not a doing.
    Musing,
    Doing,
}

fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn rude(text: &str) -> bool {
    let ws = words(text);
    // A word with its middle starred out ("f***") is a swear, politely.
    let starred = text
        .split_whitespace()
        .any(|w| w.len() > 2 && w.contains('*') && w.starts_with(|c: char| c.is_alphabetic()));
    starred
        || ws
            .iter()
            .any(|w| SWEARS.iter().any(|s| w.starts_with(s)) || RUDE_WORDS.contains(&w.as_str()))
        || text.to_lowercase().contains("shut up")
}

/// Edits between two words, a swap of neighbours counting as one.
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            d[i][j] = (d[i - 1][j] + 1)
                .min(d[i][j - 1] + 1)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[a.len()][b.len()]
}

/// A word the island knows that this is surely a slip of: one letter out
/// (two, in a long word), and only if it can't be anything else.
fn typo(verb: &str) -> Option<&'static str> {
    if verb.chars().count() < 3 {
        return None;
    }
    let most = if verb.chars().count() >= 7 { 2 } else { 1 };
    let near: Vec<(&'static str, usize)> = GUESSABLE
        .iter()
        .map(|&g| (g, distance(verb, g)))
        // Never the word itself: a known word that ended up here (one only
        // some places answer to) is a doing, not a slip, so nothing loops.
        .filter(|&(_, d)| d > 0 && d <= most)
        .collect();
    let best = near.iter().map(|n| n.1).min()?;
    let mut at_best = near.iter().filter(|n| n.1 == best);
    let first = at_best.next()?;
    at_best.next().is_none().then_some(first.0)
}

fn said(text: &str) -> Said {
    let ws = words(text);
    let first = ws.first().map(String::as_str).unwrap_or("");
    if rude(text) {
        Said::Rude
    } else if FIGHTY.contains(&first) {
        Said::Fighty
    } else if text.trim_end().ends_with('?')
        || (ASKING.contains(&first) && !text.trim_end().ends_with('!'))
    {
        Said::Question
    } else if ASKING.contains(&first) {
        // "What a lovely day!": not a question, just glad.
        Said::Chatter
    } else if CHATTY.contains(&first) {
        Said::Chatter
    } else if THINGS.contains(&first) {
        Said::Musing
    } else if let Some(t) = typo(first) {
        Said::Typo(t)
    } else {
        Said::Doing
    }
}

/// "dance" → "dances", "wash" → "washes", "fly" → "flies".
fn third_person(verb: &str) -> String {
    let v = verb.to_lowercase();
    match v.as_str() {
        "be" => return "is".into(),
        "have" => return "has".into(),
        "do" => return "does".into(),
        "go" => return "goes".into(),
        _ => {}
    }
    let vowel = |c: char| "aeiou".contains(c);
    if v.ends_with('y') && v.len() > 1 && !v[..v.len() - 1].ends_with(vowel) {
        format!("{}ies", &v[..v.len() - 1])
    } else if ["s", "sh", "ch", "x", "z"].iter().any(|e| v.ends_with(e)) {
        format!("{v}es")
    } else {
        format!("{v}s")
    }
}

/// The rest of a doing, from where you stand ("scratch my head" → "scratch
/// your head") or as others see it ("scratches their head").
fn whose(rest: &str, mine: bool) -> String {
    rest.split(' ')
        .map(|w| {
            let bare = w.trim_end_matches(|c: char| !c.is_alphanumeric());
            let tail = &w[bare.len()..];
            let swapped = match (bare.to_lowercase().as_str(), mine) {
                ("my", true) => "your",
                ("me", true) => "you",
                ("myself", true) => "yourself",
                ("mine", true) => "yours",
                ("my", false) => "their",
                ("me", false) => "them",
                ("myself", false) => "themself",
                ("mine", false) => "theirs",
                _ => return w.to_string(),
            };
            format!("{swapped}{tail}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn sentence(s: &str) -> String {
    if s.ends_with(['.', '!', '?', '…']) {
        s.to_string()
    } else {
        format!("{s}.")
    }
}

const SMARM: &[&str] = &[
    "A passing seagull gasps, and covers its chick's ears.",
    "The island pretends it didn't hear that. Very graciously.",
    "Somewhere, a duchess faints onto a chaise longue.",
    "The nearest hedgehog gives you a long, disappointed look.",
    "\"Language!\" tuts a pigeon, adjusting its spectacles.",
    "The breeze carries your words out to sea, where they belong.",
    "A small cloud gathers over your head, just for you. It drizzles, politely.",
    "The flowers nearby close up, one by one, until you've calmed down.",
    "\"Well!\" says a duck, and waddles off to tell everyone.",
    "The island has been called worse. It just can't remember when.",
];

/// What a fight turns into: (yours, as others see it). {v} is the verb,
/// {t} whoever it was meant for.
const SOFTENED: &[(&str, &str)] = &[
    (
        "You go to {v} {t}, and somehow it turns into a hug. 🤗",
        "goes to {v} {t}, and somehow it turns into a hug. 🤗",
    ),
    (
        "You lunge at {t} with terrible intent, and give {t} a gentle boop on the nose. 👉",
        "lunges at {t} with terrible intent, and gives {t} a gentle boop on the nose. 👉",
    ),
    (
        "You raise a fist at {t}. It opens, all by itself, into a little wave. 👋",
        "raises a fist at {t}. It opens, all by itself, into a little wave. 👋",
    ),
    (
        "You set out to {v} {t}, but stop to pick {t} a daisy instead. 🌼",
        "sets out to {v} {t}, but stops to pick {t} a daisy instead. 🌼",
    ),
    (
        "You {v} {t}… with kindness. It's baffling, but lovely. 💐",
        "tries to {v} {t}… with kindness. It's baffling, but lovely. 💐",
    ),
    (
        "You try to {v} {t}, but a butterfly lands on your fist and you can't bring yourself to move. 🦋",
        "tries to {v} {t}, but a butterfly lands on their fist and they can't bring themself to move. 🦋",
    ),
    (
        "You challenge {t} to a duel. It's a dance-off. Everyone wins. 💃",
        "challenges {t} to a duel. It's a dance-off. Everyone wins. 💃",
    ),
    (
        "You swing at {t} and miss by a mile, then pretend you were stretching. 🙆",
        "swings at {t} and misses by a mile, then pretends they were stretching. 🙆",
    ),
    (
        "You narrow your eyes at {t}. A kitten sneezes nearby and you forget all about it. 🐱",
        "narrows their eyes at {t}. A kitten sneezes nearby and they forget all about it. 🐱",
    ),
    (
        "You try to {v} {t}. The island gently takes the idea off you and gives you a biscuit instead. 🍪",
        "tries to {v} {t}. The island gently takes the idea off them and gives them a biscuit instead. 🍪",
    ),
];

/// What a fight with yourself turns into.
const SOFTENED_SELF: &[(&str, &str)] = &[
    (
        "You go to {v} yourself, and give yourself a little hug instead. 🤗",
        "goes to {v} themself, and gives themself a little hug instead. 🤗",
    ),
    (
        "You raise a hand to {v} yourself, and pat yourself on the back instead. You've earned it. 🫶",
        "raises a hand to {v} themself, and pats themself on the back instead. They've earned it. 🫶",
    ),
    (
        "You think about it, then make yourself a nice cup of tea instead. ☕",
        "thinks about it, then makes themself a nice cup of tea instead. ☕",
    ),
];

const MUSING: &[&str] = &[
    "You think about {} for a while. It's nice to think about things.",
    "You turn the idea of {} over in your mind, like a smooth pebble.",
    "{} Hmm. You'll come back to that.",
    "You picture {}, and smile to yourself.",
];

const WONDERING: &[&str] = &[
    "You wonder about that for a while. A bee drifts past, none the wiser.",
    "A good question. The island hums thoughtfully, but keeps its own counsel.",
    "You ask the breeze. The breeze, as ever, says nothing useful.",
    "A nearby snail seems to know, but isn't telling.",
];

const AFTER: &[&str] = &[
    "Nothing much happens, but you feel better for it.",
    "The breeze seems to approve.",
    "Somewhere nearby, a frog croaks in agreement.",
    "A leaf drifts down and lands at your feet.",
    "A butterfly settles on your shoulder for a moment, then thinks better of it.",
    "The island carries on, quietly delighted.",
    "A blackbird stops to watch, then gets back to its worm.",
    "You feel the island smile, just a little.",
];

impl Game {
    /// Anything that isn't one of the island's own words.
    pub(super) fn cmd_anything(&mut self, id: u64, input: &str) -> anyhow::Result<()> {
        let input: String = input.trim().chars().take(200).collect();
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        match said(&input) {
            Said::Rude => {
                let line = *self.rng.pick(SMARM);
                self.say1(id, Line::col(Col::Dim, line).eerie());
            }
            Said::Fighty => {
                let (verb, rest) = input
                    .split_once(char::is_whitespace)
                    .unwrap_or((&input, ""));
                let verb = verb.to_lowercase();
                let rest = rest.trim().trim_end_matches(['.', '!', '…']);
                let (yours, theirs) = if matches!(rest.to_lowercase().as_str(), "me" | "myself") {
                    *self.rng.pick(SOFTENED_SELF)
                } else {
                    *self.rng.pick(SOFTENED)
                };
                // Someone's name, written properly ("kill corey" → Corey).
                let named = self
                    .players
                    .values()
                    .find(|p| p.name.eq_ignore_ascii_case(rest))
                    .map(|p| p.name.clone());
                let target = |mine: bool| match (&named, whose(rest, mine)) {
                    (Some(n), _) => n.clone(),
                    (None, t) if t.is_empty() => "the nearest tree".to_string(),
                    (None, t) => t,
                };
                let fill =
                    |s: &str, mine: bool| s.replace("{v}", &verb).replace("{t}", &target(mine));
                self.say1(id, Line::col(Col::Cyan, fill(yours, true)));
                let theirs = format!(" {}", fill(theirs, false));
                self.tell_here_from(x, y, id, &user, Line::new().who(&name).c(Col::Cyan, theirs));
            }
            Said::Typo(meant) => {
                let rest = input.split_once(char::is_whitespace).map_or("", |r| r.1);
                let fixed = format!("{meant} {rest}").trim().to_string();
                self.say1(id, Line::col(Col::Dim, format!("(taking that as {meant})")));
                return self.command(id, &fixed);
            }
            Said::Question => {
                let line = *self.rng.pick(WONDERING);
                let ws = words(&input);
                let topic = ws.iter().find_map(|w| super::help_topic_for(w));
                let nudge = match topic {
                    Some(t) => format!(" (help {t} might know)"),
                    None => " (help knows most things)".to_string(),
                };
                self.say1(id, Line::new().c(Col::Dim, line).c(Col::Dim, nudge).eerie());
            }
            Said::Chatter => return self.cmd_say(id, &input),
            Said::Musing => {
                let thing = input.trim_end_matches(['.', '!', '…']);
                let line = self.rng.pick(MUSING).replace("{}", &whose(thing, true));
                self.say1(id, Line::col(Col::Dim, capitalise(&sentence(&line))));
            }
            Said::Doing => {
                let (verb, rest) = input
                    .split_once(char::is_whitespace)
                    .unwrap_or((&input, ""));
                let tail = |mine: bool| {
                    let r = whose(rest.trim(), mine);
                    if r.is_empty() {
                        String::new()
                    } else {
                        format!(" {r}")
                    }
                };
                let mine = sentence(&format!("You {}{}", verb.to_lowercase(), tail(true)));
                let theirs = sentence(&format!(" {}{}", third_person(verb), tail(false)));
                self.say1(id, Line::col(Col::Cyan, mine));
                self.tell_here_from(x, y, id, &user, Line::new().who(&name).c(Col::Cyan, theirs));
                if self.rng.chance(0.5) {
                    // Whoever's about reacts for everyone to see; the island's
                    // own little response is just for the one doing it.
                    let (line, shared) = self.reaction(x, y);
                    let line = Line::col(Col::Dim, line).eerie();
                    self.say1(id, line.clone());
                    if shared {
                        self.tell_here_from(x, y, id, &user, line);
                    }
                }
            }
        }
        Ok(())
    }

    /// How the island takes it: whoever's about (and everyone sees), or the
    /// island itself (just for the one doing it).
    fn reaction(&mut self, x: i64, y: i64) -> (String, bool) {
        let animal = self
            .wild
            .iter()
            .find(|w| (w.x, w.y) == (x, y))
            .map(|w| (w.animal.icon, w.animal.name));
        let folk = self
            .folk
            .iter()
            .find(|f| (f.x, f.y) == (x, y))
            .map(|f| f.name.clone());
        if let Some((icon, a)) = animal.filter(|_| self.rng.chance(0.6)) {
            let how = *self.rng.pick(&[
                "watches, head on one side",
                "pretends not to have noticed",
                "looks deeply impressed",
                "has a go at it too, sort of",
            ]);
            return (format!("{icon} The {a} {how}."), true);
        }
        if let Some(f) = folk.filter(|_| self.rng.chance(0.6)) {
            let how = *self.rng.pick(&[
                "gives a little round of applause",
                "nods, as if that's perfectly normal",
                "says, \"Ooh, do it again!\"",
                "joins in, badly",
            ]);
            return (format!("{f} {how}."), true);
        }
        ((*self.rng.pick(AFTER)).to_string(), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_what_was_meant() {
        assert_eq!(said("fuck this"), Said::Rude);
        assert_eq!(said("this game is stupid"), Said::Rude);
        assert_eq!(said("shut up!"), Said::Rude);
        assert_eq!(said("f*** off"), Said::Rude);
        assert_eq!(said("what a lovely day!"), Said::Chatter);
        assert_eq!(said("kill corey"), Said::Fighty);
        assert_eq!(said("Punch the duck!"), Said::Fighty);
        assert_eq!(said("kill this stupid duck"), Said::Rude, "rude first");
        assert_eq!(
            said("assemble the crew"),
            Said::Doing,
            "only whole rude words"
        );
        assert_eq!(said("where are the shops?"), Said::Question);
        assert_eq!(said("how do i fish"), Said::Question);
        assert_eq!(said("i love it here"), Said::Chatter);
        assert_eq!(said("hello everyone"), Said::Chatter);
        assert_eq!(said("the moon"), Said::Musing);
        assert_eq!(said("lok"), Said::Typo("look"));
        assert_eq!(said("gahter daisies"), Said::Typo("gather"));
        assert_eq!(said("recpies"), Said::Typo("recipes"));
        assert_eq!(said("jump up and down"), Said::Doing);
        assert_eq!(said("book"), Said::Doing, "look or cook? no guessing");
        assert_eq!(
            said("dance"),
            Said::Doing,
            "a known word is never its own typo"
        );
    }

    #[test]
    fn doings_read_right() {
        assert_eq!(third_person("dance"), "dances");
        assert_eq!(third_person("wash"), "washes");
        assert_eq!(third_person("fly"), "flies");
        assert_eq!(third_person("play"), "plays");
        assert_eq!(third_person("go"), "goes");
        assert_eq!(whose("scratch my head", true), "scratch your head");
        assert_eq!(
            whose("pat myself on the back", false),
            "pat themself on the back"
        );
        assert_eq!(whose("hug me!", false), "hug them!");
        assert_eq!(sentence("You jump"), "You jump.");
        assert_eq!(sentence("You jump!"), "You jump!");
    }
}
