//! The island: a grid of tiles plus the things people have made on it.

use super::db::{Db, PlantingRow, ThingRow, TileRow};
use super::paint;
use super::rng;
use super::terrain::Terrain;
use super::weather::{Clock, Phase, Season, Sky, Weather};
use crate::protocol::{Col, Line, Span};
use std::collections::HashSet;

/// The whole archipelago: four islands, each in a 64x64 patch.
pub const SIZE: i64 = 2 * super::islands::REGION;

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
    /// The colour it's painted, in the painter's own word.
    pub paint: Option<String>,
}

impl Thing {
    /// Noticeboards and the Island Journal (not music boxes or docks).
    pub fn is_board(&self) -> bool {
        matches!(self.kind.as_deref(), Some("board" | "journal"))
    }
    pub fn is_dock(&self) -> bool {
        self.kind.as_deref() == Some("dock")
    }
    /// Things that belong to the island itself, which nobody unmakes or pushes.
    pub fn is_fixed(&self) -> bool {
        matches!(
            self.kind.as_deref(),
            Some("journal" | "dock" | "lighthouse")
        )
    }
    pub fn is_lighthouse(&self) -> bool {
        self.kind.as_deref() == Some("lighthouse")
    }
}

pub struct Home {
    pub owner: String,
    pub x: i64,
    pub y: i64,
    pub guests: Vec<String>,
    /// The colour it's painted, in the owner's own word.
    pub paint: Option<String>,
}

/// Rooms inside buildings live off the map, one per building, on the same
/// side of it as the building's island, so that which island you're on (and
/// everything that goes by where you're standing) still works inside.
pub const ROOMS: i64 = 1_000_000;
/// How far apart rooms are, so that nothing that goes by distance (a shout,
/// a gramophone, a smell from the oven) carries from one room to the next.
pub const ROOM_STRIDE: i64 = 1_000;

/// Where the room inside building `id` (standing at bx, by) is.
pub fn room_for(id: i64, bx: i64, by: i64) -> (i64, i64) {
    let side = |v: i64| {
        if v.div_euclid(super::islands::REGION) >= 1 {
            1
        } else {
            -1
        }
    };
    (side(bx) * (ROOMS + id * ROOM_STRIDE), side(by) * ROOMS)
}

/// Which building's room (x, y) is, if it's a room.
pub fn room_of(x: i64, y: i64) -> Option<i64> {
    let off = x.abs() - ROOMS;
    (y.abs() == ROOMS && off > 0 && off % ROOM_STRIDE == 0).then_some(off / ROOM_STRIDE)
}

/// Off the map, where the rooms are.
pub fn in_room_space(x: i64, y: i64) -> bool {
    x.abs() >= ROOMS || y.abs() >= ROOMS
}

/// A home is the 3x3 patch around where it was claimed.
pub const HOME_RADIUS: i64 = 1;

impl Home {
    pub fn covers(&self, x: i64, y: i64) -> bool {
        (x - self.x).abs() <= HOME_RADIUS && (y - self.y).abs() <= HOME_RADIUS
    }
    pub fn welcomes(&self, name: &str) -> bool {
        self.owner.eq_ignore_ascii_case(name)
            || self.guests.iter().any(|g| g.eq_ignore_ascii_case(name))
    }
}

pub struct World {
    pub seed: u64,
    tiles: Vec<Tile>,
    pub things: Vec<Thing>,
    pub homes: Vec<Home>,
    /// username → nickname, for everyone who's ever visited.
    pub nicks: std::collections::HashMap<String, String>,
    /// What people have written about places (`describe`), by position.
    pub descriptions: std::collections::HashMap<(i64, i64), String>,
    /// Trees and flowers people have planted, growing, by position.
    pub plantings: std::collections::HashMap<(i64, i64), PlantingRow>,
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
    /// Open a world, or grow a brand-new one from `seed` (random if none:
    /// every new world is its own shape). The seed is kept, so a world keeps
    /// its shape whatever it's started with later.
    pub fn load_or_create(mut db: Db, seed: Option<u64>) -> anyhow::Result<World> {
        let seed = match db.meta("seed") {
            Some(s) => s.parse()?,
            None => {
                // Short enough to read out, should anyone want a twin.
                let seed = match seed {
                    Some(s) => s,
                    None => super::accounts::random_u64()? % 1_000_000,
                };
                db.save_tiles(&super::islands::ISLANDS[0].generate(seed))?;
                db.set_meta("seed", &seed.to_string())?;
                println!("🌱 grew a brand-new island from seed {seed}");
                seed
            }
        };
        // Worlds from before the archipelago grow the other islands, once.
        if db.meta("archipelago").is_none() {
            for island in &super::islands::ISLANDS[1..] {
                db.save_tiles(&island.generate(seed))?;
            }
            db.set_meta("archipelago", "1")?;
            println!(
                "⛵ three new islands rose out of the sea: Driftwood Isle, Mossy Holm and Ember Rock"
            );
        }
        let mut tiles: Vec<Tile> = (0..SIZE * SIZE)
            .map(|_| Tile {
                terrain: Terrain::DeepWater,
                name: None,
            })
            .collect();
        for row in db.tiles()? {
            if in_bounds(row.x, row.y) {
                tiles[idx(row.x, row.y)] = Tile {
                    terrain: Terrain::from_code(&row.terrain),
                    name: row.name,
                };
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
                paint: r.paint,
            })
            .collect();
        let homes = load_homes(&db)?;
        let nicks = db.users()?.into_iter().collect();
        let descriptions = db
            .descriptions()?
            .into_iter()
            .map(|(x, y, t)| ((x, y), t))
            .collect();
        let plantings = db
            .plantings()?
            .into_iter()
            .map(|p| ((p.x, p.y), p))
            .collect();
        let mut world = World {
            seed,
            tiles,
            things,
            homes,
            nicks,
            descriptions,
            plantings,
            weather: Weather::new(SIZE, seed),
            db,
        };
        world.ensure_docks()?;
        world.ensure_lighthouse()?;
        Ok(world)
    }

    pub fn tile(&self, x: i64, y: i64) -> Option<&Tile> {
        in_bounds(x, y).then(|| &self.tiles[idx(x, y)])
    }

    pub fn terrain(&self, x: i64, y: i64) -> Terrain {
        // Indoors: a floor, never water, where nothing grows or turns up.
        if in_room_space(x, y) {
            return Terrain::Dirt;
        }
        self.tile(x, y).map_or(Terrain::DeepWater, |t| t.terrain)
    }

    /// The building a room is inside, if it's still standing.
    pub fn room_building(&self, x: i64, y: i64) -> Option<&Thing> {
        let id = room_of(x, y)?;
        self.things.iter().find(|t| t.id == id)
    }

    /// Where (x, y) is out on the island: the building's spot, for a room.
    pub fn outside(&self, x: i64, y: i64) -> (i64, i64) {
        match room_of(x, y) {
            Some(_) => self
                .room_building(x, y)
                .map_or_else(|| self.spawn_point(), |b| (b.x, b.y)),
            None => (x, y),
        }
    }

    pub fn set_terrain(&mut self, x: i64, y: i64, terrain: Terrain) -> anyhow::Result<()> {
        if !in_bounds(x, y) {
            return Ok(());
        }
        self.tiles[idx(x, y)].terrain = terrain;
        // Reshaping the ground clears whatever was growing there.
        if self.plantings.remove(&(x, y)).is_some() {
            self.db.remove_planting(x, y)?;
        }
        self.persist_tile(x, y)
    }

    /// Something newly planted at (x, y), starting to grow now.
    pub fn plant(&mut self, x: i64, y: i64, kind: &str, by: &str, at: i64) -> anyhow::Result<()> {
        let row = PlantingRow {
            x,
            y,
            kind: kind.to_string(),
            by: by.to_string(),
            at,
            told: 0,
        };
        self.db.set_planting(&row)?;
        self.plantings.insert((x, y), row);
        Ok(())
    }

    /// Its planter has heard it's grown this far.
    pub fn told_planter(&mut self, x: i64, y: i64, stage: i64) -> anyhow::Result<()> {
        if let Some(p) = self.plantings.get_mut(&(x, y)) {
            p.told = stage;
            self.db.set_planting(p)?;
        }
        Ok(())
    }

    /// The ground's map symbol, drawn lighter while something planted there
    /// is still young.
    fn glyph_at(&self, x: i64, y: i64) -> (&'static str, Col) {
        let t = self.terrain(x, y);
        let young = self
            .plantings
            .get(&(x, y))
            .and_then(|p| super::gardens::stage_of(p, now()))
            .filter(|(p, s)| *s < super::gardens::grown(p.kind));
        match young {
            Some((p, _)) if p.kind == super::gardens::Kind::Tree => ("♣", Col::Green),
            Some(_) => ("\"", Col::Pink),
            None => t.glyph(),
        }
    }

    pub fn set_description(
        &mut self,
        x: i64,
        y: i64,
        text: Option<String>,
        by: &str,
        at: i64,
    ) -> anyhow::Result<()> {
        self.db.set_description(x, y, text.as_deref(), by, at)?;
        match text {
            Some(t) => self.descriptions.insert((x, y), t),
            None => self.descriptions.remove(&(x, y)),
        };
        Ok(())
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
        let row = TileRow {
            x,
            y,
            terrain: t.terrain.code().into(),
            name: t.name.clone(),
        };
        self.db.save_tiles(&[row])
    }

    /// Someone's nickname, from their username (or the text itself, for
    /// things like "the island").
    pub fn nick(&self, user: &str) -> String {
        self.nicks
            .get(user)
            .cloned()
            .unwrap_or_else(|| user.to_string())
    }

    /// The home (x, y) is part of. A room inside a building counts as part
    /// of the home the building stands in, so the same people may change it.
    pub fn home_at(&self, x: i64, y: i64) -> Option<&Home> {
        let (x, y) = if room_of(x, y).is_some() {
            self.room_building(x, y).map(|b| (b.x, b.y))?
        } else {
            (x, y)
        };
        self.homes.iter().find(|h| h.covers(x, y))
    }

    pub fn place_title(&self, x: i64, y: i64) -> String {
        if room_of(x, y).is_some() {
            return match self.room_building(x, y) {
                Some(b) => format!(
                    "Inside {}'s {}",
                    self.nick(&b.maker),
                    strip_article(&b.name)
                ),
                None => "Somewhere Indoors".into(),
            };
        }
        match self.tile(x, y) {
            Some(Tile { name: Some(n), .. }) => n.clone(),
            Some(_) if self.home_at(x, y).is_some() => {
                let h = self.home_at(x, y).unwrap();
                let what = if (h.x, h.y) == (x, y) {
                    "Home"
                } else {
                    "Garden"
                };
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
            paint: None,
        })?;
        self.things.push(Thing {
            id,
            x,
            y,
            name,
            action,
            trigger,
            maker: maker.into(),
            kind: kind.map(Into::into),
            paint: None,
        });
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

    /// Every island has a little dock with a rowing boat, on the shore facing
    /// the middle of the archipelago. Put up any that are missing.
    pub fn ensure_docks(&mut self) -> anyhow::Result<()> {
        let mid = (SIZE / 2, SIZE / 2);
        for island in &super::islands::ISLANDS {
            if self
                .things
                .iter()
                .any(|t| t.is_dock() && island.covers(t.x, t.y))
            {
                continue;
            }
            let shore = |x: i64, y: i64| {
                let t = self.terrain(x, y);
                t.walkable()
                    && !t.is_wet()
                    && DIRS.iter().any(|d| self.terrain(x + d.2, y + d.3).is_wet())
            };
            let spot = (island.oy..island.oy + super::islands::REGION)
                .flat_map(|y| (island.ox..island.ox + super::islands::REGION).map(move |x| (x, y)))
                .filter(|&(x, y)| shore(x, y))
                .min_by_key(|&(x, y)| (x - mid.0).pow(2) + (y - mid.1).pow(2));
            if let Some((x, y)) = spot {
                self.add_thing(
                    x,
                    y,
                    "a little wooden dock, with a rowing boat tied up".into(),
                    None,
                    Trigger::Always,
                    "the island",
                    Some("dock"),
                )?;
                if self.tile(x, y).is_some_and(|t| t.name.is_none()) {
                    self.set_name(x, y, Some(format!("{} Dock", island.name)))?;
                }
            }
        }
        Ok(())
    }

    /// An old ruined lighthouse on Driftwood Isle's outermost point, for
    /// everyone to mend together. Put up once.
    pub fn ensure_lighthouse(&mut self) -> anyhow::Result<()> {
        if self.things.iter().any(|t| t.is_lighthouse()) {
            return Ok(());
        }
        let island = &super::islands::ISLANDS[1];
        let mid = (SIZE / 2, SIZE / 2);
        let shore = |x: i64, y: i64| {
            let t = self.terrain(x, y);
            t.walkable()
                && !t.is_wet()
                && DIRS.iter().any(|d| self.terrain(x + d.2, y + d.3).is_wet())
        };
        let spot = (island.oy..island.oy + super::islands::REGION)
            .flat_map(|y| (island.ox..island.ox + super::islands::REGION).map(move |x| (x, y)))
            .filter(|&(x, y)| {
                shore(x, y)
                    && self.home_at(x, y).is_none()
                    && !self.things_at(x, y).any(|t| t.is_dock())
            })
            .max_by_key(|&(x, y)| (x - mid.0).pow(2) + (y - mid.1).pow(2));
        if let Some((x, y)) = spot {
            self.add_thing(
                x,
                y,
                "an old lighthouse".into(),
                None,
                Trigger::Always,
                "the island",
                Some("lighthouse"),
            )?;
            if self.tile(x, y).is_some_and(|t| t.name.is_none()) {
                self.set_name(x, y, Some("Lighthouse Point".into()))?;
            }
        }
        Ok(())
    }

    /// Every named place and home on every island, by island then name:
    /// (x, y, name, island).
    pub fn places(&self) -> Vec<(i64, i64, String, &'static super::islands::Island)> {
        let mut out: Vec<(i64, i64, String, &'static super::islands::Island)> = Vec::new();
        for y in 0..SIZE {
            for x in 0..SIZE {
                if let Some(n) = self.tile(x, y).and_then(|t| t.name.clone()) {
                    out.push((x, y, n, super::islands::island_at(x, y)));
                }
            }
        }
        for h in &self.homes {
            if !out.iter().any(|p| (p.0, p.1) == (h.x, h.y)) {
                out.push((
                    h.x,
                    h.y,
                    self.place_title(h.x, h.y),
                    super::islands::island_at(h.x, h.y),
                ));
            }
        }
        out.sort_by_key(|p| (p.3.id, p.2.to_lowercase()));
        out
    }

    /// Whether everyone's mended and lit the lighthouse yet.
    pub fn lighthouse_lit(&self) -> bool {
        self.db
            .meta("lighthouse_stage")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0)
            >= 4
    }

    /// Bring back the last thing `by` unmade in the past day.
    pub fn undo_remove(&mut self, by: &str) -> anyhow::Result<Option<&Thing>> {
        let since = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs() as i64
            - 86_400;
        let Some(r) = self.db.last_removed(by, since) else {
            return Ok(None);
        };
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
            paint: r.paint,
        });
        Ok(self.things.last())
    }

    /// Fold one account into another, here and in the db.
    pub fn fold_account(&mut self, old: &str, new: &str) -> anyhow::Result<()> {
        self.db.fold_account(old, new)?;
        for t in self.things.iter_mut().filter(|t| t.maker == old) {
            t.maker = new.to_string();
        }
        self.homes = load_homes(&self.db)?;
        self.nicks.remove(old);
        Ok(())
    }

    pub fn paint_thing(&mut self, i: usize, paint: Option<String>) -> anyhow::Result<()> {
        self.db.paint_thing(self.things[i].id, paint.as_deref())?;
        self.things[i].paint = paint;
        Ok(())
    }

    pub fn paint_home(&mut self, owner: &str, paint: Option<String>) -> anyhow::Result<()> {
        self.db.paint_home(owner, paint.as_deref())?;
        if let Some(h) = self.homes.iter_mut().find(|h| h.owner == owner) {
            h.paint = paint;
        }
        Ok(())
    }

    pub fn move_thing(&mut self, i: usize, x: i64, y: i64) -> anyhow::Result<()> {
        let t = &mut self.things[i];
        t.x = x;
        t.y = y;
        self.db.move_thing(t.id, x, y)
    }

    /// Where newcomers arrive: the middle of Home Island.
    pub fn spawn_point(&self) -> (i64, i64) {
        let home = &super::islands::ISLANDS[0];
        let (cx, cy) = home.centre();
        let mut best = (cx, cy);
        let mut best_d = i64::MAX;
        for y in home.oy..home.oy + super::islands::REGION {
            for x in home.ox..home.ox + super::islands::REGION {
                let c = (cx, cy);
                let t = self.terrain(x, y);
                if matches!(t, Terrain::Grass | Terrain::Meadow | Terrain::Path) {
                    let d = (x - c.0).pow(2) + (y - c.1).pow(2);
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

        let title = paint::glyph_colour(
            self.home_at(x, y).and_then(|h| h.paint.as_deref()),
            Col::Magenta,
        );
        out.push(
            Line::new()
                .t("")
                .b(title, format!("~ {} ~", self.place_title(x, y))),
        );

        if room_of(x, y).is_some() {
            return self.describe_room(x, y, others, out);
        }

        // Someone's own words for the place, or the land's.
        // (or what's been planted here, as it is now).
        let growing = self
            .plantings
            .get(&(x, y))
            .and_then(|p| super::gardens::stage_of(p, now()));
        let mut text = match (self.descriptions.get(&(x, y)), growing) {
            (Some(d), _) => d.clone(),
            (None, Some((plant, stage))) => super::gardens::looks(plant, stage, Clock::season()),
            (None, None) => {
                rng::pick(here.descriptions_in(Clock::season()), self.seed, x, y).to_string()
            }
        };
        text.push(' ');
        text.push_str(&atmosphere(
            here,
            sky,
            phase,
            Clock::season(),
            self.weather.breezy(),
            self.seed,
            x,
            y,
        ));
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

        self.describe_contents(x, y, others, &mut out);
        let exits: Vec<&str> = DIRS
            .iter()
            .filter(|(_, _, dx, dy)| self.terrain(x + dx, y + dy).walkable())
            .map(|d| d.0)
            .collect();
        out.push(
            Line::new()
                .c(Col::Dim, "exits: ")
                .c(Col::Green, exits.join(" ")),
        );
        out
    }

    /// Inside a building: its owner's words for the room, or a cosy default.
    fn describe_room(&self, x: i64, y: i64, others: &[String], mut out: Vec<Line>) -> Vec<Line> {
        let text = match (self.descriptions.get(&(x, y)), self.room_building(x, y)) {
            (Some(d), _) => d.clone(),
            (None, Some(b)) => format!(
                "A snug little room inside the {}. Light comes in through a small window, and the walls still smell of the island.",
                strip_article(&b.name)
            ),
            (None, None) => "A snug little room.".into(),
        };
        out.push(Line::plain(text));
        self.describe_contents(x, y, others, &mut out);
        out.push(Line::new().c(Col::Dim, "exits: ").c(Col::Green, "out"));
        out
    }

    /// The things and people at a place, for `describe`.
    fn describe_contents(&self, x: i64, y: i64, others: &[String], out: &mut Vec<Line>) {
        let things: Vec<&Thing> = self.things_at(x, y).collect();
        if !things.is_empty() {
            // Each in its own paint, so a pink cottage looks pink.
            let mut line = Line::new().c(Col::Yellow, "✦ ");
            for (i, t) in things.iter().enumerate() {
                if i > 0 {
                    let sep = match (things.len(), i == things.len() - 1) {
                        (2, _) => " and ",
                        (_, true) => ", and ",
                        _ => ", ",
                    };
                    line = line.c(Col::Yellow, sep);
                }
                let name = if i == 0 {
                    capitalise(&t.name)
                } else {
                    t.name.clone()
                };
                let from = line.0.len();
                line = paint::painted(line, &name, t.paint.as_deref(), Col::Yellow);
                // Tap a thing to look at it (and see what you could do with it).
                let look = format!("look {}", strip_article(&t.name));
                for span in &mut line.0[from..] {
                    span.run = Some(look.clone());
                }
                if let Some(p) = &t.paint {
                    line = line.c(Col::Dim, format!(" (painted {p})"));
                }
            }
            out.push(line);
        }
        if !others.is_empty() {
            let verb = if others.len() == 1 { "is" } else { "are" };
            // Each name a tap, to say something to them.
            let mut line = Line::new().c(Col::Cyan, "☺ ");
            for (i, name) in others.iter().enumerate() {
                if i > 0 {
                    let sep = match (others.len(), i == others.len() - 1) {
                        (2, _) => " and ",
                        (_, true) => ", and ",
                        _ => ", ",
                    };
                    line = line.c(Col::Cyan, sep);
                }
                line = line.who(name);
            }
            out.push(line.c(Col::Cyan, format!(" {verb} here.")));
        }
    }

    /// The whole island at half scale (each character is a 2x2 patch), with
    /// you, friends, homes, boards and numbered named places.
    pub fn island_map(&self, me: (i64, i64), people: &[(i64, i64)]) -> Vec<Line> {
        const KEYS: &str = "123456789abcdefghijklmnopqrstuvwxyz";
        let island = super::islands::island_at(me.0, me.1);
        let region = super::islands::REGION;
        let mut named: Vec<(i64, i64, String)> = Vec::new();
        for y in island.oy..island.oy + region {
            for x in island.ox..island.ox + region {
                // Docks show as ⚓ rather than a number, and a lit lighthouse as ☼.
                let dock = self.things.iter().any(|t| {
                    (t.is_dock() || (t.is_lighthouse() && self.lighthouse_lit()))
                        && (t.x, t.y) == (x, y)
                });
                if let Some(n) = self
                    .tile(x, y)
                    .and_then(|t| t.name.clone())
                    .filter(|_| !dock)
                {
                    named.push((x, y, n));
                }
            }
        }
        for h in self.homes.iter().filter(|h| island.covers(h.x, h.y)) {
            if !named.iter().any(|(x, y, _)| (*x, *y) == (h.x, h.y)) {
                named.push((h.x, h.y, format!("{}'s Home", self.nick(&h.owner))));
            }
        }
        named.truncate(KEYS.len());
        let lit = self.lighthouse_lit();
        let cell = |x: i64, y: i64| (x / 2, y / 2);
        // Crop to this island, with a little sea around it.
        let land: Vec<(i64, i64)> = (island.oy..island.oy + region)
            .flat_map(|y| (island.ox..island.ox + region).map(move |x| (x, y)))
            .filter(|&(x, y)| self.terrain(x, y) != Terrain::DeepWater)
            .map(|(x, y)| cell(x, y))
            .collect();
        let (x0, x1) = (
            land.iter().map(|c| c.0).min().unwrap_or(0) - 1,
            land.iter().map(|c| c.0).max().unwrap_or(SIZE / 2) + 1,
        );
        let (y0, y1) = (
            land.iter().map(|c| c.1).min().unwrap_or(0) - 1,
            land.iter().map(|c| c.1).max().unwrap_or(SIZE / 2) + 1,
        );
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
                } else if let Some(h) = self.homes.iter().find(|h| covers((h.x, h.y))) {
                    line = line.b(paint::glyph_colour(h.paint.as_deref(), Col::Magenta), "⌂");
                } else if self
                    .things
                    .iter()
                    .any(|t| t.is_dock() && covers((t.x, t.y)))
                {
                    line = line.b(Col::Brown, "⚓");
                } else if lit
                    && self
                        .things
                        .iter()
                        .any(|t| t.is_lighthouse() && covers((t.x, t.y)))
                {
                    line = line.b(Col::Yellow, "☼");
                } else if self
                    .things
                    .iter()
                    .any(|t| t.is_board() && covers((t.x, t.y)))
                {
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
                    let t = counts
                        .iter()
                        .max_by_key(|(_, n)| *n)
                        .map_or(Terrain::DeepWater, |c| c.0);
                    let (g, c) = match [(0, 0), (1, 0), (0, 1), (1, 1)]
                        .iter()
                        .map(|(dx, dy)| (cx * 2 + dx, cy * 2 + dy))
                        .find(|&(x, y)| {
                            self.terrain(x, y) == t && self.plantings.contains_key(&(x, y))
                        }) {
                        Some((x, y)) => self.glyph_at(x, y),
                        None => t.glyph(),
                    };
                    line = line.c(c, g);
                }
            }
            out.push(line);
        }
        let mut key = Line::new()
            .b(Col::Magenta, "@")
            .t(" you  ")
            .b(Col::White, "☺")
            .t(" friends  ")
            .b(Col::Magenta, "⌂")
            .t(" homes  ")
            .b(Col::Yellow, "▣")
            .t(" boards  ")
            .b(Col::Brown, "⚓")
            .t(" dock");
        if lit
            && self
                .things
                .iter()
                .any(|t| t.is_lighthouse() && island.covers(t.x, t.y))
        {
            key = key.t("  ").b(Col::Yellow, "☼").t(" lighthouse");
        }
        out.push(key);
        for (i, (_, _, n)) in named.iter().enumerate() {
            out.push(
                Line::new()
                    .b(Col::Yellow, format!("{} ", KEYS.chars().nth(i).unwrap()))
                    .t(n.clone()),
            );
        }
        out
    }

    /// A small coloured map centred on (cx, cy).
    pub fn minimap(
        &self,
        cx: i64,
        cy: i64,
        people: &[(i64, i64)],
        finds: &[(i64, i64)],
    ) -> Vec<Line> {
        let (rw, rh) = MINIMAP_REACH;
        let people: HashSet<(i64, i64)> = people.iter().copied().collect();
        let lit = self.lighthouse_lit();
        let mut out = Vec::new();
        for y in cy - rh..=cy + rh {
            // Neighbouring cells of one colour share a span, to keep it small.
            let mut spans: Vec<Span> = Vec::new();
            for x in cx - rw..=cx + rw {
                let (g, c, b) = if x == cx && y == cy {
                    ("@", Col::Magenta, true)
                } else if people.contains(&(x, y)) {
                    ("☺", Col::White, true)
                } else if let Some(h) = self.homes.iter().find(|h| (h.x, h.y) == (x, y)) {
                    (
                        "⌂",
                        paint::glyph_colour(h.paint.as_deref(), Col::Magenta),
                        true,
                    )
                } else if self.things_at(x, y).any(|t| t.is_board()) {
                    ("▣", Col::Yellow, true)
                } else if lit && self.things_at(x, y).any(|t| t.is_lighthouse()) {
                    ("☼", Col::Yellow, true)
                } else if let Some(t) = self.things_at(x, y).next() {
                    (
                        "✦",
                        paint::glyph_colour(
                            self.things_at(x, y)
                                .find_map(|t| t.paint.as_deref())
                                .or(t.paint.as_deref()),
                            Col::Yellow,
                        ),
                        false,
                    )
                } else if finds.contains(&(x, y)) {
                    ("•", Col::Pink, true)
                } else if !in_bounds(x, y) {
                    ("≈", Col::DeepBlue, false)
                } else {
                    let (g, c) = self.glyph_at(x, y);
                    (g, c, false)
                };
                let gap = if x != cx + rw { " " } else { "" };
                match spans.last_mut() {
                    Some(last) if last.c == c && last.b == b => {
                        last.t.push_str(g);
                        last.t.push_str(gap);
                    }
                    _ => spans.push(Span {
                        t: format!("{g}{gap}"),
                        c,
                        b,
                        run: None,
                        fill: None,
                        eerie: false,
                    }),
                }
            }
            out.push(Line(spans));
        }
        out
    }
}

/// How far the minimap sees: steps left and right, and up and down.
pub const MINIMAP_REACH: (i64, i64) = (7, 4);

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
        (Season::Autumn, Meadow | Grass) => {
            "The grass has turned gold, and there's a crispness in the air."
        }
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
    let mut s = if snow {
        light
            .replace("drizzle", "flurry of snow")
            .replace("Rain falls", "Snow falls")
    } else {
        light.to_string()
    };
    if sky == Sky::Rain && phase != Phase::Day {
        s.push_str(if snow {
            " Snow falls steadily."
        } else {
            " Rain falls steadily."
        });
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
        s.push_str(rng::pick::<&str>(
            &[
                "A breeze tugs at your clothes.",
                "The wind is up, and lively.",
                "Gusts of wind come and go.",
            ],
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

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

pub fn in_bounds(x: i64, y: i64) -> bool {
    (0..SIZE).contains(&x) && (0..SIZE).contains(&y)
}

fn idx(x: i64, y: i64) -> usize {
    (y * SIZE + x) as usize
}

/// Every home, with its guests and its paint.
fn load_homes(db: &Db) -> anyhow::Result<Vec<Home>> {
    let paints = db.home_paints()?;
    Ok(db
        .homes()?
        .into_iter()
        .map(|(owner, x, y, guests)| Home {
            paint: paints.get(&owner).cloned(),
            owner,
            x,
            y,
            guests,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rooms_stay_on_their_islands_side() {
        use super::super::islands::{REGION, island_at};
        for (bx, by) in [
            (5, 5),
            (REGION + 5, 5),
            (5, REGION + 5),
            (REGION + 5, REGION + 5),
        ] {
            let (rx, ry) = room_for(42, bx, by);
            assert_eq!(room_of(rx, ry), Some(42));
            assert_eq!(
                island_at(rx, ry).id,
                island_at(bx, by).id,
                "a room is on its building's island"
            );
            assert!(in_room_space(rx, ry) && !in_room_space(bx, by));
        }
        assert_eq!(room_of(5, 5), None);
        assert_ne!(
            room_for(1, 5, 5),
            room_for(2, 5, 5),
            "every building has its own room"
        );
        let (a, b) = (room_for(1, 5, 5), room_for(2, 5, 5));
        assert!(
            (a.0 - b.0).abs() >= ROOM_STRIDE,
            "rooms are far enough apart that nothing carries between them"
        );
    }

    /// `cargo test -- --nocapture print_island` to eyeball the generator.
    #[test]
    fn every_island_has_plenty_of_land_and_a_sea_around_it() {
        for island in &super::super::islands::ISLANDS {
            let rows = island.generate(7);
            let land = rows
                .iter()
                .filter(|r| {
                    let t = Terrain::from_code(&r.terrain);
                    t.walkable() && !t.is_wet()
                })
                .count();
            assert!(land > 300, "{} is too small: {land}", island.name);
            let edge = rows
                .iter()
                .filter(|r| r.x == island.ox || r.y == island.oy)
                .all(|r| r.terrain == "deep");
            assert!(edge, "{} touches its neighbours", island.name);
        }
    }
}
