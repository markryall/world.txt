//! Little things to find while wandering: flowers, butterflies, shells,
//! mushrooms, feathers, stones and fish. They appear near people, depending
//! on the ground, the weather and the time of day, and fade if nobody
//! gathers them.

use super::rng::Rng;
use super::terrain::Terrain;
use super::weather::{Phase, Season, Sky};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Flower,
    Butterfly,
    Bug,
    Mushroom,
    Shell,
    Feather,
    Stone,
    Fish,
    Berry,
    Leaf,
    /// Made by crafting, never found lying about.
    Craft,
}

impl Kind {
    pub const ALL: [Kind; 10] = [
        Kind::Flower,
        Kind::Butterfly,
        Kind::Bug,
        Kind::Mushroom,
        Kind::Shell,
        Kind::Feather,
        Kind::Stone,
        Kind::Fish,
        Kind::Berry,
        Kind::Leaf,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Kind::Flower => "🌼 Flowers",
            Kind::Butterfly => "🦋 Butterflies & moths",
            Kind::Bug => "🐞 Beetles & bugs",
            Kind::Mushroom => "🍄 Mushrooms",
            Kind::Shell => "🐚 Seashore",
            Kind::Feather => "🪶 Feathers",
            Kind::Stone => "💎 Stones",
            Kind::Fish => "🐟 Fish",
            Kind::Berry => "🫐 Berries",
            Kind::Leaf => "🍂 Leaves & seeds",
            Kind::Craft => "✂️ Crafts",
        }
    }
    /// Living things get released rather than placed.
    pub fn alive(self) -> bool {
        matches!(self, Kind::Butterfly | Kind::Bug | Kind::Fish)
    }
    /// Chance a catch attempt fails and it gets away.
    fn escape(self) -> f64 {
        match self {
            Kind::Butterfly => 0.35,
            Kind::Bug => 0.2,
            _ => 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
}

impl Rarity {
    fn weight(self) -> f64 {
        match self {
            Rarity::Common => 10.0,
            Rarity::Uncommon => 3.5,
            Rarity::Rare => 0.8,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "",
            Rarity::Uncommon => " (uncommon)",
            Rarity::Rare => " (rare!)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    Any,
    Day,
    Night,
    Rain,
    DryDay,
    /// Only in this season.
    In(Season),
    /// Only during this season's festival.
    Festival(Season),
}

impl When {
    fn ok(self, sky: Sky, phase: Phase, season: Season, festival: bool) -> bool {
        match self {
            When::In(s) => s == season,
            When::Festival(s) => festival && s == season,
            When::Any => true,
            When::Day => phase != Phase::Night,
            When::Night => phase == Phase::Night || phase == Phase::Dusk,
            When::Rain => sky.wet(),
            When::DryDay => phase != Phase::Night && !sky.wet(),
        }
    }
}

pub struct Species {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: Kind,
    pub rarity: Rarity,
    pub when: When,
    pub ground: &'static [Terrain],
    /// How it looks when you come across it: "{A} {name} {here}".
    pub here: &'static str,
}

use Kind::*;
use Rarity::*;
use Terrain as T;
use When::*;

const MEADOWY: &[Terrain] = &[T::Meadow, T::Grass];
const MEADOW: &[Terrain] = &[T::Meadow];
const WOODS: &[Terrain] = &[T::Forest];
const SHORE: &[Terrain] = &[T::Sand];
const HIGH: &[Terrain] = &[T::Hill, T::Stone];
const ROCKY: &[Terrain] = &[T::Stone];
const POND: &[Terrain] = &[T::Pond];
const SEA: &[Terrain] = &[T::Water, T::DeepWater];

pub const SPECIES: &[Species] = &[
    // flowers
    Species { id: "daisy", name: "daisy", kind: Flower, rarity: Common, when: Any, ground: MEADOWY, here: "grows here, bright and cheerful" },
    Species { id: "buttercup", name: "buttercup", kind: Flower, rarity: Common, when: Any, ground: MEADOWY, here: "shines yellow in the grass" },
    Species { id: "poppy", name: "red poppy", kind: Flower, rarity: Common, when: Any, ground: MEADOW, here: "nods on a thin stem" },
    Species { id: "cornflower", name: "cornflower", kind: Flower, rarity: Uncommon, when: Any, ground: MEADOW, here: "peeks out, a perfect blue" },
    Species { id: "foxglove", name: "foxglove", kind: Flower, rarity: Uncommon, when: Any, ground: &[T::Meadow, T::Forest], here: "stands tall, hung with purple bells" },
    Species { id: "orchid", name: "wild orchid", kind: Flower, rarity: Rare, when: Any, ground: MEADOW, here: "hides among the grasses, delicate and pink" },
    Species { id: "moonflower", name: "moonflower", kind: Flower, rarity: Rare, when: Night, ground: MEADOWY, here: "has opened, pale and glowing in the dark" },
    // butterflies & moths
    Species { id: "cabbage_white", name: "cabbage white butterfly", kind: Butterfly, rarity: Common, when: DryDay, ground: MEADOWY, here: "dances over the flowers" },
    Species { id: "common_blue", name: "common blue butterfly", kind: Butterfly, rarity: Common, when: DryDay, ground: MEADOWY, here: "rests with its wings open" },
    Species { id: "red_admiral", name: "red admiral butterfly", kind: Butterfly, rarity: Uncommon, when: DryDay, ground: &[T::Meadow, T::Forest], here: "suns itself on a leaf" },
    Species { id: "peacock", name: "peacock butterfly", kind: Butterfly, rarity: Uncommon, when: DryDay, ground: MEADOW, here: "flashes the eyes on its wings" },
    Species { id: "swallowtail", name: "swallowtail butterfly", kind: Butterfly, rarity: Rare, when: DryDay, ground: MEADOW, here: "glides past, huge and golden" },
    Species { id: "luna_moth", name: "luna moth", kind: Butterfly, rarity: Rare, when: Night, ground: WOODS, here: "drifts by, pale green and ghostly" },
    // bugs
    Species { id: "ladybird", name: "ladybird", kind: Bug, rarity: Common, when: Day, ground: &[T::Meadow, T::Grass, T::Forest], here: "climbs a blade of grass" },
    Species { id: "firefly", name: "firefly", kind: Bug, rarity: Common, when: Night, ground: &[T::Meadow, T::Grass, T::Forest], here: "blinks softly in the dark" },
    Species { id: "stag_beetle", name: "stag beetle", kind: Bug, rarity: Uncommon, when: Any, ground: WOODS, here: "trundles over a mossy log" },
    Species { id: "jewel_beetle", name: "jewel beetle", kind: Bug, rarity: Rare, when: Day, ground: WOODS, here: "shimmers green and gold on a branch" },
    // mushrooms
    Species { id: "button", name: "button mushroom", kind: Mushroom, rarity: Common, when: Any, ground: WOODS, here: "pokes up from the leaf litter" },
    Species { id: "chanterelle", name: "chanterelle", kind: Mushroom, rarity: Uncommon, when: Rain, ground: WOODS, here: "glows orange after the rain" },
    Species { id: "fly_agaric", name: "red-and-white toadstool", kind: Mushroom, rarity: Uncommon, when: Any, ground: WOODS, here: "sits under a tree like a tiny house" },
    Species { id: "glowcap", name: "glowcap", kind: Mushroom, rarity: Rare, when: Night, ground: WOODS, here: "gives off a faint blue light" },
    // seashore
    Species { id: "cockle", name: "cockle shell", kind: Shell, rarity: Common, when: Any, ground: SHORE, here: "lies half-buried in the sand" },
    Species { id: "scallop", name: "scallop shell", kind: Shell, rarity: Common, when: Any, ground: SHORE, here: "gleams at the water's edge" },
    Species { id: "sea_glass", name: "piece of sea glass", kind: Shell, rarity: Uncommon, when: Any, ground: SHORE, here: "glints green in the sand" },
    Species { id: "starfish", name: "starfish", kind: Shell, rarity: Uncommon, when: Any, ground: SHORE, here: "has washed up, orange and five-armed" },
    Species { id: "conch", name: "conch shell", kind: Shell, rarity: Rare, when: Any, ground: SHORE, here: "sits on the sand, swirled and pink" },
    // feathers
    Species { id: "pigeon_feather", name: "grey feather", kind: Feather, rarity: Common, when: Any, ground: &[T::Hill, T::Grass], here: "flutters in the grass" },
    Species { id: "magpie_feather", name: "magpie feather", kind: Feather, rarity: Uncommon, when: Any, ground: &[T::Hill, T::Forest], here: "shines black and blue" },
    Species { id: "owl_feather", name: "owl feather", kind: Feather, rarity: Rare, when: Night, ground: &[T::Forest, T::Hill], here: "lies soft and speckled on the ground" },
    // stones
    Species { id: "pebble", name: "smooth pebble", kind: Stone, rarity: Common, when: Any, ground: &[T::Stone, T::Hill, T::Sand, T::Path], here: "is just the right size for a pocket" },
    Species { id: "quartz", name: "quartz crystal", kind: Stone, rarity: Uncommon, when: Any, ground: HIGH, here: "sparkles between the rocks" },
    Species { id: "geode", name: "geode", kind: Stone, rarity: Rare, when: Any, ground: ROCKY, here: "looks plain outside, but rattles when you nudge it" },
    // fish (caught with `fish`, never lying about)
    Species { id: "minnow", name: "minnow", kind: Fish, rarity: Common, when: Any, ground: POND, here: "" },
    Species { id: "perch", name: "perch", kind: Fish, rarity: Common, when: Any, ground: POND, here: "" },
    Species { id: "eel", name: "eel", kind: Fish, rarity: Uncommon, when: Rain, ground: POND, here: "" },
    Species { id: "golden_carp", name: "golden carp", kind: Fish, rarity: Rare, when: Any, ground: POND, here: "" },
    Species { id: "mackerel", name: "mackerel", kind: Fish, rarity: Common, when: Any, ground: SEA, here: "" },
    Species { id: "crab", name: "little crab", kind: Fish, rarity: Common, when: Any, ground: SEA, here: "" },
    Species { id: "sea_bass", name: "sea bass", kind: Fish, rarity: Uncommon, when: Any, ground: SEA, here: "" },
    Species { id: "sunfish", name: "ocean sunfish", kind: Fish, rarity: Rare, when: Day, ground: SEA, here: "" },
    // berries (animals love them)
    Species { id: "blackberry", name: "blackberry", kind: Berry, rarity: Common, when: Any, ground: &[T::Forest, T::Meadow], here: "hangs dark and juicy on a bramble" },
    Species { id: "strawberry", name: "wild strawberry", kind: Berry, rarity: Common, when: Any, ground: &[T::Meadow, T::Grass], here: "peeks out red from under its leaves" },
    Species { id: "bilberry", name: "bilberry", kind: Berry, rarity: Uncommon, when: Any, ground: &[T::Hill, T::Forest], here: "grows low and dusty blue" },
    // seasonal
    Species { id: "cherry_blossom", name: "cherry blossom", kind: Flower, rarity: Uncommon, when: In(Season::Spring), ground: MEADOWY, here: "drifts down, pale and pink" },
    Species { id: "dragonfly", name: "dragonfly", kind: Bug, rarity: Common, when: In(Season::Summer), ground: MEADOWY, here: "zips past on shimmering wings" },
    Species { id: "golden_leaf", name: "golden leaf", kind: Leaf, rarity: Common, when: In(Season::Autumn), ground: &[T::Forest, T::Grass, T::Meadow], here: "spins down from a branch" },
    Species { id: "maple_leaf", name: "red maple leaf", kind: Leaf, rarity: Uncommon, when: In(Season::Autumn), ground: &[T::Forest, T::Hill], here: "glows red, like a little flame" },
    Species { id: "acorn", name: "acorn", kind: Leaf, rarity: Common, when: In(Season::Autumn), ground: WOODS, here: "sits snug in its tiny cap" },
    Species { id: "snowdrop", name: "snowdrop", kind: Flower, rarity: Uncommon, when: In(Season::Winter), ground: &[T::Meadow, T::Grass, T::Forest], here: "nods its white head above the frost" },
    Species { id: "pinecone", name: "pinecone", kind: Leaf, rarity: Common, when: In(Season::Winter), ground: WOODS, here: "lies under the trees, dusted with frost" },
    // festival keepsakes: found only on festival days, and they glow once placed
    Species { id: "golden_blossom", name: "golden blossom", kind: Craft, rarity: Rare, when: Festival(Season::Spring), ground: &[T::Meadow, T::Grass, T::Forest], here: "shimmers like a little piece of sunlight" },
    Species { id: "glowing_shell", name: "glowing shell", kind: Craft, rarity: Rare, when: Festival(Season::Summer), ground: SHORE, here: "glows softly, like a tiny moon" },
    Species { id: "paper_lantern", name: "paper lantern", kind: Craft, rarity: Rare, when: Festival(Season::Autumn), ground: &[T::Meadow, T::Grass, T::Forest, T::Path, T::Sand], here: "glows warm and orange" },
    Species { id: "snow_globe", name: "snow globe", kind: Craft, rarity: Rare, when: Festival(Season::Winter), ground: &[T::Meadow, T::Grass, T::Forest, T::Path, T::Hill], here: "swirls with tiny snowflakes" },
    Species { id: "leaf_crown", name: "leaf crown", kind: Craft, rarity: Common, when: Any, ground: &[], here: "rustles in the breeze" },
    // crafts: `here` is what they do once placed, and `when` is when they do it
    Species { id: "flower_crown", name: "flower crown", kind: Craft, rarity: Common, when: Any, ground: &[], here: "smells sweetly of summer" },
    Species { id: "bouquet", name: "bouquet", kind: Craft, rarity: Common, when: Any, ground: &[], here: "nods its petals in the breeze" },
    Species { id: "wind_chime", name: "shell wind chime", kind: Craft, rarity: Common, when: Any, ground: &[], here: "tinkles softly in the breeze" },
    Species { id: "dreamcatcher", name: "dreamcatcher", kind: Craft, rarity: Common, when: Night, ground: &[], here: "twirls slowly, catching the night's dreams" },
    Species { id: "crystal_lantern", name: "crystal lantern", kind: Craft, rarity: Uncommon, when: Night, ground: &[], here: "glows softly in the dark" },
    Species { id: "mushroom_soup", name: "bowl of mushroom soup", kind: Craft, rarity: Common, when: Any, ground: &[], here: "steams, warm and savoury" },
    Species { id: "shell_necklace", name: "shell necklace", kind: Craft, rarity: Uncommon, when: DryDay, ground: &[], here: "glitters in the sunshine" },
    Species { id: "cairn", name: "little stone cairn", kind: Craft, rarity: Common, when: Any, ground: &[], here: "" },
    Species { id: "conch_horn", name: "conch horn", kind: Craft, rarity: Rare, when: Any, ground: &[], here: "hums with the faraway sound of the sea" },
    Species { id: "garland", name: "rainbow garland", kind: Craft, rarity: Uncommon, when: Any, ground: &[], here: "flutters in every colour" },
];

pub fn by_id(id: &str) -> Option<&'static Species> {
    SPECIES.iter().find(|s| s.id == id)
}

/// "a daisy", "an owl feather"
pub fn a(name: &str) -> String {
    if name.starts_with(['a', 'e', 'i', 'o', 'u']) { format!("an {name}") } else { format!("a {name}") }
}

/// Pick something that could turn up here and now, weighted by rarity.
pub fn choose(
    rng: &mut Rng,
    ground: Terrain,
    sky: Sky,
    phase: Phase,
    season: Season,
    festival: bool,
    fishing: bool,
) -> Option<&'static Species> {
    let options: Vec<&Species> = SPECIES
        .iter()
        .filter(|s| (s.kind == Fish) == fishing && s.ground.contains(&ground) && s.when.ok(sky, phase, season, festival))
        .collect();
    // Festival keepsakes turn up often on the day.
    let weight = |s: &Species| if matches!(s.when, When::Festival(_)) { 12.0 } else { s.rarity.weight() };
    let total: f64 = options.iter().map(|s| weight(s)).sum();
    if total <= 0.0 {
        return None;
    }
    let mut roll = rng.f() * total;
    for s in &options {
        roll -= weight(s);
        if roll <= 0.0 {
            return Some(s);
        }
    }
    options.last().copied()
}

pub struct Find {
    pub species: &'static Species,
    pub x: i64,
    pub y: i64,
    pub until: Instant,
}

/// Does a gather attempt succeed, or does it get away?
pub fn caught(rng: &mut Rng, s: &Species) -> bool {
    !rng.chance(s.kind.escape())
}

pub fn lifetime(rng: &mut Rng) -> Instant {
    Instant::now() + Duration::from_secs(rng.range(240.0, 480.0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn something_for_every_kind_of_ground_people_wander() {
        let mut rng = Rng::new(1);
        for season in [Season::Spring, Season::Summer, Season::Autumn, Season::Winter] {
            for ground in [T::Meadow, T::Grass, T::Forest, T::Sand, T::Hill, T::Stone] {
                assert!(choose(&mut rng, ground, Sky::Clear, Phase::Day, season, false, false).is_some(), "{ground:?}");
            }
        }
        assert!(choose(&mut rng, T::Pond, Sky::Clear, Phase::Day, Season::Spring, false, true).is_some());
        assert!(choose(&mut rng, T::Water, Sky::Clear, Phase::Night, Season::Winter, false, true).is_some());
    }

    #[test]
    fn seasons_and_festivals_bring_their_own_finds() {
        let mut rng = Rng::new(3);
        let mut found = std::collections::HashSet::new();
        for _ in 0..400 {
            if let Some(s) = choose(&mut rng, T::Forest, Sky::Clear, Phase::Day, Season::Autumn, true, false) {
                found.insert(s.id);
            }
        }
        assert!(found.contains("golden_leaf") && found.contains("paper_lantern"), "{found:?}");
        assert!(!found.contains("snowdrop") && !found.contains("snow_globe"), "{found:?}");
    }

    #[test]
    fn ids_are_unique() {
        for (i, s) in SPECIES.iter().enumerate() {
            assert!(SPECIES[i + 1..].iter().all(|o| o.id != s.id), "{}", s.id);
        }
    }
}
