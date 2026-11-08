//! Hums and the sounds of the island, for the terminal client: the same
//! little synth the browser has (web/index.html, "sound: hums become notes"
//! and "sounds of the island"), made here as samples and played through one
//! mpv that stays open. Notes are triangle waves; rain, wind and waves are
//! filtered noise; thunder, birds, crickets and the fire's crackle are little
//! one-off sounds now and then. Nothing here is a sound file.

use crate::protocol::Sounds;
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const RATE: f32 = 48_000.0;
/// Samples made at a time: 10ms.
const BLOCK: usize = 480;
/// How far ahead of now to keep mpv fed: enough not to stutter, little
/// enough that a hum starts when it's sent.
const AHEAD: Duration = Duration::from_millis(120);
/// How loud each is at its loudest, as in the browser.
const RAIN: f32 = 0.12;
const WIND: f32 = 0.07;
const WAVES: f32 = 0.09;
const HEARTH: f32 = 0.025;
const THUNDER: f32 = 0.35;
const BIRDS: f32 = 0.035;
const CRICKETS: f32 = 0.012;
const FIRE: f32 = 0.1;
/// Notes in one tune, as the browser plays.
const MAX_NOTES: usize = 64;

/// The synth: hand it tunes and levels, and it plays them.
pub struct Synth {
    mix: Arc<Mutex<Mix>>,
    child: Child,
}

impl Synth {
    pub fn start(mpv: &Path) -> Option<Synth> {
        let mut child = Command::new(mpv)
            .args([
                "--no-video",
                "--no-terminal",
                "--idle=no",
                "--audio-display=no",
                "--cache=no",
                "--volume=100",
            ])
            .args([
                "--demuxer=rawaudio",
                "--demuxer-rawaudio-format=s16le",
                "--demuxer-rawaudio-rate=48000",
                "--demuxer-rawaudio-channels=1",
                "-",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut stdin = child.stdin.take()?;
        let mix = Arc::new(Mutex::new(Mix::new(seed())));
        let m = mix.clone();
        std::thread::spawn(move || {
            let begun = Instant::now();
            let mut made: u64 = 0;
            let mut block = [0f32; BLOCK];
            let mut bytes = Vec::with_capacity(BLOCK * 2);
            loop {
                let due = begun.elapsed() + AHEAD;
                if Duration::from_secs_f64(made as f64 / RATE as f64) > due {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                m.lock().map(|mut mix| mix.render(&mut block)).ok();
                bytes.clear();
                for s in block {
                    bytes.extend(((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
                }
                if stdin.write_all(&bytes).is_err() {
                    break;
                }
                made += BLOCK as u64;
            }
        });
        Some(Synth { mix, child })
    }

    /// Hum a tune: `@90 C E G-`, as the world writes them.
    pub fn tune(&self, text: &str) {
        if let Ok(mut m) = self.mix.lock() {
            m.tune(text);
        }
    }

    /// How much of each sound there is where you stand.
    pub fn levels(&self, s: Sounds) {
        if let Ok(mut m) = self.mix.lock() {
            m.levels = s;
        }
    }
}

impl Drop for Synth {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64)
        | 1
}

/// A tune's notes: (MIDI note, or None for a rest; beats), and the tempo.
/// The same reading as the browser's parseTune.
pub fn parse_tune(text: &str) -> (f32, Vec<(Option<i32>, f32)>) {
    let mut bpm = 120.0;
    let mut notes = Vec::new();
    for w in text
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|w| !w.is_empty())
    {
        if let Some(t) = w.strip_prefix('@')
            && t.len() <= 3
            && let Ok(n) = t.parse::<u32>()
        {
            bpm = n.clamp(40, 240) as f32;
            continue;
        }
        if let Some(n) = note(w) {
            notes.push(n);
        }
    }
    (bpm, notes)
}

fn note(w: &str) -> Option<(Option<i32>, f32)> {
    let mut cs = w.chars().peekable();
    let first = cs.next()?;
    let midi = if first == '_' {
        None
    } else {
        let semi = match first.to_ascii_lowercase() {
            'c' => 0,
            'd' => 2,
            'e' => 4,
            'f' => 5,
            'g' => 7,
            'a' => 9,
            'b' => 11,
            _ => return None,
        };
        let acc = match cs.peek() {
            Some('#') => {
                cs.next();
                1
            }
            Some('b') => {
                cs.next();
                -1
            }
            _ => 0,
        };
        let oct = match cs.peek() {
            Some(d @ '2'..='6') => {
                let o = *d as i32 - '0' as i32;
                cs.next();
                o
            }
            _ => 4,
        };
        Some(12 * (oct + 1) + semi + acc)
    };
    let rest: String = cs.collect();
    let holds = rest.chars().take_while(|&c| c == '-').count();
    let after = &rest[holds..];
    let halves = after.chars().take_while(|&c| c == '/').count();
    let tail = &after[halves..];
    if holds > 7 || halves > 3 || !(tail.is_empty() || tail == ".") {
        return None;
    }
    let beats = (1 + holds) as f32 / 2f32.powi(halves as i32) * if tail == "." { 1.5 } else { 1.0 };
    Some((midi, beats))
}

/// Everything sounding, made 10ms at a time.
struct Mix {
    rng: u64,
    /// Samples made so far: the synth's clock.
    now: u64,
    on: bool,
    levels: Sounds,
    voices: Vec<Voice>,
    rain: Bed,
    hearth: Bed,
    wind: Bed,
    waves: Bed,
    wind_next: u64,
    wave: Option<Wave>,
    waves_next: u64,
    thunder_next: Option<u64>,
    birds_next: u64,
    crickets_next: u64,
    fire_next: u64,
}

/// Noise through a filter, at a volume that drifts to where it's told.
struct Bed {
    filter: Biquad,
    gain: f32,
    target: f32,
    /// Seconds to get most of the way there.
    tau: f32,
}

impl Bed {
    fn new(filter: Biquad, tau: f32) -> Self {
        Bed {
            filter,
            gain: 0.0,
            target: 0.0,
            tau,
        }
    }
}

/// A wave rolling in and drawing back: from (gain, freq) to the peak over
/// `rise`, then down to 15% over `fall`.
struct Wave {
    start: u64,
    from: (f32, f32),
    peak: f32,
    rise: f32,
    fall: f32,
}

/// One little sound with its own end.
enum Voice {
    /// A hummed note: triangle wave, held, then let go.
    Note { start: u64, freq: f32, ring: f32 },
    /// A puff of filtered noise: crackles and thunder.
    Burst {
        start: u64,
        filter: Biquad,
        peak: f32,
        attack: f32,
        decay: f32,
    },
    /// A chirp, sliding up.
    Chirp {
        start: u64,
        from: f32,
        to: f32,
        len: f32,
        peak: f32,
    },
    /// A cricket's pulse.
    Pulse { start: u64, freq: f32, peak: f32 },
}

impl Mix {
    fn new(seed: u64) -> Self {
        Mix {
            rng: seed,
            now: 0,
            on: true,
            levels: Sounds::default(),
            voices: Vec::new(),
            rain: Bed::new(Biquad::lowpass(1400.0, 0.707), 1.5),
            hearth: Bed::new(Biquad::lowpass(400.0, 0.707), 1.0),
            wind: Bed::new(Biquad::bandpass(500.0, 0.7), 1.2),
            waves: Bed::new(Biquad::lowpass(700.0, 0.707), 0.5),
            wind_next: 0,
            wave: None,
            waves_next: 0,
            thunder_next: None,
            birds_next: 0,
            crickets_next: 0,
            fire_next: 0,
        }
    }

    fn random(&mut self) -> f32 {
        // xorshift: plenty for noise.
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 40) as f32 / (1u64 << 24) as f32
    }

    fn at(&self, secs: f32) -> u64 {
        self.now + (secs * RATE) as u64
    }

    fn tune(&mut self, text: &str) {
        if !self.on {
            return;
        }
        let (bpm, notes) = parse_tune(text);
        let beat = 60.0 / bpm;
        let mut t = 0.05;
        for (midi, beats) in notes.into_iter().take(MAX_NOTES) {
            let len = beats * beat;
            if let Some(m) = midi {
                let freq = 440.0 * 2f32.powf((m - 69) as f32 / 12.0);
                self.voices.push(Voice::Note {
                    start: self.at(t),
                    freq,
                    ring: (len * 0.9).max(0.08),
                });
            }
            t += len;
        }
    }

    fn level(&self, v: f32) -> f32 {
        if self.on { v.clamp(0.0, 1.0) } else { 0.0 }
    }

    /// What's due: gusts, waves, and the one-off sounds (the browser's
    /// four-times-a-second check, done each block here).
    fn schedule(&mut self) {
        let l = self.levels;
        self.rain.target = self.level(l.rain) * RAIN;
        self.hearth.target = self.level(l.fire) * HEARTH;
        let wind = self.level(l.wind);
        if wind == 0.0 {
            self.wind.target = 0.0;
            self.wind_next = 0;
        } else if self.now >= self.wind_next {
            self.wind.target = wind * WIND * (0.35 + self.random() * 0.65);
            let f = 300.0 + self.random() * 600.0;
            self.wind.filter = Biquad::bandpass(f, 0.7).keeping(&self.wind.filter);
            let gap = 2.0 + self.random() * 3.0;
            self.wind_next = self.at(gap);
        }
        let waves = self.level(l.waves);
        if waves == 0.0 {
            self.wave = None;
            self.waves.target = 0.0;
            self.waves_next = 0;
        } else if self.now >= self.waves_next {
            let peak = waves * WAVES * (0.6 + self.random() * 0.4);
            let (rise, fall) = (2.0 + self.random(), 4.0 + self.random() * 2.0);
            self.wave = Some(Wave {
                start: self.now,
                from: (self.waves.gain, 700.0),
                peak,
                rise,
                fall,
            });
            let gap = rise + fall + self.random() * 2.0;
            self.waves_next = self.at(gap);
        }
        // Thunder waits a while before its first rumble; the rest start soon.
        let thunder = self.level(l.thunder);
        if thunder == 0.0 {
            self.thunder_next = None;
        } else {
            match self.thunder_next {
                None => {
                    let gap = 20.0 + self.random() * 40.0;
                    self.thunder_next = Some(self.at(gap));
                }
                Some(t) if self.now >= t => {
                    self.rumble(thunder);
                    let gap = 20.0 + self.random() * 40.0;
                    self.thunder_next = Some(self.at(gap));
                }
                _ => {}
            }
        }
        let birds = self.level(l.birds);
        if birds > 0.0 && self.now >= self.birds_next {
            if self.birds_next > 0 {
                self.chirps(birds);
            }
            let gap = (1.5 + self.random() * 5.0) / birds;
            self.birds_next = self.at(gap);
        } else if birds == 0.0 {
            self.birds_next = 0;
        }
        let crickets = self.level(l.crickets);
        if crickets > 0.0 && self.now >= self.crickets_next {
            self.cricket(crickets);
            let gap = 0.7 + self.random() * 0.6;
            self.crickets_next = self.at(gap);
        }
        let fire = self.level(l.fire);
        if fire > 0.0 && self.now >= self.fire_next {
            let f = 1000.0 + self.random() * 3000.0;
            let peak = (0.3 + self.random() * 0.7) * FIRE * fire;
            let decay = 0.02 + self.random() * 0.05;
            self.voices.push(Voice::Burst {
                start: self.at(0.05),
                filter: Biquad::bandpass(f, 1.5),
                peak,
                attack: 0.002,
                decay,
            });
            let gap = 0.04 + self.random() * 0.3;
            self.fire_next = self.at(gap);
        }
    }

    fn rumble(&mut self, level: f32) {
        let f1 = 110.0 + self.random() * 90.0;
        let d1 = 3.5 + self.random() * 2.0;
        self.voices.push(Voice::Burst {
            start: self.at(0.05),
            filter: Biquad::lowpass(f1, 0.7),
            peak: THUNDER * level,
            attack: 0.4,
            decay: d1,
        });
        let f2 = 80.0 + self.random() * 40.0;
        let later = 0.65 + self.random();
        self.voices.push(Voice::Burst {
            start: self.at(later),
            filter: Biquad::lowpass(f2, 0.7),
            peak: THUNDER * level * 0.6,
            attack: 0.8,
            decay: 2.5,
        });
    }

    fn chirps(&mut self, level: f32) {
        let base = 2200.0 + self.random() * 1800.0;
        let n = 2 + (self.random() * 4.0) as usize;
        let mut t = 0.05;
        for _ in 0..n {
            let len = 0.05 + self.random() * 0.07;
            let from = base * (0.9 + self.random() * 0.3);
            let to = base * (1.2 + self.random() * 0.4);
            self.voices.push(Voice::Chirp {
                start: self.at(t),
                from,
                to,
                len,
                peak: BIRDS * level,
            });
            t += len + 0.03 + self.random() * 0.08;
        }
    }

    fn cricket(&mut self, level: f32) {
        let freq = 4200.0 + self.random() * 600.0;
        for i in 0..3 {
            self.voices.push(Voice::Pulse {
                start: self.at(0.05 + 0.045 * i as f32),
                freq,
                peak: CRICKETS * level,
            });
        }
    }

    fn render(&mut self, out: &mut [f32]) {
        self.schedule();
        let dt = out.len() as f32 / RATE;
        // The beds' volumes drift to their targets, a block at a time.
        for bed in [&mut self.rain, &mut self.hearth, &mut self.wind] {
            bed.gain += (bed.target - bed.gain) * (1.0 - (-dt / bed.tau).exp());
        }
        // A wave: ramps up and back, with its filter opening and closing.
        let mut wave_freq = None;
        match &self.wave {
            Some(w) => {
                let t = (self.now - w.start) as f32 / RATE;
                let (g, f) = if t < w.rise {
                    let k = t / w.rise;
                    (
                        w.from.0 + (w.peak - w.from.0) * k,
                        w.from.1 + (1000.0 - w.from.1) * k,
                    )
                } else {
                    let k = ((t - w.rise) / w.fall).min(1.0);
                    (
                        w.peak + (w.peak * 0.15 - w.peak) * k,
                        1000.0 + (350.0 - 1000.0) * k,
                    )
                };
                self.waves.gain = g;
                wave_freq = Some(f);
            }
            None => self.waves.gain += (0.0 - self.waves.gain) * (1.0 - (-dt / 1.5f32).exp()),
        }
        if let Some(f) = wave_freq {
            self.waves.filter = Biquad::lowpass(f, 0.707).keeping(&self.waves.filter);
        }
        for s in out.iter_mut() {
            let mut v = 0.0;
            for i in 0..4 {
                let n = self.random() * 2.0 - 1.0;
                let bed = match i {
                    0 => &mut self.rain,
                    1 => &mut self.hearth,
                    2 => &mut self.wind,
                    _ => &mut self.waves,
                };
                if bed.gain > 0.0001 {
                    v += bed.filter.run(n) * bed.gain;
                }
            }
            *s = v;
        }
        let now = self.now;
        let mut voices = std::mem::take(&mut self.voices);
        voices.retain_mut(|voice| {
            let mut alive = false;
            for (i, s) in out.iter_mut().enumerate() {
                let at = now + i as u64;
                match voice.sample(at, self) {
                    Some(x) => {
                        *s += x;
                        alive = true;
                    }
                    None if voice.start() > at => alive = true,
                    None => {}
                }
            }
            alive
        });
        voices.append(&mut self.voices);
        self.voices = voices;
        self.now += out.len() as u64;
    }
}

impl Voice {
    fn start(&self) -> u64 {
        match self {
            Voice::Note { start, .. }
            | Voice::Burst { start, .. }
            | Voice::Chirp { start, .. }
            | Voice::Pulse { start, .. } => *start,
        }
    }

    /// This voice's sample at `at`, or None before it starts and after it ends.
    fn sample(&mut self, at: u64, mix: &mut Mix) -> Option<f32> {
        let start = self.start();
        if at < start {
            return None;
        }
        let t = (at - start) as f32 / RATE;
        match self {
            Voice::Note { freq, ring, .. } => {
                let ring = *ring;
                let env = if t < 0.02 {
                    0.18 * t / 0.02
                } else if t < ring.max(0.03) {
                    0.18 + (0.12 - 0.18) * (t - 0.02) / (ring.max(0.03) - 0.02)
                } else if t < ring + 0.12 {
                    0.12 * (0.001f32 / 0.12).powf((t - ring) / 0.12)
                } else {
                    return None;
                };
                // A triangle wave.
                let p = (t * *freq).fract();
                Some(env * (4.0 * (p - 0.5).abs() - 1.0))
            }
            Voice::Burst {
                filter,
                peak,
                attack,
                decay,
                ..
            } => {
                if t > *attack + *decay {
                    return None;
                }
                let env = if t < *attack {
                    *peak * t / *attack
                } else {
                    *peak * (0.0001 / *peak).powf((t - *attack) / *decay)
                };
                let n = mix.random() * 2.0 - 1.0;
                Some(filter.run(n) * env)
            }
            Voice::Chirp {
                from,
                to,
                len,
                peak,
                ..
            } => {
                if t > *len {
                    return None;
                }
                // Phase of an exponential sweep from `from` to `to`.
                let k = (*to / *from).ln() / *len;
                let phase = *from * ((k * t).exp() - 1.0) / k;
                let env = if t < 0.01 {
                    *peak * t / 0.01
                } else {
                    *peak * (0.0001 / *peak).powf((t - 0.01) / (*len - 0.01))
                };
                Some(env * (std::f32::consts::TAU * phase).sin())
            }
            Voice::Pulse { freq, peak, .. } => {
                if t > 0.02 {
                    return None;
                }
                let env = if t < 0.004 {
                    *peak * t / 0.004
                } else {
                    *peak * (1.0 - (t - 0.004) / 0.016)
                };
                Some(env * (std::f32::consts::TAU * *freq * t).sin())
            }
        }
    }
}

/// A two-pole filter (the Audio EQ Cookbook's, as a browser's BiquadFilter).
#[derive(Clone)]
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    x: [f32; 2],
    y: [f32; 2],
}

impl Biquad {
    fn new(freq: f32, q: f32, band: bool) -> Self {
        let w = std::f32::consts::TAU * freq.clamp(10.0, RATE * 0.45) / RATE;
        let alpha = w.sin() / (2.0 * q.max(0.1));
        let cos = w.cos();
        let a0 = 1.0 + alpha;
        let b = if band {
            [alpha, 0.0, -alpha]
        } else {
            [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0]
        };
        Biquad {
            b: b.map(|v| v / a0),
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn lowpass(freq: f32, q: f32) -> Self {
        Self::new(freq, q, false)
    }

    fn bandpass(freq: f32, q: f32) -> Self {
        Self::new(freq, q, true)
    }

    /// New settings, carrying on from where the old filter was (no click).
    fn keeping(mut self, old: &Biquad) -> Self {
        self.x = old.x;
        self.y = old.y;
        self
    }

    fn run(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tunes_like_the_browser() {
        let (bpm, notes) = parse_tune("@90 C E/ G- _ F#5 Bb3// A. hello");
        assert_eq!(bpm, 90.0);
        assert_eq!(
            notes,
            vec![
                (Some(60), 1.0),
                (Some(64), 0.5),
                (Some(67), 2.0),
                (None, 1.0),
                (Some(78), 1.0),
                (Some(58), 0.25),
                (Some(69), 1.5)
            ]
        );
        assert_eq!(parse_tune("@999 C").0, 240.0);
        assert!(
            parse_tune("Cat hello C7").1.is_empty(),
            "words aren't notes"
        );
    }

    fn loudness(mix: &mut Mix, secs: f32) -> f32 {
        let mut block = [0f32; BLOCK];
        let mut peak = 0f32;
        for _ in 0..(secs * RATE / BLOCK as f32) as usize {
            mix.render(&mut block);
            peak = block.iter().fold(peak, |p, s| p.max(s.abs()));
        }
        peak
    }

    #[test]
    fn a_hum_sounds_then_stops() {
        let mut m = Mix::new(7);
        assert_eq!(loudness(&mut m, 0.2), 0.0, "quiet to begin with");
        m.tune("@120 C E G");
        // Three half-second notes, the last one let go by about 2.1s.
        let p = loudness(&mut m, 2.2);
        assert!(p > 0.1 && p <= 0.19, "{p}");
        assert_eq!(loudness(&mut m, 0.5), 0.0, "and done");
        assert!(m.voices.is_empty());
    }

    #[test]
    fn rain_comes_and_goes() {
        let mut m = Mix::new(7);
        m.levels = Sounds {
            rain: 1.0,
            ..Default::default()
        };
        let p = loudness(&mut m, 6.0);
        assert!(p > 0.02 && p < 0.5, "{p}");
        m.levels = Sounds::default();
        loudness(&mut m, 10.0);
        assert!(loudness(&mut m, 1.0) < 0.002);
    }

    #[test]
    fn everything_at_once_stays_in_bounds() {
        let mut m = Mix::new(11);
        m.levels = Sounds {
            rain: 1.0,
            thunder: 1.0,
            wind: 1.0,
            waves: 1.0,
            birds: 1.0,
            crickets: 1.0,
            fire: 1.0,
        };
        m.tune("C E G C5");
        let p = loudness(&mut m, 30.0);
        assert!(p.is_finite() && p > 0.05 && p < 1.0, "{p}");
    }

    #[test]
    fn sound_off_is_silent() {
        let mut m = Mix::new(3);
        m.on = false;
        m.levels = Sounds {
            rain: 1.0,
            birds: 1.0,
            ..Default::default()
        };
        m.tune("C E G");
        assert!(loudness(&mut m, 3.0) < 1e-6);
    }
}
