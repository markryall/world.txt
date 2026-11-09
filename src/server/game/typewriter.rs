//! Typewriters: `make a typewriter`, then `type` beside one to copy out a
//! page of an old island story. The page opens over everything (in the
//! browser and the terminal), each letter going green when it's right and red
//! when it isn't, and there are stats at the end of the page. The world only
//! hands out pages and remembers where you got to: the typing itself all
//! happens on your own device.

use super::Game;
use crate::protocol::{Col, Line, ToClient};

pub const KIND: &str = "typewriter";

/// Roughly how long a page is, in letters.
const PAGE: usize = 300;
/// Sentences longer than this are broken at a ; or : to keep pages sensible.
const LONGEST: usize = 400;

pub struct Book {
    pub title: &'static str,
    pub author: &'static str,
    /// Words you can ask for it by.
    pub calls: &'static [&'static str],
    text: &'static str,
}

/// Public-domain stories with water and islands in them (from Project
/// Gutenberg), one paragraph a line.
pub const BOOKS: &[Book] = &[
    Book {
        title: "Treasure Island",
        author: "Robert Louis Stevenson",
        calls: &["treasure", "pirate", "stevenson"],
        text: include_str!("../typing/treasure-island.txt"),
    },
    Book {
        title: "The Wind in the Willows",
        author: "Kenneth Grahame",
        calls: &["wind", "willows", "toad", "mole", "ratty", "grahame"],
        text: include_str!("../typing/the-wind-in-the-willows.txt"),
    },
    Book {
        title: "Three Men in a Boat",
        author: "Jerome K. Jerome",
        calls: &["three men", "boat", "jerome"],
        text: include_str!("../typing/three-men-in-a-boat.txt"),
    },
    Book {
        title: "Robinson Crusoe",
        author: "Daniel Defoe",
        calls: &["robinson", "crusoe", "defoe"],
        text: include_str!("../typing/robinson-crusoe.txt"),
    },
    Book {
        title: "Alice's Adventures in Wonderland",
        author: "Lewis Carroll",
        calls: &["alice", "wonderland", "carroll"],
        text: include_str!("../typing/alice.txt"),
    },
];

pub fn is_typewriter(name: &str) -> bool {
    name.to_lowercase().contains("typewriter")
}

/// Where a page starts: book, paragraph, sentence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Place {
    pub book: usize,
    pub para: usize,
    pub sentence: usize,
}

impl Place {
    fn save(&self) -> String {
        format!("{} {} {}", self.book, self.para, self.sentence)
    }

    fn load(s: &str) -> Option<Place> {
        let mut n = s.split(' ').map(|n| n.parse::<usize>().ok());
        let place = Place {
            book: n.next()??,
            para: n.next()??,
            sentence: n.next()??,
        };
        (place.book < BOOKS.len()).then_some(place)
    }
}

fn paragraphs(book: usize) -> Vec<&'static str> {
    BOOKS[book].text.lines().filter(|l| !l.is_empty()).collect()
}

/// A paragraph's sentences, with any very long one broken at a ; or :.
fn sentences(para: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let b = para.as_bytes();
    let mut start = 0;
    for i in 0..b.len() {
        let end = matches!(b[i], b'.' | b'!' | b'?')
            && b.get(i + 1)
                .is_some_and(|c| matches!(c, b' ' | b'"' | b'\''))
            && {
                // Past any closing quote, a space, then a capital or a quote.
                let mut j = i + 1;
                while matches!(b.get(j), Some(b'"' | b'\'')) {
                    j += 1;
                }
                b.get(j) == Some(&b' ')
                    && b.get(j + 1)
                        .is_some_and(|c| c.is_ascii_uppercase() || matches!(c, b'"' | b'\''))
            };
        if end {
            let mut j = i + 1;
            while matches!(b.get(j), Some(b'"' | b'\'')) {
                j += 1;
            }
            out.push(para[start..j].trim());
            start = j;
        }
    }
    if start < para.len() {
        out.push(para[start..].trim());
    }
    out.into_iter()
        .filter(|s| !s.is_empty())
        .flat_map(|s| split_long(s, &["; ", ": ", " - "]))
        .flat_map(|s| split_long(s, &[", "]))
        .filter(|s| !s.is_empty())
        .collect()
}

/// A very long sentence, broken after any of `at` into pieces of a decent size.
fn split_long<'a>(s: &'a str, at: &[&str]) -> Vec<&'a str> {
    if s.len() <= LONGEST {
        return vec![s];
    }
    let mut parts = Vec::new();
    let mut from = 0;
    for (i, _) in s.char_indices() {
        if let Some(sep) = at.iter().find(|sep| s[i..].starts_with(**sep))
            && i - from > PAGE / 2
        {
            let cut = i + sep.trim_end().len();
            parts.push(s[from..cut].trim());
            from = cut;
        }
    }
    parts.push(s[from..].trim());
    parts.retain(|p| !p.is_empty());
    parts
}

/// The page starting at `at`, and where the one after it starts.
pub fn page(at: Place) -> (String, Place) {
    let paras = paragraphs(at.book);
    let mut place = at;
    if place.para >= paras.len() {
        place = Place {
            book: at.book,
            para: 0,
            sentence: 0,
        };
    }
    let mut text = String::new();
    loop {
        let ss = sentences(paras[place.para]);
        if place.sentence >= ss.len() {
            place.para += 1;
            place.sentence = 0;
            if place.para >= paras.len() || !text.is_empty() && text.len() >= PAGE / 2 {
                // The end of the book (it starts again), or a paragraph's end
                // with a decent page already.
                if place.para >= paras.len() {
                    place.para = 0;
                }
                break;
            }
            continue;
        }
        let s = ss[place.sentence];
        if !text.is_empty() && text.len() + s.len() > PAGE + PAGE / 3 {
            break;
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(s);
        place.sentence += 1;
        if text.len() >= PAGE {
            break;
        }
    }
    (text, place)
}

/// The book these words ask for, if any.
fn book_called(words: &str) -> Option<usize> {
    let w = words.to_lowercase();
    BOOKS
        .iter()
        .position(|b| w.contains(&b.title.to_lowercase()) || b.calls.iter().any(|c| w.contains(c)))
}

fn key(user: &str) -> String {
    format!("typing:{user}")
}

impl Game {
    pub(super) fn typewriter_here(&self, x: i64, y: i64) -> bool {
        self.world
            .things_at(x, y)
            .any(|t| t.kind.as_deref() == Some(KIND))
    }

    pub(super) fn typewriter_command(
        &mut self,
        id: u64,
        verb: &str,
        rest: &str,
    ) -> Option<anyhow::Result<()>> {
        let r = rest.trim().to_lowercase();
        let words = match verb {
            "type" | "typing" | "typewriter" => r.as_str(),
            "sit" | "use" | "practise" | "practice"
                if r.contains("typewriter") || r.contains("typing") =>
            {
                ""
            }
            _ => return None,
        };
        Some(self.cmd_type(id, words))
    }

    fn cmd_type(&mut self, id: u64, words: &str) -> anyhow::Result<()> {
        let Some(p) = self.players.get(&id) else {
            return Ok(());
        };
        let (x, y, name, user, bot) = (p.x, p.y, p.name.clone(), p.user.clone(), p.bot);
        if !self.typewriter_here(x, y) {
            self.say1(
                id,
                Line::plain("There's no typewriter here. You could always make one."),
            );
            return Ok(());
        }
        if bot {
            self.say1(
                id,
                Line::plain("You tap a key or two. Clack. It's more fun for people with fingers."),
            );
            return Ok(());
        }
        let saved = self
            .world
            .db
            .meta(&key(&user))
            .and_then(|s| Place::load(&s));
        let words = words
            .trim_start_matches("on ")
            .trim_start_matches("some ")
            .trim();
        let fresh = || {
            let mut rng = crate::server::rng::Rng::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_nanos() as u64),
            );
            let book = (rng.next() % BOOKS.len() as u64) as usize;
            let para = (rng.next() % paragraphs(book).len() as u64) as usize;
            Place {
                book,
                para,
                sentence: 0,
            }
        };
        let at = if let Some(book) = book_called(words) {
            match saved {
                Some(p) if p.book == book => p,
                _ => Place {
                    book,
                    para: 0,
                    sentence: 0,
                },
            }
        } else if matches!(
            words,
            "something else" | "new" | "else" | "another book" | "afresh"
        ) {
            fresh()
        } else {
            saved.unwrap_or_else(fresh)
        };
        let (text, next) = page(at);
        self.world.db.set_meta(&key(&user), &next.save())?;
        let book = &BOOKS[at.book];
        if let Some(p) = self.players.get(&id) {
            let _ = p.tx.send(ToClient::Typewriter {
                title: book.title.to_string(),
                author: book.author.to_string(),
                text,
            });
        }
        self.say1(
            id,
            Line::new()
                .c(
                    Col::Sand,
                    "⌨️ You wind a fresh sheet into the typewriter: a page of ",
                )
                .b(Col::Sand, book.title)
                .c(
                    Col::Dim,
                    " (type the name of another book, or 'type something else', for a change)",
                ),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::new().who(&name).c(
                Col::Sand,
                " sits down at the typewriter. Clack, clack, clack... ding!",
            ),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_book_has_pages() {
        for (b, book) in BOOKS.iter().enumerate() {
            let paras = paragraphs(b);
            assert!(paras.len() > 300, "{}", book.title);
            assert!(book.text.is_ascii(), "{}", book.title);
            let (text, next) = page(Place {
                book: b,
                para: 0,
                sentence: 0,
            });
            assert!(
                text.len() >= 100 && text.len() < 900,
                "{}: {text}",
                book.title
            );
            assert_ne!(
                next,
                Place {
                    book: b,
                    para: 0,
                    sentence: 0
                }
            );
        }
    }

    #[test]
    fn pages_follow_on() {
        let mut at = Place {
            book: 1,
            para: 0,
            sentence: 0,
        };
        let mut all = String::new();
        for _ in 0..5 {
            let (text, next) = page(at);
            assert!(!text.is_empty());
            all.push_str(&text);
            all.push(' ');
            at = next;
        }
        assert!(all.starts_with("The Mole had been working very hard"));
        assert!(all.contains("spring-cleaning"));
    }

    #[test]
    fn walks_every_book_with_sensible_pages() {
        for (b, book) in BOOKS.iter().enumerate() {
            let mut at = Place {
                book: b,
                para: 0,
                sentence: 0,
            };
            let mut pages = 0;
            loop {
                let (text, next) = page(at);
                assert!(!text.is_empty());
                assert!(text.len() < 1000, "{}: {text}", book.title);
                pages += 1;
                if next.para < at.para {
                    break; // round to the start again
                }
                at = next;
                assert!(pages < 5000, "{} never ends", book.title);
            }
        }
    }

    #[test]
    fn sentences_split_after_quotes() {
        let s = sentences("\"Bother!\" said the Mole. \"Hang spring-cleaning!\" Up we go.");
        assert_eq!(
            s,
            [
                "\"Bother!\" said the Mole.",
                "\"Hang spring-cleaning!\"",
                "Up we go."
            ]
        );
    }

    #[test]
    fn finds_books_by_name() {
        assert_eq!(book_called("treasure island"), Some(0));
        assert_eq!(book_called("the one with toad"), Some(1));
        assert_eq!(book_called("alice"), Some(4));
        assert_eq!(book_called("something else"), None);
    }

    #[test]
    fn places_round_trip() {
        let p = Place {
            book: 2,
            para: 17,
            sentence: 3,
        };
        assert_eq!(Place::load(&p.save()), Some(p));
        assert_eq!(Place::load("99 1 1"), None);
        assert_eq!(Place::load("nonsense"), None);
    }
}
