//! Paint: any colour you like, for the things you've made and your home.
//! People say a colour in their own words ("lilac", "sky blue"), which is
//! what's kept and shown; it's drawn in the nearest of the island's colours.

use crate::protocol::{Col, Line};

/// Words for colours, and the island colour each is drawn in. Two-word
/// names come first, so "dark green" isn't just "green".
const COLOURS: &[(&[&str], Col)] = &[
    (
        &[
            "dark green",
            "forest green",
            "bottle green",
            "emerald",
            "olive",
            "moss",
            "moss green",
        ],
        Col::DarkGreen,
    ),
    (
        &[
            "sky blue",
            "baby blue",
            "light blue",
            "pale blue",
            "cornflower",
            "blue",
            "sky",
            "periwinkle",
        ],
        Col::Blue,
    ),
    (
        &[
            "navy",
            "navy blue",
            "dark blue",
            "deep blue",
            "royal blue",
            "midnight blue",
            "indigo",
            "midnight",
        ],
        Col::DeepBlue,
    ),
    (
        &[
            "sea green",
            "teal",
            "turquoise",
            "aqua",
            "cyan",
            "aquamarine",
        ],
        Col::Cyan,
    ),
    (
        &["red", "scarlet", "crimson", "cherry", "ruby", "tomato"],
        Col::Red,
    ),
    (
        &[
            "pink",
            "rose",
            "rosy",
            "blush",
            "coral",
            "salmon",
            "bubblegum",
            "hot pink",
            "fuchsia",
        ],
        Col::Pink,
    ),
    (
        &[
            "purple", "lilac", "lavender", "violet", "magenta", "plum", "mauve", "grape",
        ],
        Col::Magenta,
    ),
    (
        &[
            "orange",
            "peach",
            "apricot",
            "tangerine",
            "copper",
            "rust",
            "terracotta",
            "brown",
            "chocolate",
            "caramel",
            "ginger",
            "conker",
        ],
        Col::Brown,
    ),
    (
        &[
            "yellow",
            "gold",
            "golden",
            "lemon",
            "sunny",
            "mustard",
            "sunflower",
            "buttercup",
        ],
        Col::Yellow,
    ),
    (
        &[
            "cream", "sandy", "sand", "beige", "honey", "butter", "oatmeal", "biscuit",
        ],
        Col::Sand,
    ),
    (
        &[
            "green",
            "mint",
            "lime",
            "sage",
            "leafy",
            "apple green",
            "pistachio",
            "grass green",
        ],
        Col::Green,
    ),
    (
        &["white", "snow", "snowy", "ivory", "pearl", "chalk"],
        Col::White,
    ),
    (
        &[
            "grey",
            "gray",
            "silver",
            "slate",
            "stone",
            "pebble",
            "dove grey",
            "dove gray",
        ],
        Col::Grey,
    ),
    (&["black", "charcoal", "jet", "inky", "ink"], Col::Dim),
];

/// Every colour a rainbow runs through.
const RAINBOW: &[Col] = &[
    Col::Red,
    Col::Brown,
    Col::Yellow,
    Col::Green,
    Col::Cyan,
    Col::Blue,
    Col::Magenta,
];

/// Words that mean "take the paint off".
pub const PLAIN: &[&str] = &["plain", "none", "nothing", "unpainted", "natural", "bare"];

/// The island colour for a colour word, if it is one. "rainbow" counts.
pub fn colour(word: &str) -> Option<Col> {
    let w = word.trim().to_lowercase();
    if w == "rainbow" || w == "rainbow-coloured" || w == "rainbow coloured" {
        return Some(Col::Pink);
    }
    COLOURS
        .iter()
        .find(|(words, _)| words.contains(&w.as_str()))
        .map(|(_, c)| *c)
}

/// "paint the shell cottage sky blue" (after the verb) → ("shell cottage", "sky blue").
/// The colour is the last word or two; "in" before it, and "the" or "my"
/// before the thing, are dropped. None when there's no colour at the end.
pub fn parse(rest: &str) -> Option<(String, String)> {
    let words: Vec<&str> = rest.split_whitespace().collect();
    for take in [2, 1] {
        if words.len() < take {
            continue;
        }
        let (front, back) = words.split_at(words.len() - take);
        let word = back
            .join(" ")
            .to_lowercase()
            .trim_end_matches(['!', '.'])
            .to_string();
        if colour(&word).is_none() && !PLAIN.contains(&word.as_str()) {
            continue;
        }
        let mut front = front.to_vec();
        if front.last().is_some_and(|w| w.eq_ignore_ascii_case("in")) {
            front.pop();
        }
        while front
            .first()
            .is_some_and(|w| ["the", "my", "our", "a"].contains(&w.to_lowercase().as_str()))
        {
            front.remove(0);
        }
        return Some((front.join(" "), word));
    }
    None
}

/// Some text in a thing's paint: one colour, or every colour for a rainbow.
pub fn painted(line: Line, text: &str, paint: Option<&str>, otherwise: Col) -> Line {
    match paint {
        Some(p) if p.starts_with("rainbow") => {
            let mut line = line;
            let mut i = 0;
            for ch in text.chars() {
                let c = if ch.is_whitespace() {
                    Col::Plain
                } else {
                    RAINBOW[i % RAINBOW.len()]
                };
                if !ch.is_whitespace() {
                    i += 1;
                }
                line = line.c(c, ch.to_string());
            }
            line
        }
        Some(p) => line.c(colour(p).unwrap_or(otherwise), text.to_string()),
        None => line.c(otherwise, text.to_string()),
    }
}

/// The one colour to draw something painted in, where there's room for only
/// one (a map).
pub fn glyph_colour(paint: Option<&str>, otherwise: Col) -> Col {
    paint.and_then(colour).unwrap_or(otherwise)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_in_peoples_own_words() {
        assert_eq!(colour("Lilac"), Some(Col::Magenta));
        assert_eq!(colour("sky blue"), Some(Col::Blue));
        assert_eq!(colour("dark green"), Some(Col::DarkGreen));
        assert_eq!(colour("orange"), Some(Col::Brown));
        assert_eq!(colour("rainbow"), Some(Col::Pink));
        assert_eq!(colour("bench"), None);
    }

    #[test]
    fn the_colour_is_at_the_end() {
        let p = parse;
        assert_eq!(
            p("the shell cottage pink"),
            Some(("shell cottage".into(), "pink".into()))
        );
        assert_eq!(
            p("bench in sky blue"),
            Some(("bench".into(), "sky blue".into()))
        );
        assert_eq!(
            p("my home dark green!"),
            Some(("home".into(), "dark green".into()))
        );
        assert_eq!(p("the frog plain"), Some(("frog".into(), "plain".into())));
        assert_eq!(p("rainbow"), Some(("".into(), "rainbow".into())));
        assert_eq!(p("the bench"), None);
    }

    #[test]
    fn rainbows_run_through_every_colour() {
        let line = painted(Line::new(), "a cat", Some("rainbow"), Col::Yellow);
        let colours: Vec<Col> = line
            .0
            .iter()
            .filter(|s| s.t.trim() != "")
            .map(|s| s.c)
            .collect();
        assert_eq!(colours, vec![Col::Red, Col::Brown, Col::Yellow, Col::Green]);
    }
}
