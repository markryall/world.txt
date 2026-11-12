//! Companions: wild animals turn up near people now and then. Offer one a
//! berry and it may decide you're friends, then it follows you everywhere.
//!
//! Until then they lead lives of their own: they wander their kind of ground
//! a step at a time, nose at whatever people have left about, size each other
//! up, and put up with a pat, mostly. None of it is in the help.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::nature::{self, Kind};
use crate::server::terrain::Terrain as T;
use crate::server::weather::{Clock, Phase};
use crate::server::world::{DIRS, capitalise, in_room_space};
use std::time::{Duration, Instant};

pub struct Animal {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    /// Only ever found on this island (see islands::ISLANDS), or anywhere.
    pub island: Option<u8>,
    ground: &'static [T],
    night: Option<bool>,
    /// "A fox {here}."
    here: &'static str,
    tricks: &'static [&'static str],
    /// Its tricks in a room, where there are no leaves or clover.
    indoors: &'static [&'static str],
    /// Settling on something soft in a room: "curls up on {}".
    snug: &'static str,
    /// How it gets about: "trots".
    moves: &'static str,
    /// Seconds between steps, at the quickest and slowest.
    pace: (f64, f64),
    /// Patted while it's still wild.
    patted: &'static [&'static str],
    /// What it gets up to on its own.
    busy: &'static [&'static str],
}

pub const ANIMALS: &[Animal] = &[
    Animal {
        id: "fox",
        name: "fox",
        icon: "🦊",
        island: None,
        ground: &[T::Forest],
        night: None,
        here: "peeks out from behind a tree, ears twitching",
        tricks: &[
            "sniffs the air",
            "pounces on a leaf",
            "curls its tail around your feet",
            "yips softly",
        ],
        indoors: &[
            "curls up in the warmest corner, nose tucked under its tail",
            "sits by the window, watching the world go by",
            "chases its own tail round and round, then pretends it didn't",
            "noses at the door, then thinks better of it and lies down",
        ],
        snug: "turns round three times on {} and curls up, nose under its tail",
        moves: "trots",
        pace: (20.0, 50.0),
        patted: &[
            "holds very still and lets you, just once, then skitters back a step",
            "sniffs your fingers and decides you'll do",
            "ducks your hand and gives you a look",
        ],
        busy: &[
            "digs at something only it can smell",
            "pounces on a beetle and misses",
            "sits and washes one paw, very thoroughly",
            "stares into the trees at nothing at all",
            "curls up in a ball with its tail over its nose",
        ],
    },
    Animal {
        id: "duckling",
        name: "duckling",
        icon: "🐥",
        island: None,
        ground: &[T::Pond, T::Water],
        night: Some(false),
        here: "paddles about, peeping",
        tricks: &[
            "peeps happily",
            "waddles in a little circle",
            "shakes water off its fluffy feathers",
        ],
        indoors: &[
            "waddles under the table and peeps from there",
            "peeps at its reflection in something shiny",
            "fluffs itself up and has a little doze",
            "pitter-patters from one end of the room to the other",
        ],
        snug: "settles on {}, fluffed up like a dandelion",
        moves: "paddles",
        pace: (15.0, 40.0),
        patted: &[
            "peeps and lets you stroke its fluffy head",
            "paddles off in a huff, then paddles back",
        ],
        busy: &[
            "upends itself to look for weed, tail in the air",
            "chases a dragonfly and gives up",
            "peeps at its own reflection",
            "has a tremendous splashy bath",
        ],
    },
    Animal {
        id: "rabbit",
        name: "rabbit",
        icon: "🐇",
        island: None,
        ground: &[T::Meadow, T::Grass],
        night: Some(false),
        here: "sits up on its back legs, nose twitching",
        tricks: &[
            "nibbles some clover",
            "does a happy little hop",
            "twitches its nose at you",
        ],
        indoors: &[
            "flops over on its side, utterly content",
            "does a binky across the floor",
            "washes its ears, one at a time",
            "stretches out long and flat, like a little rug",
        ],
        snug: "flops over on {}, utterly content",
        moves: "hops",
        pace: (12.0, 35.0),
        patted: &[
            "freezes, allows exactly one stroke, then flicks its ears",
            "thumps a back foot, but stays put",
            "flops onto its side, utterly unbothered",
        ],
        busy: &[
            "nibbles the clover, ears swivelling",
            "does a sudden twisting leap for no reason at all",
            "washes its face with both paws",
            "sits up tall, keeping watch",
        ],
    },
    Animal {
        id: "hedgehog",
        name: "hedgehog",
        icon: "🦔",
        island: None,
        ground: &[T::Forest, T::Grass, T::Meadow],
        night: Some(true),
        here: "snuffles through the leaves",
        tricks: &[
            "snuffles in the leaves",
            "curls into a prickly ball, then uncurls",
            "trundles along beside you",
        ],
        indoors: &[
            "snuffles along the skirting board",
            "finds a cosy gap behind something and naps in it",
            "grunts contentedly to itself",
            "trundles a slow lap of the room, inspecting everything",
        ],
        snug: "investigates {} very seriously, then falls asleep on it",
        moves: "trundles",
        pace: (30.0, 70.0),
        patted: &[
            "curls into a prickly ball. Careful!",
            "snuffles your fingers, then trundles on as if nothing happened",
        ],
        busy: &[
            "snuffles noisily through the leaves",
            "finds a worm and is very pleased about it",
            "stops, sniffs, and changes its mind about where it was going",
            "grunts contentedly to itself",
        ],
    },
    Animal {
        id: "turtle",
        name: "turtle",
        icon: "🐢",
        island: None,
        ground: &[T::Sand],
        night: None,
        here: "blinks at you from the sand",
        tricks: &[
            "blinks slowly",
            "pokes its head out to look around",
            "trundles very, very slowly after you",
        ],
        indoors: &[
            "finds a warm patch by the window and stays there",
            "blinks slowly at the room",
            "pulls its head in for a nap",
            "sets off across the floor, with no hurry at all",
        ],
        snug: "plods all the way onto {} and stays there",
        moves: "plods",
        pace: (60.0, 140.0),
        patted: &[
            "closes its eyes, which might mean it likes it",
            "pulls its head in, then pokes it out again",
        ],
        busy: &[
            "blinks at the sea for a long time",
            "digs a small hole, then sits in it",
            "nibbles a strand of seaweed, slowly",
        ],
    },
    Animal {
        id: "barn_owl",
        name: "barn owl",
        icon: "🦉",
        island: Some(0),
        ground: &[T::Forest, T::Grass, T::Meadow],
        night: Some(true),
        here: "watches you from a low branch, pale as a moon",
        tricks: &[
            "turns its head almost all the way round",
            "blinks very slowly at you",
            "fluffs up into a soft round ball",
        ],
        indoors: &[
            "perches on the back of a chair like it owns it",
            "watches the window, perfectly still",
            "dozes with one eye open",
            "preens a wing, feather by feather",
        ],
        snug: "settles on {} and fluffs up into a ball",
        moves: "glides",
        pace: (30.0, 70.0),
        patted: &[
            "lets you smooth one wing, then gives a soft hiss",
            "clicks its beak and stays put",
        ],
        busy: &[
            "drifts low over the grass without a sound",
            "listens to the ground, head tilted",
            "gives a long, eerie screech",
            "sits as still as a carving",
        ],
    },
    Animal {
        id: "kookaburra",
        name: "kookaburra",
        icon: "🐦",
        island: Some(0),
        ground: &[T::Forest, T::Grass],
        night: Some(false),
        here: "sits on a branch, eyeing you sideways",
        tricks: &[
            "throws back its head and laughs",
            "bobs its tail up and down",
            "cocks its head at you",
        ],
        indoors: &[
            "laughs at something only it found funny",
            "sits on the windowsill, eyeing the room",
            "bashes a twig against the floor to show who's boss",
            "fluffs up and settles down",
        ],
        snug: "perches on {} and chuckles to itself",
        moves: "flaps",
        pace: (20.0, 50.0),
        patted: &[
            "chuckles and hops along the branch, just out of reach",
            "lets you, then laughs about it",
        ],
        busy: &[
            "starts laughing, and doesn't stop for ages",
            "swoops down on a beetle and flies back up",
            "whacks a worm against a branch",
            "sits very still, watching the grass",
        ],
    },
    Animal {
        id: "curlew",
        name: "curlew",
        icon: "🐦",
        island: Some(0),
        ground: &[T::Grass, T::Meadow],
        night: Some(true),
        here: "stands stock still on long legs, big yellow eyes on you",
        tricks: &[
            "freezes, pretending to be a stick",
            "stalks along beside you on long legs",
            "gives a soft, sad little wail",
        ],
        indoors: &[
            "stands in a corner, pretending to be a hat stand",
            "stalks slowly round the room",
            "stares at the lamp with enormous eyes",
            "folds its long legs and sits down, very carefully",
        ],
        snug: "folds its long legs and settles on {}",
        moves: "stalks",
        pace: (25.0, 60.0),
        patted: &[
            "freezes and pretends you aren't there",
            "stalks off three steps, then stalks back",
        ],
        busy: &[
            "lets out a long, wailing cry into the dark",
            "freezes mid-step and holds it",
            "stalks after a moth",
            "stares at you without blinking",
        ],
    },
    Animal {
        id: "wombat",
        name: "wombat",
        icon: "🐾",
        island: Some(0),
        ground: &[T::Grass, T::Forest, T::Meadow],
        night: Some(true),
        here: "waddles about, round and very determined",
        tricks: &[
            "bumps into your shin on purpose",
            "digs a little hollow and sits in it",
            "scratches its back against your leg",
        ],
        indoors: &[
            "digs at the rug, and is quite disappointed",
            "falls asleep flat on its back",
            "trundles round the room bumping into things",
            "wedges itself under something and snores",
        ],
        snug: "digs at {} until it's just right, then sleeps on it",
        moves: "waddles",
        pace: (30.0, 70.0),
        patted: &[
            "leans its whole weight on your hand",
            "grunts, and carries on regardless",
        ],
        busy: &[
            "digs a hole with great determination",
            "has a long, thorough scratch",
            "munches the grass, chomp chomp",
            "trundles in a straight line, through everything",
        ],
    },
    Animal {
        id: "seal",
        name: "seal",
        icon: "🦭",
        island: Some(1),
        ground: &[T::Sand],
        night: None,
        here: "lolls on the sand, whiskers twitching",
        tricks: &[
            "rolls over on its back",
            "claps its flippers",
            "flumps along beside you",
        ],
        indoors: &[
            "lies on the floor like a big warm bolster",
            "rolls onto its back and waits to be admired",
            "barks at the door, once",
            "flops down with a deep sigh",
        ],
        snug: "flumps onto {} and lies there like a pudding",
        moves: "flumps",
        pace: (40.0, 90.0),
        patted: &[
            "blinks its big dark eyes at you",
            "snorts and rolls away a little",
            "lets you, and sighs happily",
        ],
        busy: &[
            "scratches its chin with a flipper",
            "rolls over to warm the other side",
            "barks at the waves",
            "stretches its head and tail up, like a banana",
        ],
    },
    Animal {
        id: "pelican",
        name: "pelican",
        icon: "🐦",
        island: Some(1),
        ground: &[T::Sand],
        night: Some(false),
        here: "stands on the sand, looking very important",
        tricks: &[
            "opens its enormous beak, then closes it again",
            "waddles after you",
            "shakes out its wings",
        ],
        indoors: &[
            "inspects the room down its long beak",
            "tries to fit something in its pouch, and can't",
            "stands on one leg by the window",
            "clacks its beak at the ceiling",
        ],
        snug: "settles on {} and tucks its beak into its feathers",
        moves: "waddles",
        pace: (25.0, 60.0),
        patted: &[
            "clacks its beak at you, politely",
            "lets you, then waddles off a step",
        ],
        busy: &[
            "checks its pouch for fish, and finds none",
            "stretches its wings out wide to dry",
            "stares out to sea, very serious",
            "gulps at a passing fly",
        ],
    },
    Animal {
        id: "little_penguin",
        name: "little penguin",
        icon: "🐧",
        island: Some(1),
        ground: &[T::Sand],
        night: Some(true),
        here: "waddles up from the water, blue and tiny",
        tricks: &[
            "waddles round your feet",
            "flaps its little flippers",
            "shakes off the sea",
        ],
        indoors: &[
            "waddles round and round the table",
            "stands in the corner, very upright",
            "shuffles under a chair for a nap",
            "preens its blue back",
        ],
        snug: "waddles onto {} and goes to sleep standing up",
        moves: "waddles",
        pace: (20.0, 45.0),
        patted: &[
            "puffs up its chest, then lets you",
            "yips and shuffles back",
        ],
        busy: &[
            "waddles up the beach in a hurry",
            "calls out a funny little bray",
            "stands and shakes, sprinkling water",
            "preens its blue feathers",
        ],
    },
    Animal {
        id: "badger",
        name: "badger",
        icon: "🦡",
        island: Some(2),
        ground: &[T::Forest, T::Grass],
        night: Some(true),
        here: "snuffles along, stripy and serious",
        tricks: &[
            "snuffles the ground",
            "grunts at you",
            "digs at something interesting",
        ],
        indoors: &[
            "digs at the floorboards, hopefully",
            "drags a cushion into a corner",
            "snores in a striped heap",
            "sniffs every corner, very thoroughly",
        ],
        snug: "drags {} into a corner and burrows in",
        moves: "shuffles",
        pace: (30.0, 70.0),
        patted: &[
            "huffs at you, but stays",
            "shuffles round to put its back to you",
        ],
        busy: &[
            "digs for worms with great energy",
            "snuffles noisily under a log",
            "grunts at the moon",
            "scratches itself, all over",
        ],
    },
    Animal {
        id: "red_squirrel",
        name: "red squirrel",
        icon: "🐿️",
        island: Some(2),
        ground: &[T::Forest],
        night: Some(false),
        here: "sits up on a stump, tail in a question mark",
        tricks: &[
            "scampers up you and down again",
            "flicks its tail",
            "nibbles a nut, very fast",
        ],
        indoors: &[
            "hides something behind a cushion",
            "runs along the back of the sofa",
            "sits on the windowsill, chattering",
            "checks every pocket in the room",
        ],
        snug: "curls up on {} with its tail over its nose",
        moves: "scampers",
        pace: (10.0, 30.0),
        patted: &[
            "chatters crossly and darts away a step",
            "lets you touch its tail, just once",
        ],
        busy: &[
            "buries a nut, then forgets where",
            "chatters at a bird",
            "races up a tree and back down",
            "nibbles a pinecone into a little stalk",
        ],
    },
    Animal {
        id: "koala",
        name: "koala",
        icon: "🐨",
        island: Some(2),
        ground: &[T::Forest],
        night: None,
        here: "clings to a tree trunk, mostly asleep",
        tricks: &[
            "opens one eye",
            "hugs your arm sleepily",
            "yawns enormously",
        ],
        indoors: &[
            "falls asleep holding a chair leg",
            "hugs the nearest lamp",
            "stares at nothing, chewing",
            "dozes with its chin on its paws",
        ],
        snug: "climbs onto {} and is asleep before it lands",
        moves: "climbs",
        pace: (90.0, 200.0),
        patted: &[
            "grunts and goes back to sleep",
            "leans into your hand without waking up",
        ],
        busy: &[
            "falls asleep again",
            "munches a leaf, very slowly",
            "shifts to a comfier branch and nods off",
            "grunts a deep, rumbling snore",
        ],
    },
    Animal {
        id: "possum",
        name: "possum",
        icon: "🐾",
        island: Some(2),
        ground: &[T::Forest, T::Grass],
        night: Some(true),
        here: "peers down from a branch, eyes shining",
        tricks: &[
            "hangs by its tail",
            "scurries up your arm and back down",
            "peers at you, upside down",
        ],
        indoors: &[
            "raids the fruit bowl",
            "hangs from the curtain rail by its tail",
            "rummages in a cupboard",
            "curls up on a high shelf",
        ],
        snug: "curls up on {} with its tail wrapped round it",
        moves: "scurries",
        pace: (15.0, 40.0),
        patted: &[
            "hisses a little, then sniffs your fingers",
            "freezes, then decides you're all right",
        ],
        busy: &[
            "rummages for something to eat",
            "hangs by its tail, swinging gently",
            "scurries along a branch",
            "chitters at another possum somewhere",
        ],
    },
    Animal {
        id: "mountain_goat",
        name: "mountain goat",
        icon: "🐐",
        island: Some(3),
        ground: &[T::Hill, T::Stone],
        night: Some(false),
        here: "stands on a rock that seems far too small for it",
        tricks: &[
            "butts your hand gently",
            "bleats at you",
            "hops onto a rock beside you",
        ],
        indoors: &[
            "climbs onto the table and looks proud",
            "chews the corner of a rug",
            "stands on the highest thing in the room",
            "bleats at the ceiling",
        ],
        snug: "stands on {} and refuses to come down",
        moves: "clatters",
        pace: (20.0, 50.0),
        patted: &[
            "butts your hand, gently but firmly",
            "chews your sleeve thoughtfully",
        ],
        busy: &[
            "leaps from rock to rock without a thought",
            "chews something it shouldn't",
            "balances on a ledge, quite unbothered",
            "bleats at the wind",
        ],
    },
    Animal {
        id: "salamander",
        name: "salamander",
        icon: "🦎",
        island: Some(3),
        ground: &[T::Stone, T::Hill],
        night: Some(true),
        here: "glistens black and gold under a rock",
        tricks: &[
            "crawls onto your hand, cool and slow",
            "blinks its gold-ringed eyes",
            "curls its tail",
        ],
        indoors: &[
            "finds the coolest stone on the hearth and stays there",
            "creeps along the skirting board",
            "sits very still by the fire",
            "tucks itself into a dark corner",
        ],
        snug: "creeps onto {} and goes very still",
        moves: "creeps",
        pace: (50.0, 110.0),
        patted: &[
            "sits quite still, cool under your finger",
            "creeps out from under your hand",
        ],
        busy: &[
            "creeps out over a warm stone",
            "snaps up a tiny beetle",
            "glows faintly in the ember light",
            "sits as still as the rock",
        ],
    },
    Animal {
        id: "kangaroo",
        name: "kangaroo",
        icon: "🦘",
        island: Some(3),
        ground: &[T::Grass, T::Hill, T::Meadow],
        night: None,
        here: "leans back on its tail and looks at you",
        tricks: &[
            "bounds round you in a big circle",
            "boxes the air",
            "flops onto its side",
        ],
        indoors: &[
            "bounces into the room and bounces out again",
            "lies on its side by the fire",
            "scratches its tummy, thinking",
            "peers out of the window, ears up",
        ],
        snug: "flops down on {} and scratches its tummy",
        moves: "bounds",
        pace: (15.0, 40.0),
        patted: &[
            "sniffs you, ears twitching, and stays",
            "leans away, then leans back in",
        ],
        busy: &[
            "bounds off a little way and comes back",
            "scratches its chest, thinking",
            "lies down in the sun, legs everywhere",
            "stands up very tall to look around",
        ],
    },
    Animal {
        id: "echidna",
        name: "echidna",
        icon: "🦔",
        island: Some(3),
        ground: &[T::Hill, T::Stone, T::Grass],
        night: None,
        here: "pokes its long snout into the ground",
        tricks: &[
            "snuffles your boot with its long nose",
            "tucks itself into a spiky ball",
            "trundles after you",
        ],
        indoors: &[
            "pokes its snout into every crack in the floor",
            "digs itself halfway into a corner",
            "trundles under the table",
            "noses along the skirting board for ants",
        ],
        snug: "digs itself into {} until only spines show",
        moves: "trundles",
        pace: (30.0, 70.0),
        patted: &[
            "tucks into a spiky ball. Careful!",
            "lets you stroke its nose, just once",
        ],
        busy: &[
            "licks up ants with a long pink tongue",
            "digs straight down until only spines show",
            "snuffles a stone over",
            "trundles along, nose first",
        ],
    },
];

/// How meeting one is kept in the journal, beside the finds.
pub fn met_id(a: &Animal) -> String {
    format!("met:{}", a.id)
}

pub fn animal(id: &str) -> Option<&'static Animal> {
    ANIMALS.iter().find(|a| a.id == id)
}

pub struct Wild {
    pub animal: &'static Animal,
    pub x: i64,
    pub y: i64,
    pub until: Instant,
    /// When it next takes a step.
    pub next_step: Instant,
    /// When it last did something someone saw, so it doesn't go on and on.
    pub last_seen_doing: Instant,
}

/// The way from one spot to the next: "north", or "north-east".
fn way(dx: i64, dy: i64) -> &'static str {
    DIRS.iter()
        .find(|q| q.2 == dx.signum() && q.3 == dy.signum())
        .map_or("nearby", |q| q.1)
}

/// Something soft enough for a companion to settle on: "a feather cushion"
/// becomes "the feather cushion".
fn soft(name: &str) -> Option<String> {
    const SOFT: &[&str] = &[
        "cushion", "rug", "pillow", "blanket", "beanbag", "bed", "sofa", "couch", "armchair",
        "mat", "quilt",
    ];
    let words: Vec<&str> = name.split_whitespace().collect();
    let bare = match words.first() {
        Some(&"a" | &"an" | &"the" | &"some") => &words[1..],
        _ => &words[..],
    };
    bare.iter()
        .any(|w| {
            SOFT.contains(
                &w.to_lowercase()
                    .trim_matches(|c: char| !c.is_alphanumeric()),
            )
        })
        .then(|| format!("the {}", bare.join(" ")))
}

/// A companion's friendly label: "Rusty the fox", or "your fox".
pub fn label(animal: &Animal, name: &Option<String>) -> String {
    match name {
        Some(n) => format!("{n} the {}", animal.name),
        None => format!("the {}", animal.name),
    }
}

impl Game {
    /// Now and then, an animal wanders out near someone.
    pub(super) fn animals_tick(&mut self) {
        let now = Instant::now();
        self.wild.retain(|w| w.until > now);
        let night = Clock::phase() == Phase::Night;
        let spots: Vec<(u64, i64, i64)> = self
            .players
            .iter()
            .filter(|(_, p)| p.about())
            .map(|(&id, p)| (id, p.x, p.y))
            .collect();
        for (id, px, py) in spots {
            let near = self
                .wild
                .iter()
                .any(|w| (w.x - px).abs() <= 6 && (w.y - py).abs() <= 4);
            if near || self.wild.len() >= 12 || !self.rng.chance(0.1) {
                continue;
            }
            // Right beside you, so "just to the north" is exactly one step.
            let dx = (self.rng.next() % 3) as i64 - 1;
            let dy = (self.rng.next() % 3) as i64 - 1;
            let (x, y) = (px + dx, py + dy);
            let ground = self.world.terrain(x, y);
            let island = crate::server::islands::island_at(x, y).id;
            let options: Vec<&'static Animal> = ANIMALS
                .iter()
                .filter(|a| {
                    a.ground.contains(&ground)
                        && a.night.is_none_or(|n| n == night)
                        && a.island.is_none_or(|i| i == island)
                })
                .collect();
            if options.is_empty() {
                continue;
            }
            let a = *self.rng.pick(&options);
            let until = now + Duration::from_secs(self.rng.range(360.0, 720.0) as u64);
            let next_step = now + Duration::from_secs_f64(self.rng.range(a.pace.0, a.pace.1));
            self.wild.push(Wild {
                animal: a,
                x,
                y,
                until,
                next_step,
                last_seen_doing: now,
            });
            let d = dx.abs().max(dy.abs());
            let line = if d == 0 {
                format!(
                    "{} {} {} right here!",
                    a.icon,
                    capitalise(&nature::a(a.name)),
                    a.here
                )
            } else {
                format!(
                    "{} {} {}, just to the {}.",
                    a.icon,
                    capitalise(&nature::a(a.name)),
                    a.here,
                    way(dx, dy)
                )
            };
            self.say1(id, Line::col(Col::Green, line).eerie());
            self.meet(id, a);
        }

        self.wild_wander(now);
        self.wild_busy(now);

        // Companions get up to little things.
        let pets: Vec<(i64, i64, String)> = self
            .players
            .values()
            .filter_map(|p| p.companion.as_ref().map(|(a, n)| (p.x, p.y, a, n)))
            .filter_map(|(x, y, a, n)| animal(a).map(|an| (x, y, an, n.clone())))
            .filter_map(|(x, y, an, n)| {
                if !self.rng.chance(1.0 / 25.0) {
                    return None;
                }
                let trick = if !in_room_space(x, y) {
                    (*self.rng.pick(an.tricks)).to_string()
                } else if let Some(soft) = self
                    .world
                    .things_at(x, y)
                    .find_map(|t| soft(&t.name))
                    .filter(|_| self.rng.chance(1.0 / 3.0))
                {
                    an.snug.replace("{}", &soft)
                } else {
                    (*self.rng.pick(an.indoors)).to_string()
                };
                Some((
                    x,
                    y,
                    format!("{} {} {trick}.", an.icon, capitalise(&label(an, &n))),
                ))
            })
            .collect();
        for (x, y, line) in pets {
            self.tell_watchers(x, y, Line::col(Col::Green, line));
        }
    }

    /// Each wild animal ambles about its own kind of ground, now and then
    /// drawn towards whoever's close by. People at either end see it go.
    fn wild_wander(&mut self, now: Instant) {
        let night = Clock::phase() == Phase::Night;
        for i in 0..self.wild.len() {
            if self.wild[i].next_step > now {
                continue;
            }
            let (a, x, y) = (self.wild[i].animal, self.wild[i].x, self.wild[i].y);
            self.wild[i].next_step =
                now + Duration::from_secs_f64(self.rng.range(a.pace.0, a.pace.1));
            if self.rng.chance(0.35) {
                continue; // happy where it is, for now
            }
            // Curious: sometimes it heads for the nearest person within a few steps.
            let near = self
                .players
                .values()
                .filter(|p| {
                    p.about() && (p.x, p.y) != (x, y) && (p.x - x).abs().max((p.y - y).abs()) <= 3
                })
                .map(|p| ((p.x - x).signum(), (p.y - y).signum()))
                .next();
            let fits = |g: &Self, dx: i64, dy: i64| {
                a.ground.contains(&g.world.terrain(x + dx, y + dy))
                    && g.world.home_at(x + dx, y + dy).is_none()
            };
            let step = match near {
                Some((dx, dy)) if self.rng.chance(0.4) && fits(self, dx, dy) => Some((dx, dy)),
                _ => {
                    let options: Vec<(i64, i64)> = DIRS
                        .iter()
                        .map(|d| (d.2, d.3))
                        .filter(|&(dx, dy)| fits(self, dx, dy))
                        .collect();
                    (!options.is_empty()).then(|| *self.rng.pick(&options))
                }
            };
            let Some((dx, dy)) = step else { continue };
            let (nx, ny) = (x + dx, y + dy);
            self.wild[i].x = nx;
            self.wild[i].y = ny;
            // Night creatures turn in at dawn, day ones at dusk: it just doesn't come back.
            if a.night.is_some_and(|n| n != night) {
                self.wild[i].until = now;
            }
            let the = format!(
                "{} The {} {} off to the {}.",
                a.icon,
                a.name,
                a.moves,
                way(dx, dy)
            );
            self.tell_watchers(x, y, Line::col(Col::Green, the));
            let arrives = format!(
                "{} {} {} in from the {}.",
                a.icon,
                capitalise(&nature::a(a.name)),
                a.moves,
                way(-dx, -dy)
            );
            self.tell_watchers(nx, ny, Line::col(Col::Green, arrives));
            let here: Vec<u64> = self
                .players
                .iter()
                .filter(|(_, p)| (p.x, p.y) == (nx, ny) && p.about())
                .map(|(&id, _)| id)
                .collect();
            for id in here {
                self.meet(id, a);
            }
        }
    }

    /// The first time someone meets a kind of creature, it goes in their journal.
    pub(super) fn meet(&mut self, id: u64, a: &'static Animal) {
        let Some(p) = self.players.get(&id).filter(|p| !p.bot) else {
            return;
        };
        if !self
            .world
            .db
            .journal_add(&p.user, &met_id(a))
            .unwrap_or(false)
        {
            return;
        }
        let only = a.island.map_or(String::new(), |i| {
            let isle = crate::server::islands::ISLANDS[i as usize % crate::server::islands::ISLANDS.len()].name;
            format!(" (only ever on {isle})")
        });
        self.say1(
            id,
            Line::new()
                .b(Col::Yellow, "📖 New in your journal: ")
                .b(Col::White, format!("{} {}", a.icon, capitalise(a.name)))
                .c(Col::Yellow, only),
        );
    }

    /// With someone watching, a wild animal gets on with its day: its own
    /// business, a nose at what's been left about, or the other animals.
    fn wild_busy(&mut self, now: Instant) {
        for i in 0..self.wild.len() {
            let (a, x, y) = (self.wild[i].animal, self.wild[i].x, self.wild[i].y);
            let watched = self
                .players
                .values()
                .any(|p| (p.x, p.y) == (x, y) && p.about());
            if !watched
                || now.duration_since(self.wild[i].last_seen_doing) < Duration::from_secs(45)
                || !self.rng.chance(1.0 / 12.0)
            {
                continue;
            }
            self.wild[i].last_seen_doing = now;
            let others: Vec<&'static Animal> = self
                .wild
                .iter()
                .enumerate()
                .filter(|(j, w)| *j != i && (w.x, w.y) == (x, y))
                .map(|(_, w)| w.animal)
                .collect();
            let pets: Vec<String> = self
                .players
                .values()
                .filter(|p| (p.x, p.y) == (x, y))
                .filter_map(|p| {
                    p.companion
                        .as_ref()
                        .and_then(|(c, n)| animal(c).map(|an| (an, n.clone())))
                })
                .map(|(an, n)| {
                    if an.id == a.id {
                        format!("same:{}", label(an, &n))
                    } else {
                        label(an, &n)
                    }
                })
                .collect();
            let things: Vec<String> = self.world.things_at(x, y).map(|t| t.name.clone()).collect();
            let roll = self.rng.next() % 4;
            let line = if roll == 0 && !others.is_empty() {
                let b = *self.rng.pick(&others);
                let how = *self.rng.pick(&[
                    "eye each other warily",
                    "touch noses, very briefly",
                    "pretend not to have noticed each other",
                    "have a quiet stand-off over the best spot",
                ]);
                format!("{} The {} and the {} {how}.", a.icon, a.name, b.name)
            } else if roll == 1 && !pets.is_empty() {
                let pet = self.rng.pick(&pets).clone();
                match pet.strip_prefix("same:") {
                    Some(p) => format!(
                        "{} The wild {} and {p} touch noses, then circle each other, tails up.",
                        a.icon, a.name
                    ),
                    None => format!("{} The {} keeps a careful eye on {pet}.", a.icon, a.name),
                }
            } else if roll == 2 && !things.is_empty() {
                let t = super::the(&self.rng.pick(&things).clone());
                let how = *self.rng.pick(&[
                    "sniffs at {t}",
                    "gives {t} a thorough inspection",
                    "sits down beside {t} as if it's always been there",
                    "peers at {t}, then backs off, unsure",
                ]);
                format!("{} The {} {}.", a.icon, a.name, how.replace("{t}", &t))
            } else {
                format!("{} The {} {}.", a.icon, a.name, self.rng.pick(a.busy))
            };
            self.tell_watchers(x, y, Line::col(Col::Green, line));
        }
    }

    /// For `look`: wild animals here, and everyone's companions.
    pub(super) fn animal_lines(&self, id: u64) -> Vec<Line> {
        let Some(me) = self.players.get(&id) else {
            return vec![];
        };
        let mut out = Vec::new();
        for w in self.wild.iter().filter(|w| (w.x, w.y) == (me.x, me.y)) {
            out.push(
                Line::new()
                    .c(
                        Col::Green,
                        format!(
                            "{} {} {}.",
                            w.animal.icon,
                            capitalise(&nature::a(w.animal.name)),
                            w.animal.here
                        ),
                    )
                    .c(
                        Col::Dim,
                        format!(" (befriend {}: it loves berries)", w.animal.name),
                    ),
            );
        }
        for p in self.players.values().filter(|p| (p.x, p.y) == (me.x, me.y)) {
            let Some((a, n)) = &p.companion else { continue };
            let Some(an) = animal(a) else { continue };
            let line = if p.user == me.user {
                format!(
                    "{} {} is right by your side.",
                    an.icon,
                    capitalise(&label(an, n))
                )
            } else {
                format!(
                    "{} {}, {}'s {}, is here.",
                    an.icon,
                    n.clone().unwrap_or_else(|| capitalise(&nature::a(an.name))),
                    p.name,
                    an.name
                )
            };
            out.push(Line::col(Col::Green, line));
        }
        out
    }

    /// "with Rusty the fox" for arrival lines, if they have a companion.
    pub(super) fn with_companion(&self, id: u64) -> String {
        self.players
            .get(&id)
            .and_then(|p| p.companion.as_ref())
            .and_then(|(a, n)| animal(a).map(|an| format!(", with {} {}", an.icon, label(an, n))))
            .unwrap_or_default()
    }

    pub(super) fn cmd_befriend(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let what = crate::server::world::strip_article(what.trim()).to_lowercase();
        let Some(i) = self.wild.iter().position(|w| {
            (w.x, w.y) == (x, y) && (what.is_empty() || w.animal.name.contains(&what))
        }) else {
            self.say1(id, Line::plain("There's no wild animal here to befriend. Keep wandering and they'll turn up! 🦊🐇🦔"));
            return Ok(());
        };
        if let Some((a, n)) = self.players.get(&id).and_then(|p| p.companion.clone()) {
            let current = animal(&a).map_or("friend".to_string(), |an| label(an, &n));
            self.say1(
                id,
                Line::plain(format!(
                    "You already have {current}. (let go of them first, if you're sure)"
                )),
            );
            return Ok(());
        }
        let a = self.wild[i].animal;
        let berries: Vec<(&'static nature::Species, i64)> = self
            .world
            .db
            .satchel(&user)?
            .into_iter()
            .filter_map(|(sid, n)| nature::by_id(&sid).map(|s| (s, n)))
            .filter(|(s, _)| s.kind == Kind::Berry)
            .collect();
        let Some((berry, _)) = berries.iter().max_by_key(|(_, n)| *n) else {
            self.say1(
                id,
                Line::plain(format!(
                    "The {} looks hungry. {}s love berries: gather some first! 🫐",
                    a.name,
                    capitalise(a.name)
                )),
            );
            return Ok(());
        };
        self.world.db.satchel_add(&user, berry.id, -1)?;
        if !self.rng.chance(0.5) {
            self.say1(
                id,
                Line::col(
                    Col::Green,
                    format!(
                        "{} The {} gobbles up your {} and watches you carefully. Maybe one more?",
                        a.icon, a.name, berry.name
                    ),
                ),
            );
            return Ok(());
        }
        self.wild.remove(i);
        self.world.db.set_companion(&user, Some(a.id), None)?;
        if let Some(p) = self.players.get_mut(&id) {
            p.companion = Some((a.id.to_string(), None));
        }
        self.say1(id, Line::new().b(Col::Green, format!("{} The {} eats your {} and decides you're friends! It'll follow you everywhere.", a.icon, a.name, berry.name)));
        self.say1(
            id,
            Line::col(Col::Dim, "Give it a name: petname Rusty · pet · let go"),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Green,
                format!("{} {name} makes friends with a {}!", a.icon, a.name),
            ),
        );
        Ok(())
    }

    pub(super) fn cmd_nickname(&mut self, id: u64, new: &str) -> anyhow::Result<()> {
        let Some((_, _, _name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let Some((a, _)) = self.players.get(&id).and_then(|p| p.companion.clone()) else {
            self.say1(
                id,
                Line::plain("You don't have a companion yet. Befriend an animal with a berry!"),
            );
            return Ok(());
        };
        let nick: String = new
            .trim()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-')
            .take(20)
            .collect();
        if nick.trim().is_empty() {
            self.say1(id, Line::plain("Call them what? e.g. 'petname Rusty'"));
            return Ok(());
        }
        let nick = capitalise(nick.trim());
        self.world.db.set_companion(&user, Some(&a), Some(&nick))?;
        if let Some(p) = self.players.get_mut(&id) {
            p.companion = Some((a.clone(), Some(nick.clone())));
        }
        let icon = animal(&a).map_or("💚", |an| an.icon);
        self.say1(
            id,
            Line::col(
                Col::Green,
                format!("{icon} {nick} it is! They look very pleased."),
            ),
        );
        Ok(())
    }

    pub(super) fn cmd_pet(&mut self, id: u64, what: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        // A wild one here, if that's who they mean (or they've no companion to pat).
        let what = crate::server::world::strip_article(what.trim()).to_lowercase();
        let mine = self
            .players
            .get(&id)
            .and_then(|p| p.companion.as_ref())
            .and_then(|(a, n)| animal(a).map(|an| label(an, n).to_lowercase()));
        let means_mine = mine
            .as_ref()
            .is_some_and(|m| what.is_empty() || m.contains(&what));
        let wild = self.wild.iter().position(|w| {
            (w.x, w.y) == (x, y) && (what.is_empty() || w.animal.name.contains(&what))
        });
        if let (Some(i), false) = (wild, means_mine) {
            let a = self.wild[i].animal;
            let how = *self.rng.pick(a.patted);
            let stop = if how.ends_with('!') { "" } else { "." };
            self.say1(
                id,
                Line::col(
                    Col::Green,
                    format!("{} You reach out to the {}. It {how}{stop}", a.icon, a.name),
                ),
            );
            self.tell_here(
                x,
                y,
                id,
                Line::col(
                    Col::Green,
                    format!("{} {name} reaches out to pat the {}.", a.icon, a.name),
                ),
            );
            return Ok(());
        }
        let Some(an) = self
            .players
            .get(&id)
            .and_then(|p| p.companion.as_ref())
            .and_then(|(a, n)| animal(a).map(|an| (an, n.clone())))
        else {
            self.say1(
                id,
                Line::plain(
                    "There's nobody here who wants a pat. Befriend an animal with a berry!",
                ),
            );
            return Ok(());
        };
        let (a, n) = an;
        let who = label(a, &n);
        let happy = match a.id {
            "fox" => "leans into your hand and closes its eyes",
            "duckling" => "peeps and snuggles into your palm",
            "rabbit" => "flops over, utterly content",
            "hedgehog" => "uncurls and snuffles your fingers",
            "turtle" => "closes its eyes, which surely means it likes it",
            "barn_owl" => "fluffs up and leans its soft face into your hand",
            "kookaburra" => "chuckles quietly, very pleased with itself",
            "curlew" => "folds down on its long legs and lets out a soft little trill",
            "wombat" => "rolls over for a tummy scratch",
            "seal" => "rolls onto its back and sighs happily",
            "pelican" => "rests its enormous beak on your arm",
            "little_penguin" => "leans against your leg and closes its eyes",
            "badger" => "grunts contentedly and leans in",
            "red_squirrel" => "sits on your palm, tail curled over its back",
            "koala" => "hugs your arm and falls asleep",
            "possum" => "wraps its tail round your wrist",
            "mountain_goat" => "butts your hand for more",
            "salamander" => "sits on your palm, cool and calm",
            "kangaroo" => "leans back on its tail and lets you scratch its chest",
            "echidna" => "uncurls and pokes its long nose into your sleeve",
            _ => "looks very content",
        };
        self.say1(
            id,
            Line::col(
                Col::Green,
                format!("{} You pet {who}. {} {happy}.", a.icon, capitalise(&who)),
            ),
        );
        self.tell_here(
            x,
            y,
            id,
            Line::col(Col::Green, format!("{} {name} pets {who}.", a.icon)),
        );
        // The pat chip can rest a while: freshen the panel now.
        if let Some(p) = self.players.get(&id) {
            self.patted
                .insert(p.user.clone(), std::time::Instant::now());
            p.status_due.set(true);
        }
        Ok(())
    }

    pub(super) fn cmd_let_go(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else {
            return Ok(());
        };
        let user = self.user(id);
        let Some((a, n)) = self.players.get_mut(&id).and_then(|p| p.companion.take()) else {
            self.say1(id, Line::plain("You don't have a companion to let go."));
            return Ok(());
        };
        self.world.db.set_companion(&user, None, None)?;
        let msg = match animal(&a) {
            Some(an) => format!(
                "{} {} nuzzles you goodbye and wanders back to the wild. 💚",
                an.icon,
                capitalise(&label(an, &n))
            ),
            None => "Your companion wanders back to the wild. 💚".to_string(),
        };
        self.say1(id, Line::col(Col::Green, msg));
        self.tell_here(
            x,
            y,
            id,
            Line::col(
                Col::Green,
                format!("{name}'s companion wanders back to the wild."),
            ),
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_companion_has_tricks_for_indoors() {
        for a in ANIMALS {
            assert!(a.indoors.len() >= 3, "{} needs indoor tricks", a.name);
            assert!(
                a.snug.contains("{}"),
                "{} needs somewhere to settle",
                a.name
            );
            for t in a.indoors.iter().chain([&a.snug]) {
                for outdoorsy in [
                    "leaf", "leaves", "clover", "grass", "sand", "water", "seaweed",
                ] {
                    assert!(!t.contains(outdoorsy), "{}: {t}", a.name);
                }
            }
        }
    }

    #[test]
    fn companions_settle_on_soft_things() {
        assert_eq!(
            soft("a feather cushion").as_deref(),
            Some("the feather cushion")
        );
        assert_eq!(soft("a leaf rug").as_deref(), Some("the leaf rug"));
        assert_eq!(
            soft("an enormous pink beanbag").as_deref(),
            Some("the enormous pink beanbag")
        );
        assert_eq!(soft("a driftwood table"), None);
        assert_eq!(soft("a jar"), None);
        assert_eq!(soft("a matchbox"), None);
    }
}

#[cfg(test)]
mod island_tests {
    use super::*;
    use crate::server::db::Db;
    use crate::server::islands::ISLANDS;
    use crate::server::world::World;

    #[test]
    fn every_island_has_creatures_of_its_own_that_can_turn_up_there() {
        let world = World::load_or_create(Db::open(":memory:").unwrap(), Some(7)).unwrap();
        for isle in &ISLANDS {
            let own: Vec<&Animal> = ANIMALS
                .iter()
                .filter(|a| a.island == Some(isle.id))
                .collect();
            assert!(own.len() >= 2, "{} has its own creatures", isle.name);
            for a in own {
                let room = (isle.oy..isle.oy + crate::server::islands::REGION)
                    .flat_map(|y| (isle.ox..isle.ox + 64).map(move |x| (x, y)))
                    .filter(|&(x, y)| a.ground.contains(&world.terrain(x, y)))
                    .count();
                assert!(
                    room > 20,
                    "the {} has ground to live on, on {}",
                    a.name,
                    isle.name
                );
            }
        }
    }

    #[test]
    fn every_creature_has_a_full_life() {
        for a in ANIMALS {
            assert!(!a.tricks.is_empty() && !a.indoors.is_empty() && !a.patted.is_empty());
            assert!(!a.busy.is_empty() && a.snug.contains("{}"), "{}", a.id);
            let same = ANIMALS.iter().filter(|b| b.id == a.id).count();
            assert_eq!(same, 1, "unique id {}", a.id);
        }
    }
}
