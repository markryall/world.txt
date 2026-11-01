//! The island: a grid of tiles plus the things people have made on it.

use super::db::{Db, ThingRow, TileRow};
use super::rng::{self, fbm};
use super::terrain::Terrain;
use super::weather::{Clock, Phase, Sky, Weather};
use crate::protocol::{Col, Line};

pub const SIZE: i64 = 64;

pub struct Tile {
    pub terrain: Terrain,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    Always,
    Rain,
    Sun,
    Night,
    Day,
    Arrive,
}

impl Trigger {
    pub fn code(self) -> &'static str {
        match self {
            Trigger::Always => "always",
            Trigger::Rain => "rain",
            Trigger::Sun => "sun",
            Trigger::Night => "night",
            Trigger::Day => "day",
            Trigger::Arrive => "arrive",
        }
    }
    pub fn from_code(s: &str) -> Trigger {
        match s {
            "rain" => Trigger::Rain,
            "sun" => Trigger::Sun,
            "night" => Trigger::Night,
            "day" => Trigger::Day,
            "arrive" => Trigger::Arrive,
            _ => Trigger::Always,
        }
    }
}

pub struct Thing {
    pub id: i64,
    pub x: i64,
    pub y: i64,
    pub name: String,
    pub action: Option<String>,
    pub trigger: Trigger,
    pub maker: String,
    /// "board" or "journal" for noticeboards.
    pub kind: Option<String>,
}

impl Thing {
    pub fn is_board(&self) -> bool {
        self.kind.is_some()
    }
}

pub struct World {
    pub seed: u64,
    tiles: Vec<Tile>,
    pub things: Vec<Thing>,
    pub weather: Weather,
    pub db: Db,
}

pub const DIRS: [(&str, &str, i64, i64); 8] = [
    ("n", "north", 0, -1),
    ("ne", "north-east", 1, -1),
    ("e", "east", 1, 0),
    ("se", "south-east", 1, 1),
    ("s", "south", 0, 1),
    ("sw", "south-west", -1, 1),
    ("w", "west", -1, 0),
    ("nw", "north-west", -1, -1),
];

impl World {
    pub fn load_or_create(mut db: Db, seed: u64) -> anyhow::Result<World> {
        let seed = match db.meta("seed") {
            Some(s) => s.parse()?,
            None => {
                let tiles = generate(seed);
                db.save_tiles(&tiles)?;
                db.set_meta("seed", &seed.to_string())?;
                println!("🌱 grew a brand-new island from seed {seed}");
                seed
            }
        };
        let mut tiles: Vec<Tile> = (0..SIZE * SIZE)
            .map(|_| Tile { terrain: Terrain::DeepWater, name: None })
            .collect();
        for row in db.tiles()? {
            if in_bounds(row.x, row.y) {
                tiles[idx(row.x, row.y)] =
                    Tile { terrain: Terrain::from_code(&row.terrain), name: row.name };
            }
        }
        let things = db
            .things()?
            .into_iter()
            .map(|r| Thing {
                id: r.id,
                x: r.x,
                y: r.y,
                name: r.name,
                action: r.action,
                trigger: Trigger::from_code(r.trigger.as_deref().unwrap_or("always")),
                maker: r.maker,
                kind: r.kind,
            })
            .collect();
        Ok(World { seed, tiles, things, weather: Weather::new(SIZE, seed), db })
    }

    pub fn tile(&self, x: i64, y: i64) -> Option<&Tile> {
        in_bounds(x, y).then(|| &self.tiles[idx(x, y)])
    }

    pub fn terrain(&self, x: i64, y: i64) -> Terrain {
        self.tile(x, y).map_or(Terrain::DeepWater, |t| t.terrain)
    }

    pub fn set_terrain(&mut self, x: i64, y: i64, terrain: Terrain) -> anyhow::Result<()> {
        if !in_bounds(x, y) {
            return Ok(());
        }
        self.tiles[idx(x, y)].terrain = terrain;
        self.persist_tile(x, y)
    }

    pub fn set_name(&mut self, x: i64, y: i64, name: Option<String>) -> anyhow::Result<()> {
        if !in_bounds(x, y) {
            return Ok(());
        }
        self.tiles[idx(x, y)].name = name;
        self.persist_tile(x, y)
    }

    fn persist_tile(&mut self, x: i64, y: i64) -> anyhow::Result<()> {
        let t = &self.tiles[idx(x, y)];
        let row = TileRow { x, y, terrain: t.terrain.code().into(), name: t.name.clone() };
        self.db.save_tiles(&[row])
    }

    pub fn place_title(&self, x: i64, y: i64) -> String {
        match self.tile(x, y) {
            Some(Tile { name: Some(n), .. }) => n.clone(),
            Some(t) => t.terrain.place_name().to_string(),
            None => "The Edge of the World".into(),
        }
    }

    pub fn things_at(&self, x: i64, y: i64) -> impl Iterator<Item = &Thing> {
        self.things.iter().filter(move |t| t.x == x && t.y == y)
    }

    pub fn add_thing(
        &mut self,
        x: i64,
        y: i64,
        name: String,
        action: Option<String>,
        trigger: Trigger,
        maker: &str,
        kind: Option<&str>,
    ) -> anyhow::Result<i64> {
        let id = self.db.add_thing(&ThingRow {
            id: 0,
            x,
            y,
            name: name.clone(),
            action: action.clone(),
            trigger: Some(trigger.code().into()),
            maker: maker.into(),
            kind: kind.map(Into::into),
        })?;
        self.things.push(Thing { id, x, y, name, action, trigger, maker: maker.into(), kind: kind.map(Into::into) });
        Ok(id)
    }

    /// Find a thing here whose name contains the words given.
    pub fn find_thing(&self, x: i64, y: i64, words: &str) -> Option<usize> {
        let w = words.trim().to_lowercase();
        let w = strip_article(&w);
        self.things
            .iter()
            .position(|t| t.x == x && t.y == y && t.name.to_lowercase().contains(w))
    }

    pub fn remove_thing(&mut self, i: usize) -> anyhow::Result<Thing> {
        let t = self.things.remove(i);
        self.db.remove_thing(t.id)?;
        Ok(t)
    }

    pub fn move_thing(&mut self, i: usize, x: i64, y: i64) -> anyhow::Result<()> {
        let t = &mut self.things[i];
        t.x = x;
        t.y = y;
        self.db.move_thing(t.id, x, y)
    }

    pub fn spawn_point(&self) -> (i64, i64) {
        let c = SIZE / 2;
        let mut best = (c, c);
        let mut best_d = i64::MAX;
        for y in 0..SIZE {
            for x in 0..SIZE {
                let t = self.terrain(x, y);
                if matches!(t, Terrain::Grass | Terrain::Meadow | Terrain::Path) {
                    let d = (x - c).pow(2) + (y - c).pow(2);
                    if d < best_d {
                        best_d = d;
                        best = (x, y);
                    }
                }
            }
        }
        best
    }

    /// The full look at a place. `others` are the names of other people here.
    pub fn describe(&self, x: i64, y: i64, others: &[String]) -> Vec<Line> {
        let here = self.terrain(x, y);
        let sky = self.weather.sky_at(x, y);
        let phase = Clock::phase();
        let mut out = Vec::new();

        out.push(Line::new().t("").b(Col::Magenta, format!("~ {} ~", self.place_title(x, y))));

        let mut text = rng::pick(here.descriptions(), self.seed, x, y).to_string();
        text.push(' ');
        text.push_str(&atmosphere(here, sky, phase, self.weather.breezy(), self.seed, x, y));
        out.push(Line::plain(text));

        // What lies around.
        let mut seen: Vec<(Terrain, Vec<&str>)> = Vec::new();
        for (_, long, dx, dy) in DIRS.iter().filter(|d| d.2 == 0 || d.3 == 0) {
            let t = self.terrain(x + dx, y + dy);
            if t == here {
                continue;
            }
            match seen.iter_mut().find(|(s, _)| *s == t) {
                Some((_, dirs)) => dirs.push(long),
                None => seen.push((t, vec![long])),
            }
        }
        if !seen.is_empty() {
            let parts: Vec<String> = seen
                .iter()
                .map(|(t, dirs)| format!("{} to the {}", t.nearby(), join_and(dirs)))
                .collect();
            let mut s = format!("You can see {}.", join_and(&parts));
            if let Some(c) = s.get_mut(0..1) {
                c.make_ascii_uppercase();
            }
            out.push(Line::col(Col::Dim, s));
        }

        let things: Vec<&Thing> = self.things_at(x, y).collect();
        if !things.is_empty() {
            let names: Vec<String> = things.iter().map(|t| t.name.clone()).collect();
            out.push(Line::new().c(Col::Yellow, "✦ ").c(Col::Yellow, capitalise(&join_and(&names))));
        }
        if !others.is_empty() {
            let verb = if others.len() == 1 { "is" } else { "are" };
            out.push(Line::new().c(Col::Cyan, "☺ ").b(Col::Cyan, join_and(others)).c(
                Col::Cyan,
                format!(" {verb} here."),
            ));
        }

        let exits: Vec<&str> = DIRS
            .iter()
            .filter(|(_, _, dx, dy)| self.terrain(x + dx, y + dy).walkable())
            .map(|d| d.0)
            .collect();
        out.push(Line::new().c(Col::Dim, "exits: ").c(Col::Green, exits.join(" ")));
        out
    }

    /// A small coloured map centred on (cx, cy).
    pub fn minimap(&self, cx: i64, cy: i64, people: &[(i64, i64)], finds: &[(i64, i64)]) -> Vec<Line> {
        let (rw, rh) = (7, 4);
        let mut out = Vec::new();
        for y in cy - rh..=cy + rh {
            let mut line = Line::new();
            for x in cx - rw..=cx + rw {
                if x == cx && y == cy {
                    line = line.b(Col::Magenta, "@");
                } else if people.contains(&(x, y)) {
                    line = line.b(Col::White, "☺");
                } else if self.things_at(x, y).any(|t| t.is_board()) {
                    line = line.b(Col::Yellow, "▣");
                } else if self.things_at(x, y).next().is_some() {
                    line = line.c(Col::Yellow, "✦");
                } else if finds.contains(&(x, y)) {
                    line = line.b(Col::Pink, "•");
                } else if !in_bounds(x, y) {
                    line = line.c(Col::DeepBlue, "≈");
                } else {
                    let (g, c) = self.terrain(x, y).glyph();
                    line = line.c(c, g);
                }
                if x != cx + rw {
                    line = line.t(" ");
                }
            }
            out.push(line);
        }
        out
    }
}

fn atmosphere(
    here: Terrain,
    sky: Sky,
    phase: Phase,
    breezy: bool,
    seed: u64,
    x: i64,
    y: i64,
) -> String {
    let light = match (phase, sky) {
        (Phase::Dawn, Sky::Clear) => "The sky is blushing pink with dawn.",
        (Phase::Dawn, _) => "A grey dawn light seeps through the clouds.",
        (Phase::Day, Sky::Clear) => "The sun is warm on your face.",
        (Phase::Day, Sky::Cloudy) => "Soft clouds drift overhead.",
        (Phase::Dusk, Sky::Clear) => "The sky glows gold and violet with dusk.",
        (Phase::Dusk, _) => "Dusk settles in, soft and blue.",
        (Phase::Night, Sky::Clear) => "Stars are scattered thick across the night sky.",
        (Phase::Night, _) => "The night is dark and close.",
        (_, Sky::Drizzle) => "A fine drizzle hangs in the air.",
        (_, Sky::Rain) => "Rain falls steadily.",
    };
    let mut s = light.to_string();
    if sky == Sky::Rain && phase != Phase::Day {
        s.push_str(" Rain falls steadily.");
    }
    if sky.wet() {
        let wet = match here {
            Terrain::Forest => "Drops patter on the leaves above.",
            Terrain::Water | Terrain::Pond | Terrain::DeepWater => "Raindrops dimple the water.",
            Terrain::Sand => "The sand darkens under the rain.",
            Terrain::Meadow => "The flowers bow under the wet.",
            _ => "Everything smells of rain.",
        };
        s.push(' ');
        s.push_str(wet);
    } else if breezy {
        s.push(' ');
        s.push_str(rng::pick(
            &["A breeze tugs at your clothes.", "The wind is up, and lively.", "Gusts of wind come and go."],
            seed ^ 0xb,
            x,
            y,
        ));
    }
    s
}

pub fn join_and<S: AsRef<str>>(items: &[S]) -> String {
    match items {
        [] => String::new(),
        [a] => a.as_ref().into(),
        [a, b] => format!("{} and {}", a.as_ref(), b.as_ref()),
        [rest @ .., last] => {
            let head: Vec<&str> = rest.iter().map(|s| s.as_ref()).collect();
            format!("{}, and {}", head.join(", "), last.as_ref())
        }
    }
}

pub fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

pub fn strip_article(s: &str) -> &str {
    for a in ["a ", "an ", "the ", "some "] {
        if let Some(rest) = s.strip_prefix(a) {
            return rest;
        }
    }
    s
}

pub fn in_bounds(x: i64, y: i64) -> bool {
    (0..SIZE).contains(&x) && (0..SIZE).contains(&y)
}

fn idx(x: i64, y: i64) -> usize {
    (y * SIZE + x) as usize
}

/// Grow an island.
fn generate(seed: u64) -> Vec<TileRow> {
    let mut rows = Vec::new();
    let c = (SIZE - 1) as f64 / 2.0;
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (fx, fy) = (x as f64, y as f64);
            let d = (((fx - c) / c).powi(2) + ((fy - c) / c).powi(2)).sqrt();
            let e = fbm(seed, fx / 11.0, fy / 11.0) + 0.42 - 1.15 * d * d;
            let m = fbm(seed ^ 0x5eed, fx / 8.0, fy / 8.0);
            let terrain = if e < 0.22 {
                Terrain::DeepWater
            } else if e < 0.32 {
                Terrain::Water
            } else if e < 0.37 {
                Terrain::Sand
            } else if e < 0.74 {
                if m > 0.58 {
                    Terrain::Forest
                } else if m < 0.36 {
                    Terrain::Meadow
                } else {
                    Terrain::Grass
                }
            } else if e < 0.86 {
                Terrain::Hill
            } else {
                Terrain::Stone
            };
            rows.push(TileRow { x, y, terrain: terrain.code().into(), name: None });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cargo test -- --nocapture print_island` to eyeball the generator.
    #[test]
    fn print_island() {
        let rows = generate(7);
        let mut land = 0;
        for y in 0..SIZE {
            let mut s = String::new();
            for x in 0..SIZE {
                let t = Terrain::from_code(&rows[idx(x, y)].terrain);
                if t.walkable() && !t.is_wet() {
                    land += 1;
                }
                s.push_str(t.glyph().0);
            }
            println!("{s}");
        }
        assert!(land > 600, "island too small: {land}");
    }
}
