//! The archipelago: four islands, each in its own 64x64 patch of sea, with
//! deep water between them. Docks on each shore have a rowing boat for
//! sailing between them.

use super::db::TileRow;
use super::rng::fbm;
use super::terrain::Terrain;

/// Each island's patch of sea.
pub const REGION: i64 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Home,
    Beach,
    Woods,
    Rocky,
}

pub struct Island {
    pub id: u8,
    pub name: &'static str,
    /// Top-left corner of its patch.
    pub ox: i64,
    pub oy: i64,
    pub style: Style,
    pub blurb: &'static str,
}

pub const ISLANDS: [Island; 4] = [
    Island { id: 0, name: "Home Island", ox: 0, oy: 0, style: Style::Home, blurb: "meadows, woods and a rocky heart" },
    Island { id: 1, name: "Driftwood Isle", ox: REGION, oy: 0, style: Style::Beach, blurb: "long beaches, pearls and driftwood" },
    Island { id: 2, name: "Mossy Holm", ox: 0, oy: REGION, style: Style::Woods, blurb: "deep green woods and giant mushrooms" },
    Island { id: 3, name: "Ember Rock", ox: REGION, oy: REGION, style: Style::Rocky, blurb: "warm stone heights full of gems" },
];

pub fn island_at(x: i64, y: i64) -> &'static Island {
    let (rx, ry) = (x.div_euclid(REGION).clamp(0, 1), y.div_euclid(REGION).clamp(0, 1));
    ISLANDS.iter().find(|i| i.ox == rx * REGION && i.oy == ry * REGION).unwrap_or(&ISLANDS[0])
}

pub fn by_name(words: &str) -> Option<&'static Island> {
    let w = words.trim().to_lowercase();
    if w.is_empty() {
        return None;
    }
    ISLANDS.iter().find(|i| i.name.to_lowercase().contains(&w) || i.name.to_lowercase().split(' ').next() == w.split(' ').next())
}

impl Island {
    pub fn covers(&self, x: i64, y: i64) -> bool {
        (self.ox..self.ox + REGION).contains(&x) && (self.oy..self.oy + REGION).contains(&y)
    }
    pub fn centre(&self) -> (i64, i64) {
        (self.ox + REGION / 2, self.oy + REGION / 2)
    }

    /// Grow this island's patch. Home Island is exactly the original island,
    /// so worlds from before the archipelago keep their shape.
    pub fn generate(&self, seed: u64) -> Vec<TileRow> {
        // (elevation boost, falloff, top of the sand, top of the land, top of the hills, wetness)
        let (lift, falloff, sand, land, hills, wet) = match self.style {
            Style::Home => (0.42, 1.15, 0.37, 0.74, 0.86, 0.0),
            Style::Beach => (0.40, 1.55, 0.48, 0.80, 0.92, -0.05),
            Style::Woods => (0.42, 1.25, 0.36, 0.78, 0.90, 0.22),
            Style::Rocky => (0.50, 1.30, 0.36, 0.60, 0.74, -0.1),
        };
        let seed = if self.style == Style::Home { seed } else { seed ^ (self.id as u64).wrapping_mul(0x9e37_79b9) };
        let mut rows = Vec::new();
        let c = (REGION - 1) as f64 / 2.0;
        for y in 0..REGION {
            for x in 0..REGION {
                let (fx, fy) = (x as f64, y as f64);
                let d = (((fx - c) / c).powi(2) + ((fy - c) / c).powi(2)).sqrt();
                let e = fbm(seed, fx / 11.0, fy / 11.0) + lift - falloff * d * d;
                let m = fbm(seed ^ 0x5eed, fx / 8.0, fy / 8.0) + wet;
                let terrain = if e < 0.22 {
                    Terrain::DeepWater
                } else if e < 0.32 {
                    Terrain::Water
                } else if e < sand {
                    Terrain::Sand
                } else if e < land {
                    if m > 0.58 {
                        Terrain::Forest
                    } else if m < 0.36 {
                        Terrain::Meadow
                    } else {
                        Terrain::Grass
                    }
                } else if e < hills {
                    Terrain::Hill
                } else {
                    Terrain::Stone
                };
                // A ring of deep sea keeps the islands apart.
                let edge = x < 2 || y < 2 || x >= REGION - 2 || y >= REGION - 2;
                let terrain = if edge { Terrain::DeepWater } else { terrain };
                rows.push(TileRow { x: self.ox + x, y: self.oy + y, terrain: terrain.code().into(), name: None });
            }
        }
        rows
    }
}
