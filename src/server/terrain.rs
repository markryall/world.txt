//! What the ground is made of, and how it looks and reads.

use crate::protocol::Col;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terrain {
    DeepWater,
    Water,
    Pond,
    Sand,
    Grass,
    Meadow,
    Forest,
    Hill,
    Stone,
    Path,
    Dirt,
}

use Terrain::*;

pub const ALL: [Terrain; 11] = [
    DeepWater, Water, Pond, Sand, Grass, Meadow, Forest, Hill, Stone, Path, Dirt,
];

impl Terrain {
    pub fn code(self) -> &'static str {
        match self {
            DeepWater => "deep",
            Water => "water",
            Pond => "pond",
            Sand => "sand",
            Grass => "grass",
            Meadow => "meadow",
            Forest => "forest",
            Hill => "hill",
            Stone => "stone",
            Path => "path",
            Dirt => "dirt",
        }
    }

    pub fn from_code(s: &str) -> Terrain {
        ALL.into_iter().find(|t| t.code() == s).unwrap_or(Grass)
    }

    pub fn glyph(self) -> (&'static str, Col) {
        match self {
            DeepWater => ("≈", Col::DeepBlue),
            Water => ("~", Col::Blue),
            Pond => ("~", Col::Cyan),
            Sand => (".", Col::Sand),
            Grass => ("\"", Col::Green),
            Meadow => ("*", Col::Pink),
            Forest => ("♣", Col::DarkGreen),
            Hill => ("^", Col::Brown),
            Stone => ("▲", Col::Grey),
            Path => ("·", Col::Yellow),
            Dirt => (",", Col::Brown),
        }
    }

    pub fn is_wet(self) -> bool {
        matches!(self, DeepWater | Water | Pond)
    }

    pub fn walkable(self) -> bool {
        self != DeepWater
    }

    /// Short name used for titles and for "to the north, ..." hints.
    pub fn place_name(self) -> &'static str {
        match self {
            DeepWater => "Open Sea",
            Water => "Shallows",
            Pond => "Little Pond",
            Sand => "Sandy Shore",
            Grass => "Grassy Field",
            Meadow => "Wildflower Meadow",
            Forest => "Quiet Woods",
            Hill => "Gentle Hill",
            Stone => "Stony Height",
            Path => "Worn Path",
            Dirt => "Turned Earth",
        }
    }

    pub fn nearby(self) -> &'static str {
        match self {
            DeepWater => "the open sea",
            Water => "shallow water",
            Pond => "a little pond",
            Sand => "pale sand",
            Grass => "soft grass",
            Meadow => "a haze of wildflowers",
            Forest => "dark trees",
            Hill => "a grassy rise",
            Stone => "grey rocks",
            Path => "a worn path",
            Dirt => "freshly turned earth",
        }
    }

    /// Descriptions for this time of year: meadows aren't buzzing in winter.
    pub fn descriptions_in(self, season: super::weather::Season) -> &'static [&'static str] {
        use super::weather::Season::*;
        match (self, season) {
            (Meadow, Autumn) => &[
                "A meadow of tall gold grass and seed heads, rattling softly in the wind.",
                "The last few flowers hold on here, among the golden grasses.",
            ],
            (Meadow, Winter) => &[
                "A quiet meadow, asleep for winter. Brown seed heads poke up through the frost.",
                "The meadow is resting: flattened grass, bare stems, and a robin hopping about.",
            ],
            _ => self.descriptions(),
        }
    }

    pub fn descriptions(self) -> &'static [&'static str] {
        match self {
            DeepWater => &["Cold deep water heaves gently all around you."],
            Water => &[
                "You stand knee-deep in clear, cool water. Small fish dart around your ankles.",
                "The water here is shallow and sun-warmed, the sandy bottom rippled like a fingerprint.",
                "Gentle waves lap at your shins. Strands of weed sway beneath the surface.",
            ],
            Pond => &[
                "A small, still pond. Its surface holds a perfect copy of the sky.",
                "You're up to your knees in a little pond. Tadpoles scatter from your feet.",
            ],
            Sand => &[
                "Pale sand, warm and soft, shifts under your feet. Shells glint here and there.",
                "A strip of sandy shore, scattered with driftwood worn silver by the sea.",
                "The sand here is packed and damp, crisscrossed with tiny bird tracks.",
            ],
            Grass => &[
                "Soft green grass rolls away in every direction, nodding in the breeze.",
                "An open field of short, springy grass. Clover grows thick in the hollows.",
                "Long grass whispers around your knees. Somewhere a cricket is keeping time.",
            ],
            Meadow => &[
                "A meadow bursting with wildflowers: poppies, daisies, little blue cornflowers.",
                "Bees drift lazily between the flowers here. The air smells of honey.",
                "Tall flowers sway around you, purple and yellow and white.",
            ],
            Forest => &[
                "Tall trees close in overhead, their leaves filtering the light into green and gold.",
                "A hush of old trees. Moss covers the roots and the air smells of earth.",
                "The woods are thick here. Ferns brush your legs and a woodpecker knocks somewhere above.",
            ],
            Hill => &[
                "You stand on a gentle hill. The land falls away around you, and you can see a long way.",
                "A grassy rise, scattered with rabbit holes. The wind is stronger up here.",
            ],
            Stone => &[
                "Grey stones and lichen. The rock is warm where the sun has touched it.",
                "A rocky height. Tufts of thyme grow in the cracks between boulders.",
            ],
            Path => &[
                "A worn little path, trodden smooth by many feet.",
                "A winding path of packed earth, edged with pebbles.",
            ],
            Dirt => &[
                "Freshly turned earth, dark and crumbly. It smells of rain.",
                "A patch of bare soil, waiting for something to grow.",
            ],
        }
    }
}
