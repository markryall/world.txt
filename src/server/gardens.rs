//! Gardens that grow: what people plant comes up slowly, over real days.
//! A sapling becomes a young tree, then a tall one, then an old one; shoots
//! come into bud, then bloom. Nothing wilts and nothing needs tending:
//! patience is the whole of it. It's all told in words, never numbers.

use super::db::PlantingRow;
use super::weather::Season;

const HOUR: i64 = 3600;
const DAY: i64 = 24 * HOUR;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Tree,
    Flowers,
}

pub struct Plant {
    pub id: &'static str,
    /// "birch", "bluebells"
    pub name: &'static str,
    /// Just one: "apple", "bluebell"
    one: &'static str,
    pub kind: Kind,
    /// When grown, by season: spring, summer, autumn, winter.
    seasons: [&'static str; 4],
}

use Kind::*;

pub const PLANTS: &[Plant] = &[
    Plant {
        id: "oak",
        name: "oak",
        one: "oak",
        kind: Tree,
        seasons: [
            "Its new leaves are soft and pale.",
            "",
            "Acorns are dropping from it, one by one.",
            "Its bare branches spread wide against the sky.",
        ],
    },
    Plant {
        id: "birch",
        name: "birch",
        one: "birch",
        kind: Tree,
        seasons: [
            "Catkins dangle from its twigs.",
            "Its leaves flicker silver in the breeze.",
            "Its leaves have turned to little gold coins.",
            "Its white bark glows against the grey.",
        ],
    },
    Plant {
        id: "willow",
        name: "willow",
        one: "willow",
        kind: Tree,
        seasons: [
            "Its long branches are hazed with green.",
            "Its branches trail down like a curtain.",
            "Its narrow leaves are turning yellow.",
            "Its bare whips sway in the wind.",
        ],
    },
    Plant {
        id: "apple",
        name: "apple tree",
        one: "apple",
        kind: Tree,
        seasons: [
            "Apple blossom, white and pink, covers its branches.",
            "Little green apples are swelling among the leaves.",
            "Red apples hang heavy on its branches.",
            "Its knobbly branches are bare and patient.",
        ],
    },
    Plant {
        id: "cherry",
        name: "cherry tree",
        one: "cherry",
        kind: Tree,
        seasons: [
            "It's a cloud of pink blossom.",
            "Dark cherries hide among its leaves.",
            "Its leaves have turned a deep, glowing red.",
            "Its bark shines like polished copper in the cold.",
        ],
    },
    Plant {
        id: "pine",
        name: "pine",
        one: "pine",
        kind: Tree,
        seasons: [
            "New candles of growth stand up from its branches.",
            "It smells warm and resinous in the sun.",
            "Its cones creak open in the dry air.",
            "Snow rests on its dark needles.",
        ],
    },
    Plant {
        id: "rowan",
        name: "rowan",
        one: "rowan",
        kind: Tree,
        seasons: [
            "It's covered in creamy flowers.",
            "Green berries are forming in clusters.",
            "Bright red berries hang from it in bunches.",
            "Birds have been at its last few berries.",
        ],
    },
    Plant {
        id: "bluebells",
        name: "bluebells",
        one: "bluebell",
        kind: Flowers,
        seasons: ["A haze of bluebells nods here, smelling sweet.", "", "", ""],
    },
    Plant {
        id: "poppies",
        name: "poppies",
        one: "poppy",
        kind: Flowers,
        seasons: ["", "Red poppies blaze here, papery and bright.", "", ""],
    },
    Plant {
        id: "sunflowers",
        name: "sunflowers",
        one: "sunflower",
        kind: Flowers,
        seasons: [
            "",
            "Sunflowers stand tall, their faces turned to the sun.",
            "The sunflowers hang their heavy heads, full of seeds.",
            "",
        ],
    },
    Plant {
        id: "lavender",
        name: "lavender",
        one: "lavender",
        kind: Flowers,
        seasons: [
            "",
            "Lavender grows in soft purple rows, humming with bees.",
            "",
            "",
        ],
    },
    Plant {
        id: "foxgloves",
        name: "foxgloves",
        one: "foxglove",
        kind: Flowers,
        seasons: ["", "Foxgloves stand tall, hung with purple bells.", "", ""],
    },
];

pub fn by_id(id: &str) -> Option<&'static Plant> {
    PLANTS.iter().find(|p| p.id == id)
}

/// What someone asked to plant: a tree or flowers by name, or just "tree"
/// or "flowers" (None: let the island choose). Some(None) for grass.
pub fn wanted(words: &str) -> Option<(Kind, Option<&'static Plant>)> {
    let w = words.to_lowercase();
    if let Some(p) = PLANTS
        .iter()
        .find(|p| w.contains(p.id) || w.contains(p.one))
    {
        return Some((p.kind, Some(p)));
    }
    if ["tree", "forest", "wood", "seed", "sapling"]
        .iter()
        .any(|t| w.contains(t))
    {
        return Some((Tree, None));
    }
    if ["flower", "bulb", "meadow", "bloom"]
        .iter()
        .any(|t| w.contains(t))
    {
        return Some((Flowers, None));
    }
    None
}

/// The island's choice: a willow by water, otherwise anything that suits.
pub fn choose(kind: Kind, near_water: bool, roll: u64) -> &'static Plant {
    if kind == Tree && near_water {
        return by_id("willow").expect("willows");
    }
    let these: Vec<&Plant> = PLANTS
        .iter()
        .filter(|p| p.kind == kind && p.id != "willow")
        .collect();
    these[(roll % these.len() as u64) as usize]
}

/// How grown something is, from 0 (just planted) to fully grown.
pub fn stage(kind: Kind, age: i64) -> i64 {
    let steps: &[i64] = match kind {
        Tree => &[DAY, 3 * DAY, 10 * DAY],
        Flowers => &[2 * HOUR, 12 * HOUR],
    };
    steps.iter().filter(|&&s| age >= s).count() as i64
}

pub fn grown(kind: Kind) -> i64 {
    match kind {
        Tree => 2,
        Flowers => 2,
    }
}

/// "a young birch", "bluebells in bud"
pub fn label(p: &Plant, stage: i64) -> String {
    match (p.kind, stage) {
        (Tree, 0) => format!("{} sapling", super::nature::a(p.one)),
        (Tree, 1) => format!("a young {}", p.name),
        (Tree, 2) => format!("a tall {}", p.name),
        (Tree, _) => format!("an old {}", p.name),
        (Flowers, 0) => format!("{} shoots", p.one),
        (Flowers, 1) => format!("{} in bud", p.name),
        (Flowers, _) => format!("{} in bloom", p.name),
    }
}

fn season_index(s: Season) -> usize {
    match s {
        Season::Spring => 0,
        Season::Summer => 1,
        Season::Autumn => 2,
        Season::Winter => 3,
    }
}

/// How it looks now.
pub fn looks(p: &Plant, stage: i64, season: Season) -> String {
    let base = match (p.kind, stage) {
        (Tree, 0) => format!(
            "A little {} sapling, no taller than your knee, held straight by a stake.",
            p.one
        ),
        (Tree, 1) => format!(
            "A young {}, slim and a little taller than you, its leaves catching the light.",
            p.name
        ),
        (Tree, 2) => format!("A tall {}, its branches wide enough to sit under.", p.name),
        (Tree, _) => format!(
            "An old {}, its trunk grooved and mossy, its roots gripping the earth as if it's always been here.",
            p.name
        ),
        (Flowers, 0) => format!(
            "Little green shoots of {} are pushing up through the earth.",
            p.name
        ),
        (Flowers, 1) => format!("The {} are in bud, nearly ready.", p.name),
        (Flowers, _) => format!("The {} are in bloom.", p.name),
    };
    let flourish = if stage >= grown(p.kind) {
        p.seasons[season_index(season)]
    } else {
        ""
    };
    match (p.kind, flourish) {
        // Blooming flowers say it best in their own words.
        (Flowers, f) if !f.is_empty() => f.to_string(),
        (_, "") => base,
        (_, f) => format!("{base} {f}"),
    }
}

/// Who planted what's growing here, and when: "You planted the birch here
/// 2 days ago." Where someone's own words have taken the place of how it
/// looks, it says how it's coming on too, so it's never a mystery.
pub fn planted(p: &Plant, stage: i64, who: &str, ago: &str, unseen: bool) -> String {
    let now = match (p.kind, stage, unseen) {
        (_, _, false) => String::new(),
        (Tree, s, _) => format!(", and it's {} now", label(p, s)),
        (Flowers, 0, _) => ", and they're just shoots so far".into(),
        (Flowers, 1, _) => ", and they're in bud".into(),
        (Flowers, _, _) => ", and they're in bloom".into(),
    };
    format!("{who} planted the {} here {ago}{now}.", p.name)
}

/// When it grows a stage: "has grown into a young birch".
pub fn grew(p: &Plant, stage: i64) -> String {
    match p.kind {
        Tree => format!("has grown into {}", label(p, stage)),
        Flowers if stage == 1 => "are in bud".to_string(),
        Flowers => "are in bloom".to_string(),
    }
}

/// "just now", "yesterday", "3 days ago"
pub fn ago(secs: i64) -> String {
    match secs {
        ..HOUR => "just now".into(),
        HOUR..DAY => {
            if secs < 2 * HOUR {
                "an hour ago".into()
            } else {
                format!("{} hours ago", secs / HOUR)
            }
        }
        DAY..172_800 => "yesterday".into(),
        172_800..1_209_600 => format!("{} days ago", secs / DAY),
        _ => format!("{} weeks ago", secs / (7 * DAY)),
    }
}

/// Where a planting is in its life, now.
pub fn stage_of(row: &PlantingRow, now: i64) -> Option<(&'static Plant, i64)> {
    let p = by_id(&row.kind)?;
    Some((p, stage(p.kind, now - row.at)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trees_take_days_and_flowers_hours() {
        assert_eq!(stage(Tree, 0), 0);
        assert_eq!(stage(Tree, DAY - 1), 0);
        assert_eq!(stage(Tree, DAY), 1);
        assert_eq!(stage(Tree, 4 * DAY), 2);
        assert_eq!(stage(Tree, 30 * DAY), 3);
        assert_eq!(stage(Flowers, HOUR), 0);
        assert_eq!(stage(Flowers, 3 * HOUR), 1);
        assert_eq!(stage(Flowers, DAY), 2);
    }

    #[test]
    fn knows_what_you_want_to_plant() {
        assert_eq!(
            wanted("an apple tree").map(|w| w.1.map(|p| p.id)),
            Some(Some("apple"))
        );
        assert_eq!(
            wanted("a birch").map(|w| w.1.map(|p| p.id)),
            Some(Some("birch"))
        );
        assert_eq!(
            wanted("some sunflower seeds").map(|w| w.1.map(|p| p.id)),
            Some(Some("sunflowers"))
        );
        assert_eq!(
            wanted("a poppy").map(|w| w.1.map(|p| p.id)),
            Some(Some("poppies"))
        );
        assert_eq!(
            wanted("tree").map(|w| (w.0, w.1.is_none())),
            Some((Tree, true))
        );
        assert_eq!(
            wanted("flowers").map(|w| (w.0, w.1.is_none())),
            Some((Flowers, true))
        );
        assert!(wanted("grass").is_none());
        assert!(wanted("").is_none());
    }

    #[test]
    fn willows_by_water() {
        assert_eq!(choose(Tree, true, 3).id, "willow");
        for roll in 0..20 {
            let p = choose(Tree, false, roll);
            assert!(p.kind == Tree && p.id != "willow");
            assert_eq!(choose(Flowers, true, roll).kind, Flowers);
        }
    }

    #[test]
    fn says_what_was_planted() {
        let birch = by_id("birch").unwrap();
        assert_eq!(
            planted(birch, 1, "You", "2 days ago", false),
            "You planted the birch here 2 days ago."
        );
        assert_eq!(
            planted(birch, 1, "You", "2 days ago", true),
            "You planted the birch here 2 days ago, and it's a young birch now."
        );
        assert_eq!(
            planted(by_id("bluebells").unwrap(), 2, "Poppy", "yesterday", true),
            "Poppy planted the bluebells here yesterday, and they're in bloom."
        );
    }

    #[test]
    fn reads_nicely() {
        let apple = by_id("apple").unwrap();
        assert_eq!(label(apple, 0), "an apple sapling");
        assert_eq!(label(by_id("poppies").unwrap(), 0), "poppy shoots");
        assert_eq!(label(by_id("birch").unwrap(), 0), "a birch sapling");
        assert_eq!(label(by_id("birch").unwrap(), 3), "an old birch");
        assert_eq!(label(by_id("bluebells").unwrap(), 0), "bluebell shoots");
        assert!(looks(apple, 2, Season::Autumn).contains("Red apples"));
        assert!(
            !looks(apple, 1, Season::Autumn).contains("Red apples"),
            "too young to fruit"
        );
        assert_eq!(
            looks(by_id("poppies").unwrap(), 2, Season::Summer),
            "Red poppies blaze here, papery and bright."
        );
        assert_eq!(
            looks(by_id("poppies").unwrap(), 2, Season::Winter),
            "The poppies are in bloom."
        );
        assert_eq!(ago(30), "just now");
        assert_eq!(ago(DAY + 5), "yesterday");
        assert_eq!(ago(3 * DAY), "3 days ago");
    }
}
