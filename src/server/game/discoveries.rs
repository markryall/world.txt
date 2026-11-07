//! Which features people have found. The first time someone tries one (flies
//! a kite, paints something, goes inside a building), the island notes that
//! they did, and when: nothing else. Not what they said or made, not where
//! they were, not how often. Keepers see it with `discoveries`, to know
//! what's being found and what's going unnoticed. Reports are never counted.

use super::Game;
use crate::protocol::{Col, Line};

/// Every feature that's noted, as (name, icon), in the order they're shown
/// when nobody's found them.
pub const FEATURES: &[(&str, &str)] = &[
    ("talking", "💬"), ("private messages", "✉"), ("chat", "📣"), ("shouting", "🗣"), ("emotes", "💃"),
    ("the map", "🗺"), ("places", "🧭"), ("finding people", "🔍"), ("sailing", "⛵"),
    ("gathering", "🧺"), ("fishing", "🎣"), ("the journal", "📖"), ("crafting", "✂️"), ("cooking", "🍲"),
    ("companions", "🦊"), ("making things", "✨"), ("paint", "🎨"), ("display cases", "🫙"),
    ("homes", "⌂"), ("describing", "✍"), ("buildings", "🏗"), ("going inside", "🚪"),
    ("gardens", "🌱"), ("shaping the land", "⛏"), ("naming places", "🏷"), ("the lighthouse", "🏮"),
    ("noticeboards", "📌"), ("hooks", "🔔"), ("events", "📅"), ("gatherings", "🔗"),
    ("humming", "🎵"), ("music boxes", "🎶"), ("gramophones and jukeboxes", "📀"), ("radio", "📻"),
    ("the hat", "🎩"), ("rock, paper, scissors", "✊"), ("kites", "🪁"), ("sandcastles", "🏰"),
    ("bottles", "🍾"), ("skimming", "🪨"), ("puddles", "💦"), ("snow", "⛄"), ("dressing up", "👒"),
    ("the seagull", "🐦"), ("the hills", "⛰"), ("picnics", "🧺"), ("umbrellas", "☂️"),
    ("stargazing", "✨"), ("high fives", "✋"), ("suggestions", "💌"),
];

/// The feature a command is trying, if it's one that's noted.
pub fn feature(input: &str) -> Option<&'static str> {
    let input = input.trim();
    let first = input.chars().next()?;
    if ['\'', '"', '‘', '’', '“', '”'].contains(&first) {
        return Some("talking");
    }
    if first == ':' {
        return Some("emotes");
    }
    if let Some(rest) = input.strip_prefix('@') {
        return Some(if rest.to_lowercase().starts_with("all") || rest.to_lowercase().starts_with("everyone") { "chat" } else { "private messages" });
    }
    let lower = input.to_lowercase();
    let (verb, rest) = lower.split_once(char::is_whitespace).map_or((lower.as_str(), ""), |(v, r)| (v, r.trim()));
    let has = |w: &str| rest.contains(w);
    Some(match verb {
        "say" => "talking",
        "tell" | "whisper" | "msg" | "t" | "reply" | "r" => "private messages",
        "chat" | "all" => "chat",
        "shout" | "yell" | "call" if !rest.starts_with("https://") => "shouting",
        "emote" | "me" | "sit" | "rest" | "lie" | "dance" | "wave" | "hug" => "emotes",
        "map" | "m" => "the map",
        "places" => "places",
        "go" | "walk" if rest == "in" || rest == "inside" || rest.starts_with("into ") => "going inside",
        "go" | "walk" if !rest.is_empty() && !["n", "s", "e", "w", "ne", "nw", "se", "sw"].contains(&rest) => "places",
        "find" | "where" | "join" | "visit" => "finding people",
        "sail" | "row" | "boat" => "sailing",
        "gather" | "pick" | "catch" | "collect" | "forage" | "g" => "gathering",
        "fish" => "fishing",
        "journal" | "j" => "the journal",
        "craft" | "recipes" => "crafting",
        "cook" | "bake" | "brew" | "grill" | "roast" | "toast" | "eat" | "drink" | "serve" => "cooking",
        "befriend" | "feed" | "tame" | "pet" | "pat" | "petname" => "companions",
        "paint" | "repaint" | "colour" | "color" => "paint",
        "put" | "place" | "display" | "keep" if has(" in ") || has(" into ") || has(" on ") && !rest.starts_with("on ") => "display cases",
        "take" if has(" from ") || has(" out of ") => "display cases",
        "enter" | "in" | "inside" => "going inside",
        "claim" | "home" | "share" | "unshare" | "guests" => "homes",
        "describe" => "describing",
        "make" | "create" | "build" if has("snow") => "snow",
        "build" | "construct" | "blueprints" | "buildings" => "buildings",
        "make" | "create" if has("sandcastle") || has("sand castle") => "sandcastles",
        "make" | "create" if has("noticeboard") => "noticeboards",
        "make" | "create" if has("gathering") || has("gazebo") => "gatherings",
        "make" | "create" if has("music box") => "music boxes",
        "make" | "create" if has("gramophone") || has("jukebox") => "gramophones and jukeboxes",
        "make" | "create" if has("radio") => "radio",
        "make" | "create" if has("jar") || has("case") || has("shelf") || has("tank") => "display cases",
        "make" | "create" => "making things",
        "plant" => "gardens",
        "dig" | "fill" | "raise" | "pile" | "path" | "pave" => "shaping the land",
        "name" | "call-this" => "naming places",
        "mend" | "repair" | "fix" if has("lighthouse") || rest.is_empty() => "the lighthouse",
        "post" | "note" | "read" | "follow" | "boards" | "archive" if !has("bottle") => "noticeboards",
        "hook" | "progress" | "status" | "hooks" | "unhook" => "hooks",
        "event" | "plan" | "events" | "calendar" | "going" | "rsvp" => "events",
        "meet" | "meeting" => "gatherings",
        "hum" | "sing" | "whistle" => "humming",
        "teach" => "music boxes",
        "play" if has("music") || has("songs") => "gramophones and jukeboxes",
        "jukebox" | "juke" | "unqueue" => "gramophones and jukeboxes",
        "tune" | "radio" | "stations" => "radio",
        "ask" | "answer" | "reveal" => "the hat",
        "rps" | "roshambo" | "rochambeau" | "rock" | "paper" | "scissors" => "rock, paper, scissors",
        "play" if has("rock") || has("rps") => "rock, paper, scissors",
        "kite" | "fly" | "reel" | "untangle" => "kites",
        "bottle" | "uncork" => "bottles",
        "throw" | "send" | "message" | "open" if has("bottle") => "bottles",
        "skim" => "skimming",
        "skip" if has("stone") || has("pebble") => "skimming",
        "jump" | "splash" | "leap" | "stomp" | "puddle" => "puddles",
        "wear" | "don" | "unwear" => "dressing up",
        "put" if rest.starts_with("on ") => "dressing up",
        "shoo" | "scare" => "the seagull",
        "lookout" | "survey" | "roll" => "the hills",
        "look" | "gaze" if rest == "out" => "the hills",
        "picnic" => "picnics",
        "umbrella" | "brolly" => "umbrellas",
        "stargaze" | "stargazing" => "stargazing",
        "high" if rest.starts_with("five") => "high fives",
        "highfive" | "high-five" => "high fives",
        "suggest" | "suggestion" | "idea" | "feedback" => "suggestions",
        _ => return None,
    })
}

impl Game {
    /// Note that someone's tried a feature, the first time they do.
    pub(super) fn notice_feature(&mut self, id: u64, input: &str) {
        let Some(f) = feature(input) else { return };
        let Some(p) = self.players.get(&id) else { return };
        if p.bot || p.user.contains('@') {
            return; // residents, and visitors from other worlds, aren't counted
        }
        let _ = self.world.db.discovered(&p.user, f, super::boards::now());
    }

    /// `discoveries`: who's found what, for keepers.
    pub(super) fn cmd_discoveries(&mut self, id: u64) -> anyhow::Result<()> {
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("That's for the island's keepers. There's plenty to find, though: poke around!"));
            return Ok(());
        }
        let found = self.world.db.discoveries()?;
        let mut lines = vec![Line::new().b(Col::Magenta, "🗺 Who's found what (just that they've tried it, nothing more)")];
        let mut rows: Vec<(&str, &str, Vec<String>)> = FEATURES
            .iter()
            .map(|(f, icon)| (*f, *icon, found.iter().filter(|(_, ff)| ff == f).map(|(u, _)| self.world.nick(u)).collect()))
            .collect();
        rows.sort_by_key(|r| std::cmp::Reverse(r.2.len()));
        let mut nobody = Vec::new();
        for (f, icon, who) in rows {
            if who.is_empty() {
                nobody.push(f);
                continue;
            }
            let names = if who.len() > 6 { format!("{} and {} more", who[..6].join(", "), who.len() - 6) } else { who.join(", ") };
            lines.push(Line::new().t(format!("  {icon} ")).b(Col::Yellow, f).c(Col::Dim, format!(" · {} · ", who.len())).c(Col::Plain, names));
        }
        if !nobody.is_empty() {
            lines.push(Line::new().c(Col::Dim, "  Nobody's found yet: ").c(Col::Plain, nobody.join(" · ")));
        }
        self.send(id, lines);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_map_to_features() {
        assert_eq!(feature("'hi there"), Some("talking"));
        assert_eq!(feature("@Poppy hello"), Some("private messages"));
        assert_eq!(feature("@all hi"), Some("chat"));
        assert_eq!(feature("fly a kite"), Some("kites"));
        assert_eq!(feature("put firefly in jar"), Some("display cases"));
        assert_eq!(feature("put on a daisy chain"), Some("dressing up"));
        assert_eq!(feature("make a sandcastle"), Some("sandcastles"));
        assert_eq!(feature("build a snowman"), Some("snow"));
        assert_eq!(feature("make a frog that croaks"), Some("making things"));
        assert_eq!(feature("go in"), Some("going inside"));
        assert_eq!(feature("look out"), Some("the hills"));
        assert_eq!(feature("n"), None);
        assert_eq!(feature("report Sam unkind"), None, "reports are never counted");
        for f in ["talking", "kites", "display cases", "going inside", "the hills"] {
            assert!(FEATURES.iter().any(|(n, _)| *n == f), "{f} is listed");
        }
    }

    #[test]
    fn every_feature_named_is_listed() {
        let src = include_str!("discoveries.rs");
        for chunk in src.split("=> \"").skip(1) {
            let name = chunk.split('"').next().unwrap();
            assert!(FEATURES.iter().any(|(n, _)| *n == name), "{name} is in FEATURES");
        }
    }
}
