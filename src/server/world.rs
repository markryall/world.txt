//! The island: a grid of tiles plus the things people have made on it.

use super::db::{Db, ThingRow, TileRow};
use super::rng::{self, fbm};
use super::terrain::Terrain;
use super::weather::{Clock, Phase, Season, Sky, Weather};
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

pub struct Home {
    pub owner: String,
    pub x: i64,
    pub y: i64,
    pub guests: Vec<String>,
}

/// A home is the 3x3 patch around where it was claimed.
pub const HOME_RADIUS: i64 = 1;

impl Home {
    pub fn covers(&self, x: i64, y: i64) -> bool {
        (x - self.x).abs() <= HOME_RADIUS && (y - self.y).abs() <= HOME_RADIUS
    }
    pub fn welcomes(&self, name: &str) -> bool {
        self.owner.eq_ignore_ascii_case(name) || self.guests.iter().any(|g| g.eq_ignore_ascii_case(name))
    }
}

pub struct World {
    pub seed: u64,
    tiles: Vec<Tile>,
    pub things: Vec<Thing>,
    pub homes: Vec<Home>,
    /// username → nickname, for everyone who's ever visited.
    pub nicks: std::collections::HashMap<String, String>,
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
        let homes = db
            .homes()?
            .into_iter()
            .map(|(owner, x, y, guests)| Home { owner, x, y, guests })
            .collect();
        let nicks = db.users()?.into_iter().collect();
        Ok(World { seed, tiles, things, homes, nicks, weather: Weather::new(SIZE, seed), db })
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

    /// Someone's nickname, from their username (or the text itself, for
    /// things like "the island").
    pub fn nick(&self, user: &str) -> String {
        self.nicks.get(user).cloned().unwrap_or_else(|| user.to_string())
    }

    pub fn home_at(&self, x: i64, y: i64) -> Option<&Home> {
        self.homes.iter().find(|h| h.covers(x, y))
    }

    pub fn place_title(&self, x: i64, y: i64) -> String {
        match self.tile(x, y) {
            Some(Tile { name: Some(n), .. }) => n.clone(),
            Some(_) if self.home_at(x, y).is_some() => {
                let h = self.home_at(x, y).unwrap();
                let what = if (h.x, h.y) == (x, y) { "Home" } else { "Garden" };
                format!("{}'s {what}", self.nick(&h.owner))
            }
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

    pub fn remove_thing(&mut self, i: usize, by: &str) -> anyhow::Result<Thing> {
        let t = self.things.remove(i);
        self.db.remove_thing(t.id, by)?;
        Ok(t)
    }

    /// Bring back the last thing `by` unmade in the past day.
    pub fn undo_remove(&mut self, by: &str) -> anyhow::Result<Option<&Thing>> {
        let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64 - 86_400;
        let Some(r) = self.db.last_removed(by, since) else { return Ok(None) };
        self.db.restore_thing(r.id)?;
        self.things.push(Thing {
            id: r.id,
            x: r.x,
            y: r.y,
            name: r.name,
            action: r.action,
            trigger: Trigger::from_code(r.trigger.as_deref().unwrap_or("always")),
            maker: r.maker,
            kind: r.kind,
        });
        Ok(self.things.last())
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

        let mut text = rng::pick(here.descriptions_in(Clock::season()), self.seed, x, y).to_string();
        text.push(' ');
        text.push_str(&atmosphere(here, sky, phase, Clock::season(), self.weather.breezy(), self.seed, x, y));
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

    /// The whole island at half scale (each character is a 2x2 patch), with
    /// you, friends, homes, boards and numbered named places.
    pub fn island_map(&self, me: (i64, i64), people: &[(i64, i64)]) -> Vec<Line> {
        const KEYS: &str = "123456789abcdefghijklmnopqrstuvwxyz";
        let mut named: Vec<(i64, i64, String)> = Vec::new();
        for y in 0..SIZE {
            for x in 0..SIZE {
                if let Some(n) = self.tile(x, y).and_then(|t| t.name.clone()) {
                    named.push((x, y, n));
                }
            }
        }
        for h in &self.homes {
            if !named.iter().any(|(x, y, _)| (*x, *y) == (h.x, h.y)) {
                named.push((h.x, h.y, format!("{}'s Home", self.nick(&h.owner))));
            }
        }
        named.truncate(KEYS.len());
        let cell = |x: i64, y: i64| (x / 2, y / 2);
        // Crop to the island, with a little sea around it.
        let land: Vec<(i64, i64)> = (0..SIZE)
            .flat_map(|y| (0..SIZE).map(move |x| (x, y)))
            .filter(|&(x, y)| self.terrain(x, y) != Terrain::DeepWater)
            .map(|(x, y)| cell(x, y))
            .collect();
        let (x0, x1) = (land.iter().map(|c| c.0).min().unwrap_or(0) - 1, land.iter().map(|c| c.0).max().unwrap_or(SIZE / 2) + 1);
        let (y0, y1) = (land.iter().map(|c| c.1).min().unwrap_or(0) - 1, land.iter().map(|c| c.1).max().unwrap_or(SIZE / 2) + 1);
        let mut out = Vec::new();
        for cy in y0.max(0)..=y1.min(SIZE / 2 - 1) {
            let mut line = Line::new();
            for cx in x0.max(0)..=x1.min(SIZE / 2 - 1) {
                let covers = |p: (i64, i64)| cell(p.0, p.1) == (cx, cy);
                if covers(me) {
                    line = line.b(Col::Magenta, "@");
                } else if people.iter().any(|p| covers(*p)) {
                    line = line.b(Col::White, "☺");
                } else if let Some(i) = named.iter().position(|(x, y, _)| covers((*x, *y))) {
                    line = line.b(Col::Yellow, KEYS.chars().nth(i).unwrap().to_string());
                } else if self.homes.iter().any(|h| covers((h.x, h.y))) {
                    line = line.b(Col::Magenta, "⌂");
                } else if self.things.iter().any(|t| t.is_board() && covers((t.x, t.y))) {
                    line = line.b(Col::Yellow, "▣");
                } else {
                    // The most common ground in the patch.
                    let mut counts: Vec<(Terrain, usize)> = Vec::new();
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let t = self.terrain(cx * 2 + dx, cy * 2 + dy);
                        match counts.iter_mut().find(|(c, _)| *c == t) {
                            Some((_, n)) => *n += 1,
                            None => counts.push((t, 1)),
                        }
                    }
                    let t = counts.iter().max_by_key(|(_, n)| *n).map_or(Terrain::DeepWater, |c| c.0);
                    let (g, c) = t.glyph();
                    line = line.c(c, g);
                }
            }
            out.push(line);
        }
        out.push(Line::new().b(Col::Magenta, "@").t(" you  ").b(Col::White, "☺").t(" friends  ").b(Col::Magenta, "⌂").t(" homes  ").b(Col::Yellow, "▣").t(" boards"));
        for (i, (_, _, n)) in named.iter().enumerate() {
            out.push(Line::new().b(Col::Yellow, format!("{} ", KEYS.chars().nth(i).unwrap())).t(n.clone()));
        }
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
                } else if self.homes.iter().any(|h| (h.x, h.y) == (x, y)) {
                    line = line.b(Col::Magenta, "⌂");
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

/// A touch of the season, for descriptions.
fn season_line(here: Terrain, season: Season) -> &'static str {
    use Terrain::*;
    match (season, here) {
        (Season::Spring, Forest) => "Fresh green leaves are unfurling everywhere.",
        (Season::Spring, Meadow | Grass) => "Blossom petals drift on the breeze.",
        (Season::Spring, Sand | Water | DeepWater) => "The sea sparkles in the spring light.",
        (Season::Spring, _) => "Everything feels new and green.",
        (Season::Summer, Forest) => "The woods are deep, green and humming.",
        (Season::Summer, Meadow | Grass) => "The air is thick with the hum of summer.",
        (Season::Summer, Sand | Water | DeepWater) => "The sand is warm and the water inviting.",
        (Season::Summer, _) => "It's a long, golden summer.",
        (Season::Autumn, Forest) => "The trees blaze red and gold, and leaves drift down.",
        (Season::Autumn, Meadow | Grass) => "The grass has turned gold, and there's a crispness in the air.",
        (Season::Autumn, Sand | Water | DeepWater) => "A cool autumn wind comes in off the sea.",
        (Season::Autumn, _) => "Autumn leaves skitter across the ground.",
        (Season::Winter, Forest) => "The bare branches are traced with frost.",
        (Season::Winter, Meadow | Grass) => "Frost silvers every blade of grass.",
        (Season::Winter, Pond | Water) => "A thin skin of ice edges the water.",
        (Season::Winter, Sand | DeepWater) => "The sea is grey and the wind is cold.",
        (Season::Winter, _) => "The air is cold and clear.",
    }
}

fn atmosphere(
    here: Terrain,
    sky: Sky,
    phase: Phase,
    season: Season,
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
    let snow = season == Season::Winter;
    let mut s = if snow { light.replace("drizzle", "flurry of snow").replace("Rain falls", "Snow falls") } else { light.to_string() };
    if sky == Sky::Rain && phase != Phase::Day {
        s.push_str(if snow { " Snow falls steadily." } else { " Rain falls steadily." });
    }
    s.push(' ');
    s.push_str(season_line(here, season));
    if sky.wet() && snow {
        s.push_str(" Snow settles softly on everything.");
    } else if sky.wet() {
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
