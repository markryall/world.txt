//! Music for the terminal client, through mpv: the radio's own stream, and
//! gramophone or jukebox songs, which arrive as Opus packets (stereo, 48kHz,
//! 20ms each, as the player's browser encodes them) and are wrapped in Ogg
//! here so mpv can play them from its stdin. Volume follows you about over
//! mpv's control socket. Without mpv on the PATH it's all quiet, and the
//! first music nearby says how to hear it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

/// How to get a player, for the hint.
pub const INSTALL: &str = "brew install mpv (or apt install mpv)";

/// One stream being played: a radio, or a gramophone's songs.
struct Playing {
    _child: Child,
    sock: PathBuf,
    /// The radio's address, so a new station starts a new stream.
    url: Option<String>,
    /// Ogg pages on their way to mpv's stdin (gramophones only).
    pages: Option<mpsc::UnboundedSender<Vec<u8>>>,
    ogg: Ogg,
    volume: u32,
}

impl Drop for Playing {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.sock);
    }
}

pub struct Sound {
    mpv: Option<PathBuf>,
    playing: HashMap<i64, Playing>,
}

/// What happened, for the client to tell you.
#[derive(Debug, PartialEq)]
pub enum Heard {
    Playing,
    /// Music nearby, but nothing to play it with.
    NoPlayer,
    Nothing,
}

impl Sound {
    /// Look for mpv; `quiet` leaves the music be.
    pub fn new(quiet: bool) -> Self {
        Sound { mpv: if quiet { None } else { find("mpv") }, playing: HashMap::new() }
    }

    /// A radio in earshot (or, with no url, not any more).
    pub fn radio(&mut self, thing: i64, url: Option<&str>, gain: f32) -> Heard {
        let Some(url) = url else {
            self.playing.remove(&thing);
            return Heard::Nothing;
        };
        let Some(mpv) = self.mpv.clone() else { return Heard::NoPlayer };
        let volume = volume(gain);
        if let Some(p) = self.playing.get_mut(&thing).filter(|p| p.url.as_deref() == Some(url)) {
            set_volume(p, volume);
            return Heard::Playing;
        }
        self.playing.remove(&thing);
        if let Some(p) = start(&mpv, thing, Some(url), volume) {
            self.playing.insert(thing, p);
        }
        Heard::Playing
    }

    /// Frames from a gramophone (or jukebox) in earshot.
    pub fn audio(&mut self, thing: i64, frames: &[String], gain: f32) -> Heard {
        let Some(mpv) = self.mpv.clone() else { return Heard::NoPlayer };
        let volume = volume(gain);
        let p = match self.playing.entry(thing) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => {
                let Some(p) = start(&mpv, thing, None, volume) else { return Heard::Nothing };
                e.insert(p)
            }
        };
        set_volume(p, volume);
        let mut out = Vec::new();
        for f in frames {
            if let Some(packet) = unbase64(f) {
                out.extend(p.ogg.packet(&packet));
            }
        }
        if let Some(tx) = &p.pages
            && tx.send(out).is_err()
        {
            // mpv's gone (closed, or fell over): start afresh next time.
            self.playing.remove(&thing);
        }
        Heard::Playing
    }

    /// That gramophone's stopped.
    pub fn end(&mut self, thing: i64) {
        self.playing.remove(&thing);
    }
}

/// mpv's volume is a percentage on a cubic curve; the browser plays at
/// `gain * 0.8` of full.
fn volume(gain: f32) -> u32 {
    ((gain.clamp(0.0, 1.0) * 0.8).cbrt() * 100.0).round() as u32
}

pub fn find(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(name)).find(|p| p.is_file())
}

fn start(mpv: &Path, thing: i64, url: Option<&str>, volume: u32) -> Option<Playing> {
    let sock = std::env::temp_dir().join(format!("world-txt-{}-{thing}.sock", std::process::id()));
    let _ = std::fs::remove_file(&sock);
    let mut cmd = Command::new(mpv);
    cmd.args(["--no-video", "--no-terminal", "--idle=no", "--audio-display=no"])
        .arg(format!("--volume={volume}"))
        .arg(format!("--input-ipc-server={}", sock.display()))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    match url {
        Some(u) => {
            cmd.args(["--", u]).stdin(Stdio::null());
        }
        // From stdin, with no waiting to fill a cache: it's live.
        None => {
            cmd.args(["--cache=no", "--demuxer-lavf-format=ogg", "-"]).stdin(Stdio::piped());
        }
    }
    let mut child = cmd.spawn().ok()?;
    let pages = child.stdin.take().map(|mut stdin| {
        let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
        tokio::spawn(async move {
            while let Some(bytes) = rx.recv().await {
                if stdin.write_all(&bytes).await.is_err() {
                    break;
                }
            }
        });
        tx
    });
    let mut p = Playing { _child: child, sock, url: url.map(str::to_string), pages, ogg: Ogg::new(thing as u32), volume };
    if let Some(tx) = &p.pages {
        let _ = tx.send(p.ogg.headers());
    }
    Some(p)
}

fn set_volume(p: &mut Playing, volume: u32) {
    if p.volume == volume {
        return;
    }
    p.volume = volume;
    let sock = p.sock.clone();
    tokio::spawn(async move {
        if let Ok(mut s) = tokio::net::UnixStream::connect(&sock).await {
            let _ = s.write_all(format!("{{\"command\":[\"set_property\",\"volume\",{volume}]}}\n").as_bytes()).await;
        }
    });
}

fn unbase64(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    }
    let s = s.trim_end_matches('=').as_bytes();
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c)? << (18 - 6 * i);
        }
        let bytes = n.to_be_bytes();
        out.extend_from_slice(&bytes[1..chunk.len()]);
    }
    Some(out)
}

/// Just enough Ogg to carry an Opus stream (RFC 7845): two header pages, then
/// one page per packet.
pub struct Ogg {
    serial: u32,
    seq: u32,
    granule: u64,
}

/// Samples the decoder drops at the start, as libopus encoders use.
const PRE_SKIP: u16 = 312;
/// A 20ms packet at 48kHz.
const FRAME: u64 = 960;

impl Ogg {
    pub fn new(serial: u32) -> Self {
        Ogg { serial, seq: 0, granule: 0 }
    }

    pub fn headers(&mut self) -> Vec<u8> {
        let mut head = b"OpusHead".to_vec();
        head.push(1); // version
        head.push(2); // channels
        head.extend(PRE_SKIP.to_le_bytes());
        head.extend(48000u32.to_le_bytes());
        head.extend(0i16.to_le_bytes()); // output gain
        head.push(0); // mono/stereo mapping
        let mut tags = b"OpusTags".to_vec();
        let vendor = b"world.txt";
        tags.extend((vendor.len() as u32).to_le_bytes());
        tags.extend(vendor);
        tags.extend(0u32.to_le_bytes());
        let mut out = self.page(&head, 0x02, 0);
        out.extend(self.page(&tags, 0, 0));
        out
    }

    pub fn packet(&mut self, data: &[u8]) -> Vec<u8> {
        self.granule += FRAME;
        self.page(data, 0, self.granule)
    }

    fn page(&mut self, data: &[u8], flags: u8, granule: u64) -> Vec<u8> {
        // Lacing: 255s, then the remainder (a 0 when it's an exact multiple).
        let mut lacing = vec![255u8; data.len() / 255];
        lacing.push((data.len() % 255) as u8);
        let mut p = b"OggS".to_vec();
        p.push(0);
        p.push(flags);
        p.extend(granule.to_le_bytes());
        p.extend(self.serial.to_le_bytes());
        p.extend(self.seq.to_le_bytes());
        p.extend([0u8; 4]); // crc, filled in below
        p.push(lacing.len() as u8);
        p.extend(&lacing);
        p.extend(data);
        let crc = crc(&p);
        p[22..26].copy_from_slice(&crc.to_le_bytes());
        self.seq += 1;
        p
    }
}

/// Ogg's CRC-32 (polynomial 0x04c11db7, no reflection, no final xor).
fn crc(data: &[u8]) -> u32 {
    let mut c = 0u32;
    for &b in data {
        c ^= (b as u32) << 24;
        for _ in 0..8 {
            c = if c & 0x8000_0000 != 0 { (c << 1) ^ 0x04c1_1db7 } else { c << 1 };
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_follows_the_browser() {
        assert_eq!(volume(0.0), 0);
        assert_eq!(volume(1.0), 93, "0.8 of full, on mpv's cubic curve");
        assert!(volume(0.5) > volume(0.2));
        assert_eq!(volume(2.0), volume(1.0));
    }

    #[test]
    fn reads_base64() {
        assert_eq!(unbase64("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(unbase64("aGk=").unwrap(), b"hi");
        assert_eq!(unbase64("AAEC/w==").unwrap(), [0, 1, 2, 255]);
        assert!(unbase64("no!").is_none());
    }

    #[test]
    fn ogg_crc_matches_the_reference() {
        // The CRC of "123456789" for Ogg's variant (CRC-32/MPEG-2 without the
        // initial 0xffffffff), as libogg computes it.
        assert_eq!(crc(b"123456789"), 0x89a1_897f);
    }

    #[test]
    fn ogg_pages_count_up() {
        let mut o = Ogg::new(7);
        let h = o.headers();
        assert!(h.starts_with(b"OggS") && h[5] == 0x02, "the first page begins the stream");
        let p = o.packet(&[1u8; 300]);
        assert_eq!(&p[6..14], &960u64.to_le_bytes(), "granule counts samples");
        assert_eq!(&p[18..22], &2u32.to_le_bytes(), "third page");
        assert_eq!(&p[26..29], &[2, 255, 45], "300 bytes lace as 255 + 45");
        assert_eq!(p.len(), 29 + 300);
    }
}
