//! Clouds drift across the island on the wind, so it can be raining by
//! the river and sunny in the meadow. Time runs at one game hour per
//! real minute, so a whole day passes in 24 minutes.

use super::rng::Rng;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sky {
    Clear,
    Cloudy,
    Drizzle,
    Rain,
}

impl Sky {
    /// What it's called this season: winter rain falls as snow.
    pub fn label_in(self, season: Season) -> &'static str {
        match (season, self) {
            (Season::Winter, Sky::Drizzle) => "snow flurries",
            (Season::Winter, Sky::Rain) => "snow",
            _ => self.label(),
        }
    }
    pub fn icon_in(self, phase: Phase, season: Season) -> &'static str {
        if season == Season::Winter && self.wet() { "🌨" } else { self.icon(phase) }
    }
    pub fn label(self) -> &'static str {
        match self {
            Sky::Clear => "clear",
            Sky::Cloudy => "cloudy",
            Sky::Drizzle => "drizzle",
            Sky::Rain => "rain",
        }
    }
    pub fn icon(self, phase: Phase) -> &'static str {
        match (self, phase) {
            (Sky::Clear, Phase::Night) => "☾",
            (Sky::Clear, _) => "☀",
            (Sky::Cloudy, _) => "☁",
            (Sky::Drizzle, _) => "☂",
            (Sky::Rain, _) => "☔",
        }
    }
    pub fn wet(self) -> bool {
        matches!(self, Sky::Drizzle | Sky::Rain)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Dawn,
    Day,
    Dusk,
    Night,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Dawn => "dawn",
            Phase::Day => "day",
            Phase::Dusk => "dusk",
            Phase::Night => "night",
        }
    }
}

/// The island's seasons turn every week (Monday to Monday), so a whole year
/// passes in a month.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

impl Season {
    pub fn label(self) -> &'static str {
        match self {
            Season::Spring => "spring",
            Season::Summer => "summer",
            Season::Autumn => "autumn",
            Season::Winter => "winter",
        }
    }
    pub fn icon(self) -> &'static str {
        match self {
            Season::Spring => "🌸",
            Season::Summer => "☀️",
            Season::Autumn => "🍂",
            Season::Winter => "❄️",
        }
    }
    pub fn arrival(self) -> &'static str {
        match self {
            Season::Spring => "🌸 Spring has come to the island! Blossom drifts on the breeze and everything is waking up.",
            Season::Summer => "☀️ Summer is here! Long golden days, warm sand, and dragonflies over the meadows.",
            Season::Autumn => "🍂 Autumn has arrived. The woods turn red and gold, and leaves come spinning down.",
            Season::Winter => "❄️ Winter has come. Frost on the grass, snow in the clouds, and cosy evenings ahead.",
        }
    }
    pub fn festival(self) -> Festival {
        match self {
            Season::Spring => Festival { name: "the Blossom Picnic", icon: "🧺", greeting: "It's the Blossom Picnic! Golden blossoms are hiding all over the island. 🌸" },
            Season::Summer => Festival { name: "Midsummer Beach Night", icon: "🏖️", greeting: "It's Midsummer Beach Night! Glowing shells are washing up on the shore. 🐚" },
            Season::Autumn => Festival { name: "Lantern Night", icon: "🏮", greeting: "It's Lantern Night! Paper lanterns are hiding all over the island. 🏮" },
            Season::Winter => Festival { name: "Snow Lights", icon: "✨", greeting: "It's Snow Lights! Snow globes are tucked away all over the island. ❄️" },
        }
    }
    fn from_label(s: &str) -> Option<Season> {
        [Season::Spring, Season::Summer, Season::Autumn, Season::Winter].into_iter().find(|x| x.label() == s.trim().to_lowercase())
    }
}

/// Each season's Saturday is a festival.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Festival {
    pub name: &'static str,
    pub icon: &'static str,
    pub greeting: &'static str,
}

pub struct Clock;

impl Clock {
    /// Game time in minutes past midnight (0..1440).
    pub fn minutes() -> u64 {
        let secs = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        secs % 1440
    }
    pub fn phase() -> Phase {
        match Self::minutes() / 60 {
            5..=6 => Phase::Dawn,
            7..=17 => Phase::Day,
            18..=20 => Phase::Dusk,
            _ => Phase::Night,
        }
    }
    /// This week's season. WORLD_SEASON=autumn forces one (for testing, or a treat).
    pub fn season() -> Season {
        if let Some(s) = std::env::var("WORLD_SEASON").ok().and_then(|s| Season::from_label(&s)) {
            return s;
        }
        use chrono::Datelike;
        let monday_weeks = |d: chrono::NaiveDate| (d.num_days_from_ce() as i64 - 1).div_euclid(7);
        // The island's first spring was the week of 2 November 2026.
        let first = chrono::NaiveDate::from_ymd_opt(2026, 11, 2).map_or(0, monday_weeks);
        let now = monday_weeks(chrono::Local::now().date_naive());
        [Season::Spring, Season::Summer, Season::Autumn, Season::Winter][(now - first).rem_euclid(4) as usize]
    }

    /// Saturdays are festivals. WORLD_FESTIVAL=1 starts one any day.
    pub fn festival() -> Option<Festival> {
        use chrono::Datelike;
        let forced = std::env::var("WORLD_FESTIVAL").is_ok_and(|v| v == "1");
        (forced || chrono::Local::now().weekday() == chrono::Weekday::Sat).then(|| Self::season().festival())
    }

    pub fn label() -> String {
        let m = Self::minutes();
        let (h, mm) = (m / 60, m % 60);
        let (h12, ampm) = match h {
            0 => (12, "am"),
            1..=11 => (h, "am"),
            12 => (12, "pm"),
            _ => (h - 12, "pm"),
        };
        let season = Self::season();
        let mut s = format!("{h12}:{mm:02}{ampm} · {} · {} {}", Self::phase().label(), season.icon(), season.label());
        if let Some(f) = Self::festival() {
            s.push_str(&format!(" · {} {}!", f.icon, f.name));
        }
        s
    }
}

struct Cloud {
    x: f64,
    y: f64,
    r: f64,
    strength: f64,
}

pub struct Weather {
    clouds: Vec<Cloud>,
    wind: f64, // direction in radians
    speed: f64,
    size: f64,
    rng: Rng,
}

impl Weather {
    pub fn new(size: i64, seed: u64) -> Weather {
        let mut w = Weather {
            clouds: Vec::new(),
            wind: 0.3,
            speed: 0.25,
            size: size as f64,
            rng: Rng::new(seed ^ 0xc10d),
        };
        let area = (size as f64 / 64.0).powi(2);
        for _ in 0..(4.0 * area) as usize {
            let c = Cloud {
                x: w.rng.range(0.0, w.size),
                y: w.rng.range(0.0, w.size),
                r: w.rng.range(5.0, 12.0),
                strength: w.rng.range(0.4, 1.1),
            };
            w.clouds.push(c);
        }
        w
    }

    /// Called every tick (a few seconds).
    pub fn step(&mut self) {
        self.wind += self.rng.range(-0.05, 0.05);
        self.speed = (self.speed + self.rng.range(-0.02, 0.02)).clamp(0.08, 0.5);
        let (dx, dy) = (self.wind.cos() * self.speed, self.wind.sin() * self.speed);
        for c in &mut self.clouds {
            c.x += dx;
            c.y += dy;
            c.strength = (c.strength + self.rng.range(-0.03, 0.03)).clamp(0.1, 1.2);
        }
        let s = self.size;
        self.clouds
            .retain(|c| c.x > -c.r && c.y > -c.r && c.x < s + c.r && c.y < s + c.r);
        let area = (self.size / 64.0).powi(2);
        if (self.clouds.len() as f64) < 5.0 * area && self.rng.chance(0.08 * area) {
            // Arrive from upwind.
            let along = self.rng.range(0.0, s);
            let r = self.rng.range(5.0, 12.0);
            let (x, y) = if dx.abs() > dy.abs() {
                (if dx > 0.0 { -r + 1.0 } else { s + r - 1.0 }, along)
            } else {
                (along, if dy > 0.0 { -r + 1.0 } else { s + r - 1.0 })
            };
            let strength = self.rng.range(0.3, 1.1);
            self.clouds.push(Cloud { x, y, r, strength });
        }
    }

    pub fn sky_at(&self, x: i64, y: i64) -> Sky {
        let (px, py) = (x as f64, y as f64);
        let k = self
            .clouds
            .iter()
            .map(|c| {
                let d = ((c.x - px).powi(2) + (c.y - py).powi(2)).sqrt();
                c.strength * (1.0 - d / c.r).max(0.0)
            })
            .fold(0.0, f64::max);
        if k > 0.55 {
            Sky::Rain
        } else if k > 0.3 {
            Sky::Drizzle
        } else if k > 0.08 {
            Sky::Cloudy
        } else {
            Sky::Clear
        }
    }

    pub fn wind_label(&self) -> &'static str {
        // Wind is named for where it comes from.
        let from = (self.wind + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU);
        let names = ["east", "south-east", "south", "south-west", "west", "north-west", "north", "north-east"];
        let i = ((from / std::f64::consts::TAU) * 8.0).round() as usize % 8;
        names[i]
    }

    /// How hard the wind's blowing, from about 0.08 (still) to 0.5.
    pub fn speed(&self) -> f64 {
        self.speed
    }

    pub fn breezy(&self) -> bool {
        self.speed > 0.3
    }
}
