//! Playing your own songs from the terminal, to a gramophone or a jukebox:
//! ffmpeg encodes each file as it plays (in real time, `-re`) to Opus in
//! Ogg, and the packets inside go to the world just as a browser's would
//! (stereo, 48kHz, 20ms each, a few at a time).

use crate::sound::find;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// How to get ffmpeg, for the hint.
pub const INSTALL: &str = "brew install ffmpeg (or apt install ffmpeg)";
/// Files a jukebox keeps for each person.
pub const JUKEBOX_MAX: usize = 20;
/// A gramophone goes round however many songs, within reason.
const GRAMOPHONE_MAX: usize = 500;
/// The world takes at most 60 frames a message; the browser sends 10 or so
/// every 200ms.
const BATCH: usize = 50;
const SEND_EVERY: Duration = Duration::from_millis(200);
const AUDIO: &[&str] = &[
    "mp3", "m4a", "aac", "flac", "ogg", "opus", "wav", "aiff", "aif", "alac", "wma",
];

/// What the record player has to tell the client.
#[derive(Debug)]
pub enum Played {
    /// A song's begun: say so.
    Track { title: String },
    /// Music for the world.
    Frames { frames: Vec<String>, title: String },
    /// A jukebox song's finished (or couldn't play): the world moves on.
    Done { thing: i64, key: String },
    /// Something to tell you.
    Note(String),
}

/// The record player: one gramophone or jukebox turn at a time.
pub struct Records {
    ffmpeg: Option<PathBuf>,
    tx: mpsc::UnboundedSender<Played>,
    playing: Option<JoinHandle<()>>,
}

impl Records {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<Played>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Records {
                ffmpeg: find("ffmpeg"),
                tx,
                playing: None,
            },
            rx,
        )
    }

    pub fn can_play(&self) -> bool {
        self.ffmpeg.is_some()
    }

    /// Songs round and round on a gramophone, until stopped.
    pub fn gramophone(&mut self, files: Vec<PathBuf>) {
        self.start(files, None);
    }

    /// One song for your turn on a jukebox.
    pub fn jukebox(&mut self, file: PathBuf, thing: i64, key: String) {
        self.start(vec![file], Some((thing, key)));
    }

    pub fn stop(&mut self) {
        if let Some(h) = self.playing.take() {
            h.abort();
        }
    }

    fn start(&mut self, files: Vec<PathBuf>, turn: Option<(i64, String)>) {
        self.stop();
        let Some(ffmpeg) = self.ffmpeg.clone() else {
            let _ = self.tx.send(Played::Note(format!(
                "🎶 To play songs from the terminal: {INSTALL}, then world.txt play again."
            )));
            if let Some((thing, key)) = turn {
                let _ = self.tx.send(Played::Done { thing, key });
            }
            return;
        };
        let tx = self.tx.clone();
        self.playing = Some(tokio::spawn(async move {
            match turn {
                Some((thing, key)) => {
                    play(&ffmpeg, &files[0], &tx).await;
                    let _ = tx.send(Played::Done { thing, key });
                }
                None => loop {
                    let mut any = false;
                    for f in &files {
                        any |= play(&ffmpeg, f, &tx).await;
                    }
                    if !any {
                        let _ = tx.send(Played::Note(
                            "🎶 None of those would play. 'stop music' to stop.".into(),
                        ));
                        break;
                    }
                },
            }
        }));
    }
}

impl Drop for Records {
    fn drop(&mut self) {
        self.stop();
    }
}

/// The song's name, from its file.
pub fn title(path: &Path) -> String {
    path.file_stem().map_or_else(
        || "a song".to_string(),
        |s| s.to_string_lossy().chars().take(200).collect(),
    )
}

/// Play one file to the world; false if it wouldn't play at all.
async fn play(ffmpeg: &Path, file: &Path, tx: &mpsc::UnboundedSender<Played>) -> bool {
    let title = title(file);
    let child = Command::new(ffmpeg)
        .args(["-nostdin", "-loglevel", "error", "-re", "-i"])
        .arg(file)
        .args([
            "-vn",
            "-ac",
            "2",
            "-ar",
            "48000",
            "-c:a",
            "libopus",
            "-b:a",
            "96k",
            "-frame_duration",
            "20",
            "-f",
            "ogg",
            "pipe:1",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn();
    let Ok(mut child) = child else { return false };
    let Some(mut out) = child.stdout.take() else {
        return false;
    };
    let _ = tx.send(Played::Track {
        title: title.clone(),
    });
    let mut ogg = OggReader::default();
    let mut pending: Vec<String> = Vec::new();
    let mut sent = false;
    let mut tick = tokio::time::interval(SEND_EVERY);
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        tokio::select! {
            n = out.read(&mut buf) => match n {
                Ok(0) | Err(_) => break,
                Ok(n) => pending.extend(ogg.feed(&buf[..n]).iter().map(|p| base64(p))),
            },
            _ = tick.tick() => {
                while !pending.is_empty() {
                    let frames: Vec<String> = pending.drain(..pending.len().min(BATCH)).collect();
                    sent = true;
                    let _ = tx.send(Played::Frames { frames, title: title.clone() });
                }
            }
        }
    }
    if !pending.is_empty() {
        sent = true;
        let _ = tx.send(Played::Frames {
            frames: pending,
            title: title.clone(),
        });
    }
    let _ = child.wait().await;
    if !sent {
        let _ = tx.send(Played::Note(format!("🎶 Couldn't play {title}.")));
    }
    sent
}

/// The songs a few typed words mean: files, or folders of them, with `~`,
/// quotes and backslashed spaces (as dragging a file into a terminal gives).
pub fn songs(raw: &str, max: usize) -> Result<Vec<PathBuf>, String> {
    let raw = raw.trim();
    let whole = expand(raw);
    let words = if whole.exists() {
        vec![whole]
    } else {
        split(raw).iter().map(|w| expand(w)).collect()
    };
    let mut out = Vec::new();
    for w in words {
        if w.is_dir() {
            let mut inside: Vec<PathBuf> = std::fs::read_dir(&w)
                .map_err(|e| format!("Couldn't look in {}: {e}", w.display()))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| is_audio(p))
                .collect();
            inside.sort();
            out.extend(inside);
        } else if w.is_file() {
            out.push(w);
        } else {
            return Err(format!("There's nothing at {}.", w.display()));
        }
    }
    if out.is_empty() {
        return Err("No songs there.".into());
    }
    out.truncate(max.min(GRAMOPHONE_MAX));
    Ok(out)
}

fn is_audio(p: &Path) -> bool {
    p.is_file()
        && p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| AUDIO.contains(&e.to_lowercase().as_str()))
}

fn expand(w: &str) -> PathBuf {
    match w.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map_or_else(|| PathBuf::from(w), |h| PathBuf::from(h).join(rest)),
        None => PathBuf::from(w),
    }
}

/// Words, shell-ish: quotes keep spaces, a backslash keeps the next character.
fn split(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match (c, quote) {
            ('\\', _) => cur.extend(chars.next()),
            ('"' | '\'', None) => quote = Some(c),
            (q, Some(open)) if q == open => quote = None,
            (c, None) if c.is_whitespace() => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            (c, _) => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= chunk.len() {
                ABC[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

/// Packets out of an Ogg stream, as its bytes arrive. The first two in each
/// Opus stream are its headers, which the world doesn't want.
#[derive(Default)]
struct OggReader {
    buf: Vec<u8>,
    packet: Vec<u8>,
    seen: usize,
}

impl OggReader {
    fn feed(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        loop {
            if self.buf.len() < 27 {
                break;
            }
            if &self.buf[..4] != b"OggS" {
                // Lost our place: find the next page.
                match self.buf.windows(4).skip(1).position(|w| w == b"OggS") {
                    Some(i) => {
                        self.buf.drain(..i + 1);
                        continue;
                    }
                    None => {
                        let keep = self.buf.len().saturating_sub(3);
                        self.buf.drain(..keep);
                        break;
                    }
                }
            }
            let segs = self.buf[26] as usize;
            if self.buf.len() < 27 + segs {
                break;
            }
            let lacing = self.buf[27..27 + segs].to_vec();
            let body: usize = lacing.iter().map(|&l| l as usize).sum();
            if self.buf.len() < 27 + segs + body {
                break;
            }
            if self.buf[5] & 0x02 != 0 {
                // A new stream begins: headers again.
                self.seen = 0;
                self.packet.clear();
            }
            let mut at = 27 + segs;
            for l in lacing {
                self.packet
                    .extend_from_slice(&self.buf[at..at + l as usize]);
                at += l as usize;
                if l < 255 {
                    let p = std::mem::take(&mut self.packet);
                    self.seen += 1;
                    if self.seen > 2 {
                        out.push(p);
                    }
                }
            }
            self.buf.drain(..at);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_base64() {
        assert_eq!(base64(b"hello"), "aGVsbG8=");
        assert_eq!(base64(b"hi"), "aGk=");
        assert_eq!(base64(&[0, 1, 2, 255]), "AAEC/w==");
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn reads_back_what_the_listener_writes() {
        let mut w = crate::sound::Ogg::new(3);
        let mut bytes = w.headers();
        let packets: Vec<Vec<u8>> = vec![vec![1; 10], vec![2; 255], vec![3; 600], vec![]];
        for p in &packets {
            bytes.extend(w.packet(p));
        }
        let mut r = OggReader::default();
        // In awkward pieces, as a pipe gives them.
        let mut got = Vec::new();
        for chunk in bytes.chunks(7) {
            got.extend(r.feed(chunk));
        }
        assert_eq!(got, packets, "headers dropped, packets whole");
    }

    #[test]
    fn words_like_a_shell() {
        assert_eq!(split(r#"a b\ c "d e" 'f g'"#), ["a", "b c", "d e", "f g"]);
        assert_eq!(split("  "), Vec::<String>::new());
    }

    #[test]
    fn songs_from_files_and_folders() {
        let dir = std::env::temp_dir().join(format!("world-records-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Album One")).unwrap();
        for f in [
            "Album One/02 b.mp3",
            "Album One/01 a.flac",
            "Album One/cover.jpg",
            "A Little Tune.mp3",
        ] {
            std::fs::write(dir.join(f), b"").unwrap();
        }
        let d = dir.display();
        let names = |v: Vec<PathBuf>| {
            v.iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(songs(&format!("{d}/Album One"), 20).unwrap()),
            ["01 a.flac", "02 b.mp3"],
            "a folder's songs, in order"
        );
        assert_eq!(
            names(songs(&format!("{d}/A Little Tune.mp3"), 20).unwrap()),
            ["A Little Tune.mp3"],
            "spaces without quoting"
        );
        assert_eq!(
            names(songs(&format!(r"{d}/A\ Little\ Tune.mp3 '{d}/Album One'"), 20).unwrap()).len(),
            3
        );
        assert_eq!(
            songs(&format!("{d}/Album One"), 1).unwrap().len(),
            1,
            "within the limit"
        );
        assert!(songs(&format!("{d}/nope.mp3"), 20).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
