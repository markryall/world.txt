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
fn plain(s: &str) -> String {
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
