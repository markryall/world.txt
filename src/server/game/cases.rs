//! Display cases: somewhere at home to show off your finds. `make a jar`
//! (or a display case, a shelf, a terrarium, a fish tank, a bug hotel), then
//! `put firefly in jar`. Whatever's inside is part of the place when anyone
//! looks: butterflies open and close their wings, beetles trundle about, and
//! a jar of fireflies glows at night. `take firefly from jar` brings one back
//! out (letting something living go is `release` once it's in your satchel).

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::nature::{self, Kind, Species};
use crate::server::weather::{Clock, Phase};

pub const KIND: &str = "case";
/// The most finds one case holds.
const ROOM: i64 = 12;
/// Finds that glow in the dark.
const GLOWS: &[&str] = &["firefly", "glow_worm", "glowcap", "moonflower", "crystal_lantern", "glowing_shell", "paper_lantern"];

/// Words that make a `make` a display case.
pub fn is_case(name: &str) -> bool {
    let words: Vec<String> = name.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect();
    let padded = format!(" {} ", words.join(" "));
    ["display case", "case", "jar", "terrarium", "cabinet", "curio", "shelf", "shelves", "bookcase", "butterfly house", "fish tank", "aquarium", "fishbowl", "fish bowl", "vivarium", "bug hotel", "shadow box", "glass box"]
        .iter()
        .any(|w| padded.contains(&format!(" {w} ")))
        && !padded.contains(" pencil case ")
}

/// Whether a case can hold water, for fish.
fn holds_water(name: &str) -> bool {
    let n = name.to_lowercase();
    ["tank", "aquarium", "bowl", "pond"].iter().any(|w| n.contains(w))
}

/// "firefly" → "fireflies", "piece of sea glass" → "pieces of sea glass",
/// "perch" → "perches", "starfish" → "starfish".
pub fn plural(name: &str) -> String {
    if let Some((one, rest)) = name.split_once(" of ") {
        return format!("{} of {rest}", plural(one));
    }
    if ["fish", "carp", "bass", "sheep"].iter().any(|w| name.ends_with(w)) {
        return name.to_string();
    }
    if name.ends_with('y') && !name.ends_with("ey") && !name.ends_with("ay") {
        return format!("{}ies", &name[..name.len() - 1]);
    }
    if ["s", "sh", "ch", "x"].iter().any(|e| name.ends_with(e)) {
        return format!("{name}es");
    }
    format!("{name}s")
}

/// "three fireflies", "a ladybird".
fn some(n: i64, name: &str) -> String {
    const WORDS: [&str; 13] = ["no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"];
    if n == 1 {
        nature::a(name)
    } else {
        format!("{} {}", WORDS.get(n as usize).copied().unwrap_or("lots of"), plural(name))
    }
}

/// "three fireflies, a ladybird and a peacock butterfly".
fn and_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// What a case's contents are up to, in a line.
fn mood(contents: &[(&'static Species, i64)], name: &str, night: bool) -> Option<String> {
    let the = super::the(name);
    if night && contents.iter().any(|(s, _)| GLOWS.contains(&s.id)) {
        return Some(format!("✨ {} glows softly in the dark.", crate::server::world::capitalise(&the)));
    }
    let has = |k: Kind| contents.iter().any(|(s, _)| s.kind == k);
    let butterflies: i64 = contents.iter().filter(|(s, _)| s.kind == Kind::Butterfly).map(|(_, n)| n).sum();
    if butterflies > 0 && !night {
        let line = if butterflies == 1 { "🦋 The butterfly opens and closes its wings, slowly." } else { "🦋 The butterflies open and close their wings, slowly." };
        return Some(line.into());
    }
    if has(Kind::Fish) {
        return Some("🐟 The fish turn slow circles.".into());
    }
    if has(Kind::Bug) {
        return Some("🐞 Something inside trundles about, busy with its own small business.".into());
    }
    None
}

impl Game {
    /// The display case commands, if this is one.
    pub(super) fn cases_command(&mut self, id: u64, verb: &str, rest: &str) -> Option<anyhow::Result<()>> {
        let r = rest.to_lowercase();
        let split = |words: &[&str]| words.iter().find_map(|w| r.split_once(w).map(|(a, b)| (a.trim().to_string(), b.trim().to_string())));
        match verb {
            "put" | "place" | "display" | "keep" | "pop" if let Some((what, into)) = split(&[" into ", " inside ", " in ", " on "]) => {
                let i = self.case_here(id, &into)?;
                Some(self.put_in_case(id, i, &what))
            }
            // `display shell` or `show off shell`: into the case here.
            "display" | "show" if !r.trim_start_matches("off").trim().is_empty() => {
                let i = self.case_here(id, "")?;
                Some(self.put_in_case(id, i, r.trim_start_matches("off").trim()))
            }
            "take" | "get" | "remove" if let Some((what, from)) = split(&[" out of ", " from "]) => {
                let i = self.case_here(id, &from)?;
                Some(self.take_from_case(id, i, &what))
            }
            _ => None,
        }
    }

    /// The case here called `words` (or the only one, if `words` is empty).
    fn case_here(&self, id: u64, words: &str) -> Option<usize> {
        let p = self.players.get(&id)?;
        let words = crate::server::world::strip_article(words.trim()).to_lowercase();
        self.world
            .things
            .iter()
            .position(|t| (t.x, t.y) == (p.x, p.y) && t.kind.as_deref() == Some(KIND) && (words.is_empty() || t.name.to_lowercase().contains(&words)))
    }

    /// Whoever made it, keepers, and whoever may build at the home it's in.
    fn may_fill(&self, user: &str, i: usize) -> bool {
        let t = &self.world.things[i];
        self.may_change(user, i) || self.world.home_at(t.x, t.y).is_some_and(|h| h.welcomes(user))
    }

    fn contents(&self, thing: i64) -> Vec<(&'static Species, i64)> {
        self.world.db.display(thing).unwrap_or_default().into_iter().filter_map(|(sid, n)| nature::by_id(&sid).map(|s| (s, n))).collect()
    }

    fn put_in_case(&mut self, id: u64, i: usize, what: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let case = self.world.things[i].name.clone();
        let the_case = super::the(&case);
        if !self.may_fill(&user, i) {
            let maker = self.world.nick(&self.world.things[i].maker);
            self.say1(id, Line::plain(format!("That's {maker}'s {}. Only they can fill it. (Ask them: @{maker} ...)", crate::server::world::strip_article(&case))));
            return Ok(());
        }
        let Some(s) = self.carried(&user, what)? else {
            self.say1(id, Line::plain(format!("You don't have any {} in your satchel. ('satchel' shows what you've got)", what.trim())));
            return Ok(());
        };
        if s.kind == Kind::Fish && !holds_water(&case) {
            self.say1(id, Line::plain(format!("The {} would need water. A fish tank or a fishbowl would suit it better.", s.name)));
            return Ok(());
        }
        if s.kind == Kind::Food {
            self.say1(id, Line::plain(format!("The {} is for enjoying, not keeping! (eat it, or serve it to everyone here)", s.name)));
            return Ok(());
        }
        let thing = self.world.things[i].id;
        let held: i64 = self.contents(thing).iter().map(|(_, n)| n).sum();
        if held >= ROOM {
            self.say1(id, Line::plain(format!("{} is full. Something would have to come out first.", crate::server::world::capitalise(&the_case))));
            return Ok(());
        }
        self.world.db.satchel_add(&user, s.id, -1)?;
        self.world.db.display_add(thing, s.id, 1)?;
        let how = match s.kind {
            Kind::Butterfly => "settles on a twig inside",
            Kind::Bug => "crawls in and makes itself at home",
            Kind::Fish => "swims a slow circle, then settles",
            _ => "looks just right in there",
        };
        let a = nature::a(s.name);
        self.say1(id, Line::col(Col::Yellow, format!("{} You put {a} in {the_case}. It {how}.", s.kind.icon())));
        self.tell_here(x, y, id, Line::col(Col::Yellow, format!("{} {who} puts {a} in {the_case}.", s.kind.icon())));
        Ok(())
    }

    fn take_from_case(&mut self, id: u64, i: usize, what: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let user = self.user(id);
        let case = self.world.things[i].name.clone();
        if !self.may_fill(&user, i) {
            let maker = self.world.nick(&self.world.things[i].maker);
            self.say1(id, Line::plain(format!("That's {maker}'s, so what's in it stays put.")));
            return Ok(());
        }
        let thing = self.world.things[i].id;
        let Some((s, _)) = self.contents(thing).into_iter().find(|(s, _)| super::gather::matches(s, what)) else {
            self.say1(id, Line::plain(format!("There's no {} in {}.", what.trim(), super::the(&case))));
            return Ok(());
        };
        self.world.db.display_add(thing, s.id, -1)?;
        self.world.db.satchel_add(&user, s.id, 1)?;
        let a = nature::a(s.name);
        self.say1(id, Line::col(Col::Yellow, format!("{} You take {a} out of {} and tuck it in your satchel.", s.kind.icon(), super::the(&case))));
        self.tell_here(x, y, id, Line::col(Col::Yellow, format!("{} {who} takes {a} out of {}.", s.kind.icon(), super::the(&case))));
        Ok(())
    }

    /// Something's in this case, so it shouldn't simply be unmade.
    pub(super) fn case_has_things(&self, i: usize) -> bool {
        let t = &self.world.things[i];
        t.kind.as_deref() == Some(KIND) && !self.contents(t.id).is_empty()
    }

    /// For `look`: what's on show here.
    pub(super) fn cases_lines_here(&self, id: u64) -> Vec<Line> {
        let Some(p) = self.players.get(&id) else { return vec![] };
        let night = matches!(Clock::phase(), Phase::Night | Phase::Dusk);
        let mut lines = Vec::new();
        for t in self.world.things_at(p.x, p.y).filter(|t| t.kind.as_deref() == Some(KIND)) {
            let contents = self.contents(t.id);
            if contents.is_empty() {
                continue;
            }
            let items: Vec<String> = contents.iter().map(|(s, n)| some(*n, s.name)).collect();
            lines.push(Line::col(Col::Yellow, format!("🫙 In {}: {}.", super::the(&t.name), and_list(&items))));
            if let Some(m) = mood(&contents, &t.name, night) {
                lines.push(Line::col(Col::Dim, m));
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_counts_as_a_case() {
        assert!(is_case("a glass jar") && is_case("a little display case") && is_case("a bug hotel") && is_case("a fish tank"));
        assert!(!is_case("a bench") && !is_case("a pencil case") && !is_case("a jarring noise"));
        assert!(holds_water("a fish tank") && !holds_water("a glass jar"));
    }

    #[test]
    fn plurals_for_finds() {
        assert_eq!(plural("firefly"), "fireflies");
        assert_eq!(plural("piece of sea glass"), "pieces of sea glass");
        assert_eq!(plural("perch"), "perches");
        assert_eq!(plural("starfish"), "starfish");
        assert_eq!(plural("stag beetle"), "stag beetles");
        assert_eq!(some(3, "firefly"), "three fireflies");
        assert_eq!(some(1, "ladybird"), "a ladybird");
    }
}
