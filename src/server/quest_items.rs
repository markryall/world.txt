//! What the island's little quests are made of: things the folk lose, what
//! they give to say thank you, and things in pieces that come together. All
//! of them are made-up things (never found lying about by chance), so they
//! live here rather than among the island's finds, and none goes in anyone's
//! journal.

use super::nature::{Kind, Rarity, Species, When};
use super::weather::Season;

const fn item(id: &'static str, name: &'static str, here: &'static str) -> Species {
    Species {
        id,
        name,
        kind: Kind::Craft,
        rarity: Rarity::Rare,
        when: When::Any,
        ground: &[],
        here,
    }
}

/// Something one of the folk has lost, and when it can be lost.
pub struct Lost {
    pub item: Species,
    pub season: Option<Season>,
}

const fn lost(id: &'static str, name: &'static str, here: &'static str) -> Lost {
    Lost {
        item: item(id, name, here),
        season: None,
    }
}

const fn lost_in(season: Season, id: &'static str, name: &'static str, here: &'static str) -> Lost {
    Lost {
        item: item(id, name, here),
        season: Some(season),
    }
}

pub const LOST: &[Lost] = &[
    lost(
        "lost_thimble",
        "silver thimble",
        "glints up from the ground",
    ),
    lost(
        "lost_spectacles",
        "pair of reading spectacles",
        "lies folded, as if put down for just a moment",
    ),
    lost(
        "lost_pocket_watch",
        "old pocket watch",
        "ticks away quietly to itself",
    ),
    lost("lost_brass_key", "little brass key", "winks in the light"),
    lost(
        "lost_harmonica",
        "battered harmonica",
        "hums faintly when the wind catches it",
    ),
    lost(
        "lost_sketchbook",
        "dog-eared sketchbook",
        "flutters its pages in the breeze",
    ),
    lost_in(
        Season::Spring,
        "lost_seed_packet",
        "packet of sweet pea seeds",
        "rattles faintly when the wind blows",
    ),
    lost_in(
        Season::Spring,
        "lost_trowel",
        "little garden trowel",
        "is stuck in the ground, handle up",
    ),
    lost_in(
        Season::Summer,
        "lost_sunhat",
        "straw sunhat",
        "sits upside down, as if it blew off",
    ),
    lost_in(
        Season::Summer,
        "lost_bucket",
        "little red bucket",
        "has a few grains of sand still in it",
    ),
    lost_in(
        Season::Autumn,
        "lost_conker",
        "champion conker on a string",
        "is polished to a shine",
    ),
    lost_in(
        Season::Autumn,
        "lost_scarf",
        "long knitted scarf",
        "is draped where the wind left it",
    ),
    lost_in(
        Season::Winter,
        "lost_mitten",
        "red mitten",
        "pokes out of the frost, a bright red thumb",
    ),
    lost_in(
        Season::Winter,
        "lost_bobble_hat",
        "bobble hat",
        "sits with its bobble up, waiting",
    ),
];

/// What the folk give to say thank you: little things for a home.
pub const THANKS: &[Species] = &[
    item(
        "thanks_painted_pebble",
        "little painted pebble",
        "has a tiny smiling face painted on it",
    ),
    item(
        "thanks_bookmark",
        "pressed-flower bookmark",
        "holds a buttercup flat under paper",
    ),
    item(
        "thanks_honey",
        "jar of island honey",
        "glows gold where the light catches it",
    ),
    item(
        "thanks_egg_cosy",
        "knitted egg cosy",
        "sits looking very pleased with itself",
    ),
    item(
        "thanks_wooden_bird",
        "tiny carved wooden bird",
        "looks ready to fly off any moment",
    ),
    item(
        "thanks_map",
        "hand-drawn map of the island",
        "has a little X where someone kind lives",
    ),
];

/// Something in pieces, scattered about an island, that comes whole once
/// every piece is set down in the place it belongs.
pub struct Set {
    pub id: &'static str,
    /// "an old music box"
    pub name: &'static str,
    pub icon: &'static str,
    pub season: Option<Season>,
    pub pieces: [Species; 3],
    /// What everyone who helped gets to keep.
    pub keepsake: Species,
    /// "…{place}…": what happens when it comes together.
    pub whole: &'static str,
}

pub const SETS: &[Set] = &[
    Set {
        id: "music_box",
        name: "an old music box",
        icon: "🎶",
        season: None,
        pieces: [
            item(
                "piece_music_box_case",
                "music box case",
                "is painted with tiny faded roses",
            ),
            item(
                "piece_music_box_comb",
                "music box comb",
                "plinks a note when the wind brushes it",
            ),
            item(
                "piece_music_box_dancer",
                "little music box dancer",
                "stands on one toe, waiting for a tune",
            ),
        ],
        keepsake: item(
            "keepsake_music_box",
            "tiny music box",
            "plays a few tinkling notes now and then",
        ),
        whole: "The music box clicks together, and the little dancer turns as a tune drifts out over {place}.",
    },
    Set {
        id: "treasure_map",
        name: "a torn treasure map",
        icon: "🗺",
        season: None,
        pieces: [
            item(
                "piece_map_compass",
                "map corner with a compass rose",
                "flaps gently, its edge torn",
            ),
            item(
                "piece_map_path",
                "map corner with a dotted path",
                "is rolled up and tied with string",
            ),
            item(
                "piece_map_x",
                "map corner with a big red X",
                "peeks out, the X just showing",
            ),
        ],
        keepsake: item(
            "keepsake_treasure_coin",
            "shiny treasure coin",
            "gleams like real gold",
        ),
        whole: "The corners fit together into a map of the island, and the big red X is right here at {place}! A little tin of treasure is dug up and shared out.",
    },
    Set {
        id: "blossom_garland",
        name: "a garland for the Blossom Picnic",
        icon: "🌸",
        season: Some(Season::Spring),
        pieces: [
            item(
                "piece_pink_ribbon",
                "bundle of pink ribbon",
                "trails in the grass",
            ),
            item(
                "piece_paper_blossoms",
                "basket of paper blossoms",
                "spills a few pink petals",
            ),
            item(
                "piece_garden_twine",
                "spool of garden twine",
                "has rolled to a stop here",
            ),
        ],
        keepsake: item(
            "keepsake_blossom_garland",
            "blossom garland",
            "sways prettily in the breeze",
        ),
        whole: "Ribbon, blossoms and twine come together into a long garland, strung up over {place} ready for the Blossom Picnic.",
    },
    Set {
        id: "lantern_boat",
        name: "a lantern boat for Midsummer Beach Night",
        icon: "⛵",
        season: Some(Season::Summer),
        pieces: [
            item(
                "piece_boat_hull",
                "little wooden boat hull",
                "rocks gently where it lies",
            ),
            item(
                "piece_paper_sail",
                "folded paper sail",
                "flutters, eager for wind",
            ),
            item(
                "piece_candle_stub",
                "stub of a candle",
                "smells faintly of beeswax",
            ),
        ],
        keepsake: item(
            "keepsake_lantern_boat",
            "tiny lantern boat",
            "glows softly, as if bobbing on a dark sea",
        ),
        whole: "Hull, sail and candle make a little lantern boat, waiting at {place} to float out on Midsummer Beach Night.",
    },
    Set {
        id: "great_lantern",
        name: "the great lantern for Lantern Night",
        icon: "🏮",
        season: Some(Season::Autumn),
        pieces: [
            item(
                "piece_lantern_frame",
                "willow lantern frame",
                "is bent into a lovely round shape",
            ),
            item(
                "piece_lantern_paper",
                "sheet of orange lantern paper",
                "rustles like a fallen leaf",
            ),
            item(
                "piece_beeswax_candle",
                "fat beeswax candle",
                "smells of honey",
            ),
        ],
        keepsake: item(
            "keepsake_glass_lantern",
            "tiny glass lantern",
            "glows warm and amber",
        ),
        whole: "Frame, paper and candle make the great lantern, glowing over {place}, ready for Lantern Night.",
    },
    Set {
        id: "snowman_finery",
        name: "the snowman's finery for Snow Lights",
        icon: "⛄",
        season: Some(Season::Winter),
        pieces: [
            item(
                "piece_striped_scarf",
                "long striped scarf",
                "is half buried in the snow",
            ),
            item(
                "piece_top_hat",
                "battered top hat",
                "has a sprig of holly in its band",
            ),
            item(
                "piece_carrot_nose",
                "very fine carrot",
                "is just the right shape for a nose",
            ),
        ],
        keepsake: item(
            "keepsake_knitted_snowman",
            "tiny knitted snowman",
            "looks very dapper in its little scarf",
        ),
        whole: "Scarf, hat and carrot nose: the snowman at {place} is dressed up and ready for Snow Lights.",
    },
];

/// Every quest thing, by id.
pub fn by_id(id: &str) -> Option<&'static Species> {
    LOST.iter()
        .map(|l| &l.item)
        .chain(THANKS)
        .chain(
            SETS.iter()
                .flat_map(|s| s.pieces.iter().chain([&s.keepsake])),
        )
        .find(|s| s.id == id)
}

pub fn set(id: &str) -> Option<&'static Set> {
    SETS.iter().find(|s| s.id == id)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_quest_thing_has_its_own_id() {
        let mut seen = std::collections::HashSet::new();
        let all = LOST.iter().map(|l| &l.item).chain(THANKS).chain(
            SETS.iter()
                .flat_map(|s| s.pieces.iter().chain([&s.keepsake])),
        );
        for s in all {
            assert!(seen.insert(s.id), "{} twice", s.id);
            assert!(
                crate::server::nature::SPECIES.iter().all(|n| n.id != s.id),
                "{} is a find too",
                s.id
            );
            assert!(s.ground.is_empty(), "{} would turn up by chance", s.id);
            assert_eq!(by_id(s.id).map(|f| f.id), Some(s.id));
        }
    }

    #[test]
    fn every_season_has_its_own_lost_things_and_set() {
        for season in [
            Season::Spring,
            Season::Summer,
            Season::Autumn,
            Season::Winter,
        ] {
            assert!(LOST.iter().any(|l| l.season == Some(season)), "{season:?}");
            assert!(SETS.iter().any(|s| s.season == Some(season)), "{season:?}");
        }
        for s in SETS {
            assert!(s.whole.contains("{place}"), "{}", s.id);
        }
    }
}
