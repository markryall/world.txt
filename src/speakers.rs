//! Everything a client does with sound, in one place, so the terminal client
//! and `world.txt sound` (Emacs's helper) sound the same: hums and circles
//! through the synth, radios and gramophones through mpv, the island's own
//! sounds, and songs from this computer for gramophones and jukeboxes.
//!
//! The client hands over what the world says (`heard`), what's typed
//! (`typed`) and what the record player has done (`played`), and gets back
//! lines to show and messages for the world.

use crate::protocol::{Col, Line, Song, ToClient, ToServer};
use crate::records::{Played, Records};
use crate::sound::{Heard, Sound};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Something for the client to do.
#[derive(Debug)]
pub enum Out {
    /// Show this line.
    Say(Line),
    /// Send this to the world.
    Send(ToServer),
}

pub struct Speakers {
    sound: Sound,
    records: Records,
    deck: Deck,
    /// Music nearby has said how to hear it.
    told: bool,
    /// How to come back after installing something ("world.txt play again").
    again: &'static str,
}

/// The record player's side of things: which gramophone or jukebox you're
/// at, and the songs waiting to go on.
#[derive(Default)]
struct Deck {
    /// The gramophone you've wound up.
    spin: Option<i64>,
    want_play: Option<Vec<PathBuf>>,
    /// The jukebox you last added songs to, and what you've queued, by key.
    jukebox: Option<i64>,
    want_queue: Option<Vec<Song>>,
    queued: HashMap<String, PathBuf>,
}

/// Typed commands handled here rather than by the world, at least to begin with.
#[derive(Debug, PartialEq)]
enum Local {
    /// play music ~/Music/Rumours
    PlayMusic(String),
    /// add songs a.mp3 b.mp3
    AddSongs(String),
    /// sound on, sound off, or just sound to flip it.
    Sound(Option<bool>),
}

fn pink(text: impl Into<String>) -> Out {
    Out::Say(Line::col(Col::Pink, text))
}

impl Speakers {
    /// Sound starts off unless `on`. The receiver brings what the record
    /// player does, for `played`.
    pub fn new(on: bool, again: &'static str) -> (Self, mpsc::UnboundedReceiver<Played>, Vec<Out>) {
        let (records, played) = Records::new();
        let mut s = Speakers {
            sound: Sound::new(),
            records,
            deck: Deck::default(),
            told: false,
            again,
        };
        let mut out = Vec::new();
        if on {
            s.set_sound(Some(true), &mut out, false);
        }
        (s, played, out)
    }

    fn set_sound(&mut self, on: Option<bool>, out: &mut Vec<Out>, say_ok: bool) {
        let on = on.unwrap_or(!self.sound.is_on());
        match self.sound.set_on(on) {
            Ok(()) if !say_ok => {}
            Ok(()) if on => out.push(pink(
                "🔊 Sound on: music, hums and the island's own sounds. 'sound off' to stop.",
            )),
            Ok(()) => out.push(pink("🔇 Sound off.")),
            Err(_) => out.push(pink(format!(
                "🔇 Sound needs mpv: {}, then {}.",
                crate::sound::INSTALL,
                self.again
            ))),
        }
    }

    /// Something typed. None means it's for the world, as it is.
    pub fn typed(&mut self, text: &str) -> Option<Vec<Out>> {
        let mut out = Vec::new();
        let to_world = match local(text)? {
            Local::Sound(on) => {
                self.set_sound(on, &mut out, true);
                None
            }
            Local::PlayMusic(paths) => self.play_music(&paths, &mut out),
            Local::AddSongs(paths) => self.add_songs(&paths, &mut out),
        };
        out.extend(to_world.map(Out::Send));
        Some(out)
    }

    /// Something the world said.
    pub fn heard(&mut self, msg: &ToClient) -> Vec<Out> {
        let mut out = Vec::new();
        match msg {
            ToClient::Lines { lines } => {
                for l in lines {
                    if let Some(tune) = hummed(l) {
                        self.sound.tune(tune);
                    }
                }
            }
            ToClient::Status { sounds, .. } => self.sound.levels(*sounds),
            ToClient::Circle { parts, .. } => {
                for p in parts {
                    self.sound.tune(&p.tune);
                }
            }
            ToClient::Radio { thing, url, gain } => {
                let heard = self.sound.radio(*thing, url.as_deref(), *gain);
                self.tell(heard, "📻 There's a radio playing nearby.", &mut out);
            }
            ToClient::Audio {
                thing,
                frames,
                gain,
                ..
            } => {
                let heard = self.sound.audio(*thing, frames, *gain);
                self.tell(heard, "🎶 There's music playing nearby.", &mut out);
            }
            ToClient::AudioEnd { thing } => self.sound.end(*thing),
            // Playing your own songs: you're the record player.
            ToClient::SpinStart { thing } => {
                self.deck.spin = Some(*thing);
                match self.deck.want_play.take() {
                    Some(files) => self.records.gramophone(files),
                    None => out.push(pink("🎶 Which songs? play music <a file or a folder> (dragging them in works too)")),
                }
            }
            ToClient::SpinStop { thing } if self.deck.spin == Some(*thing) => {
                self.deck.spin = None;
                self.records.stop();
            }
            ToClient::JukeAdd { thing } => {
                self.deck.jukebox = Some(*thing);
                match self.deck.want_queue.take() {
                    Some(songs) => out.push(Out::Send(ToServer::Queue {
                        thing: *thing,
                        songs,
                    })),
                    None => out.push(pink(format!(
                        "📀 Which songs? add songs <files or a folder> (up to {})",
                        crate::records::JUKEBOX_MAX
                    ))),
                }
            }
            ToClient::JukeTurn { thing, key } => match self.deck.queued.remove(key) {
                Some(path) => self.records.jukebox(path, *thing, key.clone()),
                None => out.push(Out::Send(ToServer::TrackDone {
                    thing: *thing,
                    key: key.clone(),
                })),
            },
            ToClient::JukeStop { .. } => self.records.stop(),
            _ => {}
        }
        out
    }

    /// Something the record player did.
    pub fn played(&mut self, p: Played) -> Vec<Out> {
        vec![match p {
            Played::Track { title } => pink(format!("🎶 Playing {title}")),
            Played::Note(text) => pink(text),
            Played::Frames { frames, title } => Out::Send(ToServer::Audio {
                frames,
                title: Some(title),
            }),
            Played::Done { thing, key } => Out::Send(ToServer::TrackDone { thing, key }),
        }]
    }

    /// The first music you can't hear says how you could.
    fn tell(&mut self, heard: Heard, what: &str, out: &mut Vec<Out>) {
        if self.told || !matches!(heard, Heard::Off | Heard::NoPlayer) {
            return;
        }
        self.told = true;
        out.push(Out::Say(match heard {
            Heard::Off => Line::new()
                .c(Col::Pink, format!("{what} Type "))
                .b(Col::Pink, "sound on")
                .c(Col::Pink, " to hear it."),
            _ => Line::new()
                .c(Col::Pink, format!("{what} To hear it here: "))
                .b(Col::Pink, crate::sound::INSTALL)
                .c(Col::Pink, format!(", then {}.", self.again)),
        }));
    }

    /// `play music <songs>`: wind up the gramophone first if it isn't.
    fn play_music(&mut self, paths: &str, out: &mut Vec<Out>) -> Option<ToServer> {
        let files = self.songs(paths, 500, out)?;
        if self.deck.spin.is_some() {
            self.records.gramophone(files);
            return None;
        }
        self.deck.want_play = Some(files);
        Some(ToServer::Cmd {
            text: "play music".into(),
        })
    }

    /// `add songs <songs>`: only their titles go to the world.
    fn add_songs(&mut self, paths: &str, out: &mut Vec<Out>) -> Option<ToServer> {
        let files = self.songs(paths, crate::records::JUKEBOX_MAX, out)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let songs: Vec<Song> = files
            .into_iter()
            .enumerate()
            .map(|(i, f)| {
                let key = format!("t{stamp:x}{i}");
                let title = crate::records::title(&f);
                self.deck.queued.insert(key.clone(), f);
                Song { key, title }
            })
            .collect();
        match self.deck.jukebox {
            Some(thing) => Some(ToServer::Queue { thing, songs }),
            None => {
                self.deck.want_queue = Some(songs);
                Some(ToServer::Cmd {
                    text: "add songs".into(),
                })
            }
        }
    }

    fn songs(&self, paths: &str, max: usize, out: &mut Vec<Out>) -> Option<Vec<PathBuf>> {
        if !self.records.can_play() {
            out.push(pink(format!(
                "🎶 To play songs from here: {}, then {}.",
                crate::records::INSTALL,
                self.again
            )));
            return None;
        }
        match crate::records::songs(paths, max) {
            Ok(files) => Some(files),
            Err(e) => {
                out.push(pink(format!("🎶 {e}")));
                None
            }
        }
    }
}

fn local(text: &str) -> Option<Local> {
    let t = text.trim();
    match t.to_lowercase().as_str() {
        "sound" => return Some(Local::Sound(None)),
        "sound on" => return Some(Local::Sound(Some(true))),
        "sound off" => return Some(Local::Sound(Some(false))),
        _ => {}
    }
    let (verb, rest) = t.split_once(char::is_whitespace)?;
    let (what, paths) = rest.trim_start().split_once(char::is_whitespace)?;
    let (verb, what, paths) = (
        verb.to_lowercase(),
        what.to_lowercase(),
        paths.trim().to_string(),
    );
    if paths.is_empty() {
        return None;
    }
    match (verb.as_str(), what.as_str()) {
        ("play" | "wind", "music" | "songs" | "records") => Some(Local::PlayMusic(paths)),
        ("add" | "queue", "songs" | "music") => Some(Local::AddSongs(paths)),
        _ => None,
    }
}

/// A hum's tune, from the ♪ … ♪ in a line (as the browser finds it).
fn hummed(l: &Line) -> Option<&str> {
    let text = l.0.iter().find(|s| s.t.contains('♪'))?;
    let inner = text.t.split_once("♪ ")?.1;
    inner.rsplit_once(" ♪").map(|(t, _)| t)
}

/// `world.txt sound`: the speakers for a client that can't make sound itself
/// (Emacs's world-txt.el), over stdin and stdout, a line at a time.
///
/// In: whatever the world sends, as it sent it (JSON), or `typed <text>` for
/// something typed. Out: `say <line as JSON>` to show, and `send <JSON>` for
/// the world. Something typed always gets an answer: `send` with the command
/// as it was when it's for the world.
pub async fn helper(on: bool) -> anyhow::Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let (mut speakers, mut played, first) = Speakers::new(on, "M-x world-txt-reconnect");
    let mut stdout = tokio::io::stdout();
    let mut stdin = BufReader::new(tokio::io::stdin()).lines();
    write(&mut stdout, first).await?;
    loop {
        let outs = tokio::select! {
            line = stdin.next_line() => {
                let Some(line) = line? else { break };
                match line.strip_prefix("typed ") {
                    Some(text) => speakers.typed(text).unwrap_or_else(|| vec![Out::Send(ToServer::Cmd { text: text.to_string() })]),
                    None => match serde_json::from_str::<ToClient>(&line) {
                        Ok(msg) => speakers.heard(&msg),
                        Err(_) => Vec::new(),
                    },
                }
            }
            Some(p) = played.recv() => speakers.played(p),
        };
        write(&mut stdout, outs).await?;
    }
    return Ok(());

    async fn write(stdout: &mut tokio::io::Stdout, outs: Vec<Out>) -> anyhow::Result<()> {
        for o in outs {
            let line = match o {
                Out::Say(l) => format!("say {}\n", serde_json::to_string(&l)?),
                Out::Send(m) => format!("send {}\n", serde_json::to_string(&m)?),
            };
            stdout.write_all(line.as_bytes()).await?;
        }
        stdout.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn songs_from_here_stay_here() {
        assert_eq!(
            local("play music ~/Music/Rumours"),
            Some(Local::PlayMusic("~/Music/Rumours".into()))
        );
        assert_eq!(
            local("Play  songs a.mp3 b.mp3"),
            Some(Local::PlayMusic("a.mp3 b.mp3".into()))
        );
        assert_eq!(
            local("add songs ~/Music/A\\ Little\\ Tune.mp3"),
            Some(Local::AddSongs("~/Music/A\\ Little\\ Tune.mp3".into()))
        );
        assert_eq!(local("play music"), None, "plain, it's for the world");
        assert_eq!(local("add songs"), None);
        assert_eq!(local("add Sam to picnic"), None);
        assert_eq!(local("play"), None);
        assert_eq!(local("Sound On"), Some(Local::Sound(Some(true))));
        assert_eq!(local("sound"), Some(Local::Sound(None)));
        assert_eq!(local("sound of the sea"), None);
    }

    #[test]
    fn hums_are_found_in_lines() {
        let l = Line::new()
            .b(Col::Cyan, "Sam")
            .c(Col::Dim, " hums ")
            .c(Col::Pink, "♪ @90 C E G- ♪");
        assert_eq!(hummed(&l), Some("@90 C E G-"));
        assert_eq!(
            hummed(&Line::col(Col::Pink, "♪ You hear someone hums nearby.")),
            None
        );
        assert_eq!(hummed(&Line::plain("hello")), None);
    }

    #[tokio::test]
    async fn a_jukebox_turn_we_cant_play_goes_straight_back() {
        let (mut s, _played, _) = Speakers::new(false, "again");
        let out = s.heard(&ToClient::JukeTurn {
            thing: 7,
            key: "k".into(),
        });
        assert!(
            matches!(&out[..], [Out::Send(ToServer::TrackDone { thing: 7, key })] if key == "k"),
            "{out:?}"
        );
    }

    #[tokio::test]
    async fn music_nearby_says_once_how_to_hear_it() {
        let (mut s, _played, _) = Speakers::new(false, "again");
        let radio = ToClient::Radio {
            thing: 1,
            url: Some("https://radio.example/stream".into()),
            gain: 0.5,
        };
        assert_eq!(s.heard(&radio).len(), 1);
        assert!(s.heard(&radio).is_empty());
    }
}
