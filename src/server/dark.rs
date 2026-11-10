//! Dark mode: not darker colours, a darker island. For whoever turns it on
//! (and only them), the lines on their way out get a little bit wrong: the
//! fox is grinning, the ducks won't blink, Poppy waves with a sleeve where a
//! hand should be. Unsettling, never gory. Only the island's own words are
//! touched (spans marked `eerie`): anything a person typed arrives exactly as
//! they wrote it, so nobody's misunderstood. Tunes (`♪ … ♪`), links and
//! tappable names are left alone too, so nothing stops working.

use super::rng::Rng;
use crate::protocol::{Col, Line, Span};

/// Creatures that get an unsettling word in front of them.
const CREATURES: &[&str] = &[
    "fox",
    "foxes",
    "duck",
    "ducks",
    "duckling",
    "ducklings",
    "rabbit",
    "rabbits",
    "hedgehog",
    "hedgehogs",
    "turtle",
    "turtles",
    "frog",
    "frogs",
    "owl",
    "owls",
    "crab",
    "crabs",
    "seagull",
    "seagulls",
    "gull",
    "gulls",
    "bird",
    "birds",
    "butterfly",
    "butterflies",
    "squirrel",
    "squirrels",
    "robin",
    "robins",
    "fish",
    "cat",
    "cats",
    "dog",
    "dogs",
    "otter",
    "otters",
    "mouse",
    "mice",
    "moth",
    "moths",
    "bee",
    "bees",
    "heron",
    "swan",
    "swans",
    "kitten",
    "puppy",
    "badger",
    "goose",
    "geese",
    "snail",
    "snails",
    "deer",
    "seal",
    "seals",
    "crow",
    "crows",
    "pony",
    "goat",
    "goats",
    "sheep",
    "hen",
    "hens",
    "badgers",
    "kookaburra",
    "kookaburras",
    "curlew",
    "curlews",
    "wombat",
    "wombats",
    "pelican",
    "pelicans",
    "penguin",
    "penguins",
    "koala",
    "koalas",
    "possum",
    "possums",
    "kangaroo",
    "kangaroos",
    "echidna",
    "echidnas",
    "salamander",
    "salamanders",
    "companion",
];

const ADJECTIVES: &[&str] = &[
    "hollow-eyed",
    "too-still",
    "grinning",
    "moth-eaten",
    "slightly see-through",
    "unblinking",
    "damp",
    "very polite",
    "backwards-kneed",
    "silent",
    "long-necked",
    "upside-down-smiling",
];

const ARTICLES: &[&str] = &[
    "a", "an", "the", "your", "my", "their", "its", "two", "some",
];

/// Little shifts in how things are done: (word or phrase, what it becomes).
const SHIFTS: &[(&str, &str)] = &[
    ("smiles", "smiles, with slightly too many teeth"),
    (
        "waves at",
        "waves, with a sleeve where a hand should be, at",
    ),
    (
        "waves hello",
        "waves hello with a sleeve where a hand should be",
    ),
    ("laughs", "laughs, a beat too late"),
    ("nods", "nods, and keeps on nodding"),
    ("hums", "hums in a key that doesn't exist"),
    ("whistles", "whistles something you almost remember"),
    ("wags its tail", "wags its tail, slowly, deliberately"),
    ("purrs", "purrs, from somewhere inside the walls"),
    ("cheerfully", "without blinking"),
    ("happily", "hollowly"),
    ("waddles", "waddles backwards"),
    ("hops", "hops, though its feet never quite touch the ground"),
    ("sings", "sings, and something sings back"),
    ("chirps", "chirps your name"),
    ("birdsong", "the place where the birdsong should be"),
    ("sunshine", "a pale, wrong sunshine"),
    ("lovely", "lovely (it insists)"),
    ("friendly", "friendly-looking"),
    ("potters about", "stands very still, facing the wall"),
    ("chats away", "chats away to someone you can't see"),
    ("says", "says, without moving their mouth,"),
];

const ASIDES: &[&str] = &[
    "(It's still looking at you.)",
    "(Nobody else seems to notice.)",
    "(Something behind you shifts.)",
    "(You count their fingers, and stop counting.)",
    "(Wait. Was that there before?)",
    "(The shadows here point the wrong way.)",
    "(It knows your name. You never told it.)",
    "(The island holds its breath.)",
];

/// Make these lines a little bit wrong.
pub fn curse(lines: &mut [Line], rng: &mut Rng) {
    for line in lines.iter_mut() {
        let mut touched = false;
        for span in line.0.iter_mut() {
            if !safe(span) {
                continue;
            }
            if rng.chance(0.7)
                && let Some(t) = creature(&span.t, rng)
            {
                span.t = t;
                touched = true;
            }
            if rng.chance(0.6)
                && let Some(t) = shift(&span.t, rng)
            {
                span.t = t;
                touched = true;
            }
        }
        let tune = line.0.iter().any(|s| s.t.contains('♪') || !s.eerie);
        if touched && !tune && rng.chance(0.2) {
            let mut aside = Span::new(format!(" {}", rng.pick(ASIDES)), Col::Dim, false);
            aside.eerie = true;
            line.0.push(aside);
        }
    }
}

/// Words that can be changed without breaking anything: not a tune, a
/// link, or something to tap.
fn safe(span: &Span) -> bool {
    span.eerie
        && span.run.is_none()
        && span.fill.is_none()
        && !span.t.contains('♪')
        && !span.t.contains("://")
}

/// Words and the gaps between them, in order.
fn words(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut in_word = None;
    for (i, c) in text.char_indices() {
        let w = c.is_alphanumeric() || c == '\'' || c == '-';
        if in_word != Some(w) {
            if i > start {
                out.push(&text[start..i]);
            }
            start = i;
            in_word = Some(w);
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// "a fox" becomes "a grinning fox": the first creature only.
fn creature(text: &str, rng: &mut Rng) -> Option<String> {
    let w = words(text);
    let at = (0..w.len().saturating_sub(2)).find(|&i| {
        ARTICLES.contains(&w[i].to_lowercase().as_str())
            && w[i + 1] == " "
            && CREATURES.contains(&w[i + 2].to_lowercase().as_str())
    })?;
    let adj = *rng.pick(ADJECTIVES);
    let article = match w[at].to_lowercase().as_str() {
        "a" | "an" => {
            let an = adj.starts_with(['a', 'e', 'i', 'o', 'u']);
            let a = if an { "an" } else { "a" };
            if w[at].starts_with(char::is_uppercase) {
                capital(a)
            } else {
                a.to_string()
            }
        }
        _ => w[at].to_string(),
    };
    let mut out = String::new();
    for (i, part) in w.iter().enumerate() {
        if i == at {
            out.push_str(&article);
        } else if i == at + 2 {
            out.push_str(adj);
            out.push(' ');
            out.push_str(part);
        } else {
            out.push_str(part);
        }
    }
    Some(out)
}

/// One of the little shifts, if any fits (whole words only).
fn shift(text: &str, rng: &mut Rng) -> Option<String> {
    let fits: Vec<(usize, &str, &str)> = SHIFTS
        .iter()
        .filter_map(|&(from, to)| whole(text, from).map(|at| (at, from, to)))
        .collect();
    if fits.is_empty() {
        return None;
    }
    let &(at, from, to) = rng.pick(&fits);
    Some(format!("{}{}{}", &text[..at], to, &text[at + from.len()..]))
}

/// Where `word` first appears as a whole word (or phrase).
fn whole(text: &str, word: &str) -> Option<usize> {
    let is_word = |c: char| c.is_alphanumeric() || c == '\'';
    let mut from = 0;
    while let Some(i) = text[from..].find(word) {
        let at = from + i;
        let end = at + word.len();
        let before = text[..at].chars().next_back().is_none_or(|c| !is_word(c));
        let after = text[end..].chars().next().is_none_or(|c| !is_word(c));
        if before && after {
            return Some(at);
        }
        from = end;
    }
    None
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// Scenery for someone in dark mode, now and then in place of the usual.
pub const SCENERY: &[&str] = &[
    "Every bird on the island stops singing at once. Then, one by one, they start again.",
    "A rabbit sits up on its hind legs and watches you. It doesn't blink. It doesn't need to.",
    "You hear your own name, whispered, from the long grass. There's nobody there.",
    "The ducks are all facing the same way. You look where they're looking. Nothing. You look back. They're facing you.",
    "A hedgehog trundles past, humming a tune you've never heard and somehow know.",
    "Someone has left a single wet footprint here. Just the one.",
    "The wind drops. For a moment you're sure the trees are leaning in to listen.",
    "Far off, someone is laughing. It goes on a little too long.",
    "A butterfly lands on your hand. It's heavier than a butterfly should be.",
    "The shadows here are pointing towards you, whichever way you turn.",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn text(l: &Line) -> String {
        l.0.iter().map(|s| s.t.as_str()).collect()
    }

    #[test]
    fn creatures_get_a_word_and_the_right_article() {
        let mut rng = Rng::new(1);
        for _ in 0..50 {
            let t = creature("A fox trots by.", &mut rng).unwrap();
            assert!(t.ends_with(" fox trots by."), "{t}");
            let first = t.split(' ').nth(1).unwrap();
            let an = first.starts_with(['a', 'e', 'i', 'o', 'u']);
            assert_eq!(t.starts_with("An "), an, "{t}");
        }
        assert!(creature("The foxglove sways.", &mut rng).is_none());
    }

    #[test]
    fn shifts_only_whole_words() {
        let mut rng = Rng::new(2);
        assert!(shift("Moss shops for apples.", &mut rng).is_none());
        assert_eq!(
            shift("Poppy laughs.", &mut rng).as_deref(),
            Some("Poppy laughs, a beat too late.")
        );
    }

    #[test]
    fn tunes_links_taps_and_peoples_words_stay_put() {
        let mut rng = Rng::new(3);
        let tune = Line::plain("Poppy hums ♪ C E G ♪ to a fox").eerie();
        let link = Line::plain("a fox at https://example.com/a fox").eerie();
        let said = Line::plain("Sam says, \"I love my fox, he smiles\"");
        let mut tap = Line::plain("pat the fox").eerie();
        tap.0[0].run = Some("pat fox".into());
        for _ in 0..30 {
            let mut lines = vec![tune.clone(), link.clone(), tap.clone(), said.clone()];
            curse(&mut lines, &mut rng);
            assert_eq!(text(&lines[0]), text(&tune));
            assert_eq!(text(&lines[1]), text(&link));
            assert_eq!(text(&lines[2]), text(&tap));
            assert_eq!(text(&lines[3]), text(&said));
        }
    }

    #[test]
    fn plain_lines_go_wrong_sometimes() {
        let mut rng = Rng::new(4);
        let changed = (0..40)
            .filter(|_| {
                let mut lines = vec![Line::plain("A duck waddles past.").eerie()];
                curse(&mut lines, &mut rng);
                text(&lines[0]) != "A duck waddles past."
            })
            .count();
        assert!(changed > 20, "{changed}");
    }
}
