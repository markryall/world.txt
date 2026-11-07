//! Old tunes to hum along with, so nobody has to type the notes (on a phone,
//! `F#5- | D5/` means three keyboards). `hum along canon bass`, `hum along
//! frère jacques`; plain `hum along` lists them as taps.

/// One part of a song: what someone hums.
pub struct SongPart {
    pub name: &'static str,
    /// When to come in, or what it's for.
    pub about: &'static str,
    /// The tune, as typed for `hum along`: `|` between sections.
    pub tune: fn() -> String,
}

pub struct Song {
    pub title: &'static str,
    /// What it's called in a command (as typed, lower case, no accents).
    pub names: &'static [&'static str],
    pub parts: &'static [SongPart],
}

/// Pachelbel's Canon: the violin's ten sections, bars 3 to 22 with every
/// note held twice as long (as in docs/playing.md).
pub const CANON: [&str; 10] = [
        "F#5- E5- D5- C#5- B4- A4- B4- C#5-",
        "D5- C#5- B4- A4- G4- F#4- G4- E4-",
        "D4 F#4 A4 G4 F#4 D4 F#4 E4 D4 B3 D4 A4 G4 B4 A4 G4",
        "F#4 D4 E4 C#5 D5 F#5 A5 A4 B4 G4 A4 F#4 D4 D5 D5. C#5/",
        "D5/ C#5/ D5/ D4/ C#4/ A4/ E4/ F#4/ D4/ D5/ C#5/ B4/ C#5/ F#5/ A5/ B5/ G5/ F#5/ E5/ G5/ F#5/ E5/ D5/ C#5/ B4/ A4/ G4/ F#4/ E4/ G4/ F#4/ E4/",
        "D4/ E4/ F#4/ G4/ A4/ E4/ A4/ G4/ F#4/ B4/ A4/ G4/ A4/ G4/ F#4/ E4/ D4/ B3/ B4/ C#5/ D5/ C#5/ B4/ A4/ G4/ F#4/ E4/ B4/ A4/ B4/ A4/ G4/",
        "F#4 F#5 E5- _ D5 F#5- B5- A5- B5- C#6-",
        "D6 D5 C#5- _ B4 D5- D5-- D5 D5 G5 E5 A5",
        "A5/ F#5// G5// A5/ F#5// G5// A5// A4// B4// C#5// D5// E5// F#5// G5// F#5/ D5// E5// F#5/ F#4// G4// A4// B4// A4// G4// A4// F#4// G4// A4// G4/ B4// A4// G4/ F#4// E4// F#4// E4// D4// E4// F#4// G4// A4// B4// G4/ B4// A4// B4/ C#5// D5// A4// B4// C#5// D5// E5// F#5// G5// A5//",
        "F#5/ D5// E5// F#5/ E5// D5// E5// C#5// D5// E5// F#5// E5// D5// C#5// D5/ B4// C#5// D5/ D4// E4// F#4// G4// F#4// E4// F#4// D5// C#5// D5// B4/ D5// C#5// B4/ A4// G4// A4// G4// F#4// G4// A4// B4// C#5// D5// B4/ D5// C#5// D5/ C#5// B4// C#5// D5// E5// D5// C#5// D5// B4// C#5//",
];

fn canon_bass() -> String {
    "@110 D3- A2- B2- F#2- G2- D2- G2- A2-".to_string()
}
fn canon_tune() -> String {
    format!("@110 {}", CANON.join(" | "))
}
fn frere() -> String {
    "@120 C D E C C D E C | E F G- E F G- | G/ A/ G/ F/ E C G/ A/ G/ F/ E C | C G3 C- C G3 C-".to_string()
}
fn twinkle() -> String {
    "@110 C C G G A A G- F F E E D D C-".to_string()
}
fn ode() -> String {
    "@120 E E F G G F E D C C D E E. D/ D- E E F G G F E D C C D E D. C/ C-".to_string()
}
fn jingle() -> String {
    "@160 E E E- E E E- E G C. D/ E--- F F F. F/ F E E E/ E/ E D D E D- G-".to_string()
}
fn mary() -> String {
    "@120 E D C D E E E- D D D- E G G- E D C D E E E E D D E D C---".to_string()
}

pub static SONGS: &[Song] = &[
    Song {
        title: "Pachelbel's Canon",
        names: &["canon", "pachelbel's canon", "pachelbels canon", "pachelbel"],
        parts: &[
            SongPart { name: "bass", about: "the ground: start with this", tune: canon_bass },
            SongPart { name: "tune", about: "ten sections, each busier than the last: come in a turn after the one before", tune: canon_tune },
        ],
    },
    Song {
        title: "Frère Jacques",
        names: &["frere jacques", "frere", "brother john"],
        parts: &[SongPart { name: "round", about: "everyone hums it, each a turn after the last", tune: frere }],
    },
    Song {
        title: "Twinkle, Twinkle, Little Star",
        names: &["twinkle twinkle little star", "twinkle twinkle", "twinkle"],
        parts: &[SongPart { name: "tune", about: "", tune: twinkle }],
    },
    Song { title: "Ode to Joy", names: &["ode to joy", "ode"], parts: &[SongPart { name: "tune", about: "", tune: ode }] },
    Song { title: "Jingle Bells", names: &["jingle bells", "jingle"], parts: &[SongPart { name: "tune", about: "", tune: jingle }] },
    Song {
        title: "Mary Had a Little Lamb",
        names: &["mary had a little lamb", "mary"],
        parts: &[SongPart { name: "tune", about: "", tune: mary }],
    },
];

/// Lower case, without accents or punctuation, one space between words.
pub fn plain(s: &str) -> String {
    let s: String = s
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c if c.is_alphanumeric() || c == '\'' => c,
            _ => ' ',
        })
        .collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What someone asked the songbook for.
pub enum Found {
    /// The part of a song to hum.
    Part(&'static SongPart),
    /// A song with several parts, and no (or no such) part named.
    Song(&'static Song),
}

/// `canon bass`, `Frère Jacques`, `twinkle`: a song from the songbook, and
/// which part. `None` if it isn't one (so it's notes).
pub fn find(asked: &str) -> Option<Found> {
    let asked = plain(asked);
    // The longest name that fits, so "twinkle twinkle" beats "twinkle".
    let (song, rest) = SONGS
        .iter()
        .flat_map(|s| s.names.iter().map(move |n| (s, *n)))
        .filter(|(_, n)| asked == *n || asked.starts_with(&format!("{n} ")))
        .max_by_key(|(_, n)| n.len())
        .map(|(s, n)| (s, asked[n.len()..].trim().to_string()))?;
    if song.parts.len() == 1 {
        return Some(Found::Part(&song.parts[0]));
    }
    match song.parts.iter().find(|p| !rest.is_empty() && p.name.starts_with(&rest)) {
        Some(p) => Some(Found::Part(p)),
        None => Some(Found::Song(song)),
    }
}

/// The command that hums a part, for a tap.
pub fn command(song: &Song, part: &SongPart) -> String {
    let name = song.names[0];
    if song.parts.len() == 1 { format!("hum along {name}") } else { format!("hum along {name} {}", part.name) }
}

/// The most songs one person can keep.
pub const MAX_SONGS: usize = 24;
/// The longest a song's name can be.
pub const MAX_NAME: usize = 40;

/// `learn twinkle C C G G A A G-`: the name (the words before the notes)
/// and the tune, tidied, keeping its sections.
pub fn name_and_tune(rest: &str) -> (String, String) {
    let words: Vec<&str> = rest.split_whitespace().collect();
    let at = words
        .iter()
        .position(|w| w.starts_with('|') || super::music::clean_word(w.trim_end_matches(',')).is_some())
        .unwrap_or(words.len());
    let name = words[..at].join(" ");
    let tune = words[at..]
        .join(" ")
        .split('|')
        .map(super::music::clean_tune_long)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    (name, tune)
}

/// What a song name asks for.
pub enum Asked {
    /// A tune to hum.
    Tune(String),
    /// A songbook song with parts to choose from.
    Choose(&'static Song),
}

impl super::Game {
    /// A song of yours, or from the songbook, by name; `None` if it's notes.
    pub(super) fn song_asked(&self, user: &str, asked: &str) -> Option<Asked> {
        let want = plain(asked);
        if let Some((_, tune)) = self.world.db.songs(user).ok()?.into_iter().find(|(n, _)| plain(n) == want) {
            return Some(Asked::Tune(tune));
        }
        match find(asked)? {
            Found::Part(part) => Some(Asked::Tune((part.tune)())),
            Found::Song(song) => Some(Asked::Choose(song)),
        }
    }

    /// `learn twinkle C C G G A A G-`: keep a tune to hum by name. Bare,
    /// list yours.
    pub(super) fn cmd_learn(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        use crate::protocol::{Col, Line};
        let user = self.user(id);
        let mine = self.world.db.songs(&user)?;
        if rest.trim().is_empty() {
            let mut lines = vec![Line::col(Col::Pink, if mine.is_empty() {
                "🎶 You haven't learned any songs yet. 'learn twinkle C C G G A A G-' keeps one, to hum again by name."
            } else {
                "🎶 Your songs (tap one to hum along with it):"
            })];
            for (name, tune) in &mine {
                lines.push(Line::new().t("  ").run(Col::Pink, name, format!("hum along {name}")).c(Col::Dim, format!("  {tune}")));
            }
            if !mine.is_empty() {
                lines.push(Line::col(Col::Dim, "  ('forget twinkle' lets one go)"));
            }
            self.send(id, lines);
            return Ok(());
        }
        let (name, tune) = name_and_tune(rest);
        let msg = if name.is_empty() {
            "Learn it as what? Give it a name before the notes: 'learn twinkle C C G G A A G-'.".to_string()
        } else if tune.is_empty() {
            format!("Learn which notes as {name}? e.g. 'learn {name} C C G G A A G-'.")
        } else if name.chars().count() > MAX_NAME {
            format!("That's a long name! {MAX_NAME} letters at most.")
        } else if find(&name).is_some() {
            format!("{name} is in the songbook already: 'hum along {name}'. Call yours something else?")
        } else if tune.len() > 4000 {
            "That's a very long song! Try it in two.".to_string()
        } else {
            let same = mine.iter().find(|(n, _)| plain(n) == plain(&name)).map(|(n, _)| n.clone());
            if same.is_none() && mine.len() >= MAX_SONGS {
                format!("You know {MAX_SONGS} songs already: 'forget' one first.")
            } else {
                let name = same.unwrap_or(name);
                self.world.db.learn_song(&user, &name, &tune)?;
                self.say1(id, Line::new().c(Col::Pink, "🎶 You learn ").run(Col::Pink, &name, format!("hum along {name}")).c(Col::Pink, format!(". 'hum along {name}' (or 'hum {name}') hums it.")));
                return Ok(());
            }
        };
        self.say1(id, Line::plain(msg));
        Ok(())
    }

    /// `forget twinkle`.
    pub(super) fn cmd_forget(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        use crate::protocol::{Col, Line};
        let user = self.user(id);
        let want = plain(rest);
        let found = self.world.db.songs(&user)?.into_iter().find(|(n, _)| plain(n) == want);
        match found {
            Some((name, _)) => {
                self.world.db.forget_song(&user, &name)?;
                self.say1(id, Line::col(Col::Pink, format!("🎶 You let {name} drift out of your head.")));
            }
            None => self.say1(id, Line::plain("You don't know a song called that. 'learn' lists yours.")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn songs_are_found_by_name_and_part() {
        let part = |s: &str| match find(s) {
            Some(Found::Part(p)) => {
                let song = SONGS.iter().find(|s| s.parts.iter().any(|q| std::ptr::eq(q, p))).unwrap();
                format!("{}: {}", song.title, p.name)
            }
            Some(Found::Song(song)) => song.title.to_string(),
            None => "-".to_string(),
        };
        assert_eq!(part("canon bass"), "Pachelbel's Canon: bass");
        assert_eq!(part("Pachelbel's Canon tune"), "Pachelbel's Canon: tune");
        assert_eq!(part("canon t"), "Pachelbel's Canon: tune");
        assert_eq!(part("canon"), "Pachelbel's Canon");
        assert_eq!(part("canon kazoo"), "Pachelbel's Canon");
        assert_eq!(part("Frère Jacques"), "Frère Jacques: round");
        assert_eq!(part("frere jacques"), "Frère Jacques: round");
        assert_eq!(part("Twinkle, twinkle"), "Twinkle, Twinkle, Little Star: tune");
        assert_eq!(part("C E G"), "-");
        assert_eq!(part("canonical"), "-");
        assert_eq!(part("ode"), "Ode to Joy: tune");
    }

    #[test]
    fn learning_reads_the_name_before_the_notes() {
        assert_eq!(name_and_tune("twinkle C C G G A A G-"), ("twinkle".into(), "C C G G A A G-".into()));
        assert_eq!(name_and_tune("ode to joy @120 E E F G"), ("ode to joy".into(), "@120 E E F G".into()));
        assert_eq!(name_and_tune("my round C D | E F"), ("my round".into(), "C D | E F".into()));
        assert_eq!(name_and_tune("lullaby"), ("lullaby".into(), "".into()));
        assert_eq!(name_and_tune("C E G"), ("".into(), "C E G".into()));
    }

    #[test]
    fn the_canon_is_the_one_in_playing_md() {
        let doc = include_str!("../../../docs/playing.md");
        for (i, section) in CANON.iter().enumerate() {
            assert!(doc.contains(&format!("`{section}`")), "section {} isn't in playing.md", i + 1);
        }
    }

    #[test]
    fn every_part_fits_a_circle_started_by_its_first_part() {
        for song in SONGS {
            let first = (song.parts[0].tune)();
            let circle = super::super::circle::Circle::new(std::time::Instant::now(), &first).unwrap();
            for p in song.parts {
                assert!(circle.split(&(p.tune)()).is_ok(), "{}: {}", song.title, p.name);
            }
        }
    }
}
