//! Island folk: a few scripted characters (not AI, and not players) who
//! live on the islands for a while. Each island has one or two at a time,
//! with a name drawn from a long list and a trade (a beachcomber, a painter,
//! a lamplighter…). They row in, potter about for a few hours or a couple of
//! days, then row off again, and somebody new turns up.
//!
//! They keep island hours (asleep at night), say something now and then to
//! whoever's about (never often, never to someone who's stepped away), chat
//! when they meet each other, and answer when spoken to. They make little
//! things that don't last (a painting drying on an easel, a lantern for the
//! night) and tidy them away again, so the island never fills up with them.
//! Once in a while one of them throws a little celebration, and pins it on
//! the village noticeboard.
//!
//! None of it is in the help: they're there to be bumped into.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::islands::{ISLANDS, Island, REGION};
use crate::server::terrain::Terrain as T;
use crate::server::weather::{Clock, Phase, Season};
use crate::server::world::{DIRS, Trigger, room_of};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const META: &str = "folk";
const NEXT_PARTY: &str = "folk_next_party";
/// The kind of the things they make, so they can be told apart.
pub const KIND: &str = "folk";
const MOST_PER_ISLAND: usize = 2;
/// Their makings, across the whole world at once.
const MOST_MADE: usize = 8;
/// How long someone goes between hearing from the same one of them, unasked.
const QUIET_FOR: Duration = Duration::from_secs(10 * 60);
/// How long they stay, in seconds.
const STAY: (f64, f64) = (3.0 * 3600.0, 48.0 * 3600.0);
/// How long a celebration goes on, in seconds.
const PARTY_LASTS: i64 = 75 * 60;
const HOUR: i64 = 3600;
const DAY: i64 = 24 * HOUR;

pub const NAMES: &[&str] = &[
    "Bramble", "Teasel", "Wren", "Fennel", "Corrie", "Tam", "Marigold", "Hob", "Juniper", "Pim", "Sorrel", "Ottoline",
    "Alder", "Bryony", "Clover", "Dabney", "Elspeth", "Figgis", "Gorse", "Hazel", "Ivo", "Jory", "Kit", "Lark", "Merrow",
    "Nettle", "Oakley", "Perpetua", "Quill", "Rosehip", "Saffy", "Thistle", "Umber", "Vesper", "Wilf", "Yarrow", "Zinnia",
    "Agnes", "Barnaby", "Cosmo", "Delphine", "Ernie", "Florrie", "Gus", "Hettie", "Ignatius", "Jemima", "Kenelm", "Lettice",
    "Mungo", "Nell", "Ozzie", "Pip", "Rufus", "Sid", "Tilly", "Ursula", "Vi", "Winnie", "Xander", "Yolanda", "Zeb",
    "Albie", "Birdie", "Cuthbert", "Dot", "Edmund", "Fern", "Gilbert", "Honor", "Inigo", "Juno", "Kester", "Lupin",
    "Mabel", "Ned", "Olive", "Percy", "Queenie", "Reuben", "Sylvie", "Toby", "Una", "Verity", "Walter", "Ysolde",
    "Arlo", "Bess", "Crispin", "Dulcie", "Ebb", "Fable", "Griff", "Holly", "Isadora", "Jasper", "Kipper", "Linnet",
    "Moss", "Noor", "Orrin", "Primrose", "Rook", "Sable", "Tansy", "Ulric", "Violet", "Wynn", "Yew", "Ziggy",
    "Ambrose", "Blossom", "Conker", "Dandelion", "Evander", "Flint", "Ginger", "Heron", "Iris", "Jubilee", "Kelp",
    "Lavender", "Midge", "Nutmeg", "Oriel", "Pebble", "Rowan", "Skerry", "Tuppence", "Vale", "Whimbrel", "Tamsin",
    "Hollis", "Cassius", "Bunty", "Peregrine", "Morwenna", "Dorrit", "Fitz", "Greer", "Lorcan", "Maud", "Wendell",
];

pub struct Make {
    name: &'static str,
    action: Option<&'static str>,
    trigger: Trigger,
    ground: &'static [T],
    /// How long it stays before they tidy it away, in hours.
    hours: (f64, f64),
    /// "{f} props up an easel…"
    start: &'static str,
    /// "{f} packs the painting away."
    done: &'static str,
}

pub struct Trade {
    pub id: &'static str,
    pub title: &'static str,
    icon: &'static str,
    likes: &'static [T],
    /// "…, with a sack of driftwood over one shoulder."
    with: &'static [&'static str],
    busy: &'static [&'static str],
    says: &'static [&'static str],
    makes: &'static [Make],
    gives: &'static [&'static str],
    notes: &'static [&'static str],
    parties: &'static [&'static str],
    /// How they make music: "whistles", "plays on the fiddle".
    music: Option<&'static str>,
}

const OPEN: &[T] = &[T::Grass, T::Meadow, T::Path];

pub const TRADES: &[Trade] = &[
    Trade {
        id: "beachcomber", title: "beachcomber", icon: "🐚", likes: &[T::Sand],
        with: &["a sack of driftwood over one shoulder", "pockets clinking with sea glass", "a bucket of odd treasures"],
        busy: &["turns over a stone with the toe of a boot", "holds a bit of sea glass up to the light, squinting", "pockets something, then thinks better of it and puts it back", "mutters about the tide tables", "draws a long line in the sand with a stick"],
        says: &["Tide brings something new every morning. Mostly seaweed.", "Found half a teapot yesterday. Still looking for the other half.", "You can hear the sea in a shell. You can hear it out here too, mind.", "Never trust a crab that looks pleased with itself."],
        makes: &[
            Make { name: "a driftwood sculpture of something", action: Some("creaks gently in the wind"), trigger: Trigger::Always, ground: &[T::Sand], hours: (6.0, 24.0), start: "{f} props up some driftwood and lashes it together into a sculpture of… something.", done: "{f} takes the driftwood sculpture apart and stacks the bits neatly." },
            Make { name: "a spiral of shells", action: None, trigger: Trigger::Always, ground: &[T::Sand], hours: (3.0, 12.0), start: "{f} lays out a spiral of shells on the sand, one by one.", done: "{f} gathers up the spiral of shells." },
        ],
        gives: &["cockle", "sea_glass", "scallop", "driftwood", "pebble"],
        notes: &["Found: one left boot, size large, quite damp. It's by the dock. — {f}", "Has anyone seen the other half of a teapot? Blue, with roses. — {f}"],
        parties: &["driftwood bonfire", "rock-pooling afternoon", "low-tide picnic"],
        music: Some("whistles"),
    },
    Trade {
        id: "gardener", title: "gardener", icon: "🌷", likes: &[T::Meadow, T::Grass],
        with: &["a trowel tucked behind one ear", "a basket of seedlings", "muddy knees and a floppy sunhat"],
        busy: &["kneels to pull up a weed, then apologises to it", "deadheads a flower, very gently", "pokes a finger into the soil and nods", "talks quietly to a patch of clover"],
        says: &["Everything grows if you're patient. Except my patience.", "The bees are busy today. Good for them.", "Plant something and it's a promise to come back.", "Don't tell the rabbits, but I've a soft spot for them."],
        makes: &[
            Make { name: "a jam jar of wildflowers", action: None, trigger: Trigger::Always, ground: OPEN, hours: (4.0, 18.0), start: "{f} picks a little bunch of wildflowers and pops them in a jam jar.", done: "{f} takes away the jam jar of flowers, now a bit droopy." },
            Make { name: "a scarecrow in a spare sunhat", action: Some("flaps its sleeves at the crows"), trigger: Trigger::Day, ground: &[T::Meadow, T::Grass], hours: (8.0, 30.0), start: "{f} builds a scarecrow from sticks and string, and gives it a spare sunhat.", done: "{f} takes the scarecrow down, and keeps the hat." },
        ],
        gives: &["daisy", "buttercup", "cornflower", "wild_mint", "thyme"],
        notes: &["Seedlings going spare. Mostly marigolds, possibly weeds. — {f}", "Please don't feed the rabbits my lettuces. They know what they did. — {f}"],
        parties: &["first-flowers party", "garden tea", "sunflower-measuring afternoon"],
        music: Some("hums"),
    },
    Trade {
        id: "painter", title: "painter", icon: "🎨", likes: &[T::Hill, T::Meadow, T::Sand],
        with: &["an easel under one arm", "paint on both elbows", "a satchel bristling with brushes"],
        busy: &["squints at the view and holds up a thumb", "dabs at a canvas, then steps back to frown at it", "wipes a brush on a sleeve", "mixes a colour that doesn't have a name yet"],
        says: &["The light's different every hour. That's the trouble. That's the joy.", "I'm painting the sea. It won't hold still.", "Everything's a bit bluer than you'd think.", "Do you think clouds mind being painted?"],
        makes: &[Make { name: "a painting drying on an easel", action: Some("catches the light just so"), trigger: Trigger::Day, ground: &[T::Hill, T::Meadow, T::Sand, T::Grass], hours: (3.0, 12.0), start: "{f} sets up an easel and paints the view, quick and sure.", done: "{f} packs the painting away, still a little wet." }],
        gives: &[],
        notes: &["Wanted: one patient model. Must be able to sit very still. Sheep welcome. — {f}"],
        parties: &["painting-outdoors afternoon", "very small art show"],
        music: None,
    },
    Trade {
        id: "lamplighter", title: "lamplighter", icon: "🏮", likes: &[T::Path, T::Grass, T::Sand],
        with: &["a long pole with a hook on the end", "a lantern swinging from one hand"],
        busy: &["polishes a lantern on a sleeve", "checks the sky and nods to themself", "counts something on their fingers"],
        says: &["Dark comes on quick here. Best to be ready.", "Every lamp's a little bit of company.", "I like the dusk. Everything goes soft at the edges."],
        makes: &[Make { name: "a lantern on a tall pole", action: Some("glows warm and steady"), trigger: Trigger::Night, ground: &[T::Path, T::Grass, T::Sand, T::Meadow], hours: (6.0, 14.0), start: "{f} plants a tall pole and hangs a lantern from it, ready for dark.", done: "{f} takes down the lantern and its pole." }],
        gives: &[],
        notes: &["A lantern walk soon, I expect. Bring a coat. — {f}"],
        parties: &["lantern walk", "dusk-watching party"],
        music: Some("whistles"),
    },
    Trade {
        id: "fiddler", title: "fiddler", icon: "🎻", likes: OPEN,
        with: &["a fiddle case on their back", "a bow tucked under one arm"],
        busy: &["tunes a fiddle string, frowning", "taps out a rhythm on one knee", "plays three notes, then stops to think"],
        says: &["Every island's got its own tune. This one's in G, I reckon.", "Hum something. Go on. I'll pick it up.", "Music's just walking, really. One step, then another."],
        makes: &[],
        gives: &[],
        notes: &["Anyone know the second half of a tune that goes 'dum de dum'? — {f}"],
        parties: &["sing-song", "dance on the grass", "tune-swapping evening"],
        music: Some("plays on the fiddle"),
    },
    Trade {
        id: "knitter", title: "knitter", icon: "🧶", likes: &[T::Grass, T::Meadow, T::Forest],
        with: &["a bag of wool and two clacking needles", "a very long scarf, still on the needles"],
        busy: &["counts stitches under their breath", "drops a stitch and says a word you won't repeat", "holds up the knitting to see how long it's got"],
        says: &["It's a scarf. Or it will be. Or it's a very long sock.", "Cold coming. I can feel it in my needles.", "Knit one, purl one, have a sit down."],
        makes: &[Make { name: "a tiny knitted hat on a fence post", action: None, trigger: Trigger::Always, ground: OPEN, hours: (6.0, 30.0), start: "{f} knits a tiny hat and leaves it on a post for whoever's head it fits.", done: "{f} collects the tiny knitted hat. Nobody's head fitted." }],
        gives: &[],
        notes: &["Lost: one ball of red wool. Probably a rabbit. — {f}"],
        parties: &["knit-and-natter", "cosy tea afternoon"],
        music: Some("hums"),
    },
    Trade {
        id: "birdwatcher", title: "birdwatcher", icon: "🐦", likes: &[T::Forest, T::Meadow, T::Hill],
        with: &["binoculars round their neck", "a notebook full of tiny drawings"],
        busy: &["peers through binoculars at a branch", "writes something tiny in a notebook", "freezes, finger to lips, then sighs: just a leaf"],
        says: &["Shh. No, it's gone. It was a wren. Probably.", "Saw a heron this morning. It didn't see me. I'm very good.", "Every bird's got an opinion. Listen."],
        makes: &[],
        gives: &["owl_feather", "magpie_feather", "pigeon_feather"],
        notes: &["Seen: one heron, very grumpy. Two wrens, quite cheerful. — {f}"],
        parties: &["dawn chorus walk", "bird-counting picnic"],
        music: Some("whistles"),
    },
    Trade {
        id: "kitemaker", title: "kite-maker", icon: "🪁", likes: &[T::Hill, T::Grass, T::Sand],
        with: &["an armful of kites in every colour", "a reel of string on each hip"],
        busy: &["licks a finger and holds it up to the wind", "untangles a kite string, patiently", "runs a few steps, then gives up: not enough wind"],
        says: &["Wind's from the west. Good for a dragon kite.", "Every kite wants to go somewhere. You let it think it's its own idea."],
        makes: &[Make { name: "a kite tied to a post, bobbing in the sky", action: Some("tugs at its string"), trigger: Trigger::Day, ground: &[T::Hill, T::Grass, T::Sand], hours: (2.0, 8.0), start: "{f} ties a kite to a post and lets it bob up into the sky.", done: "{f} reels in the kite and rolls up the string." }],
        gives: &[],
        notes: &["Kite flying, whenever the wind's right. You'll know. — {f}"],
        parties: &["kite festival", "windy-day picnic"],
        music: None,
    },
    Trade {
        id: "baker", title: "baker", icon: "🥧", likes: OPEN,
        with: &["a basket under a checked cloth", "flour up to the elbows"],
        busy: &["lifts the cloth on the basket to check on something", "sniffs the air: something's ready", "brushes flour off a sleeve, which makes it worse"],
        says: &["There's always room for cake. It's science.", "I've made too many buns again. I always make too many buns.", "Smell that? No, that's me. Cinnamon."],
        makes: &[Make { name: "a tray of buns cooling on a stump", action: Some("smells of cinnamon"), trigger: Trigger::Always, ground: &[T::Grass, T::Meadow, T::Path, T::Forest], hours: (1.0, 4.0), start: "{f} sets a tray of buns on a stump to cool.", done: "{f} gathers up the bun tray, now mysteriously lighter." }],
        gives: &["honey_cake", "berry_crumble"],
        notes: &["Buns going spare most days. Follow your nose. — {f}"],
        parties: &["cake picnic", "bun-tasting afternoon"],
        music: Some("hums"),
    },
    Trade {
        id: "storyteller", title: "storyteller", icon: "📖", likes: &[T::Grass, T::Forest, T::Sand, T::Hill],
        with: &["a battered hat and a long coat with deep pockets", "a walking stick carved all over with little faces"],
        busy: &["stares into the distance, clearly halfway through a story in their head", "laughs at something nobody said", "mutters the end of a story, trying it a different way"],
        says: &["Did I ever tell you about the whale that wanted to be a lighthouse? No? Another time.", "Every rock on this island has a story. Most of them are boring.", "Once upon a time, this was all sea. Then it got bored."],
        makes: &[Make { name: "a little cairn with a story scratched on top", action: None, trigger: Trigger::Always, ground: &[T::Hill, T::Stone, T::Grass], hours: (12.0, 48.0), start: "{f} stacks a few stones into a cairn and scratches a story on the top one, very small.", done: "{f} takes the story cairn apart, stone by stone." }],
        gives: &["pebble"],
        notes: &["Stories told. Ask nicely. — {f}"],
        parties: &["story night", "tall-tales picnic"],
        music: None,
    },
];

/// Old tunes they know, for whistling (the same notation as `hum`).
const TUNES: &[&str] = &[
    "@110 C C G G A A G- F F E E D D C-",
    "@120 E E F G G F E D C C D E E. D/ D-",
    "@160 E E E- E E E- E G C. D/ E---",
    "@120 C D E C C D E C E F G- E F G-",
    "@120 E D C D E E E- D D D- E G G-",
];

const GREET: &[(Phase, &str)] = &[
    (Phase::Dawn, "You're up early, {you}."),
    (Phase::Dawn, "Morning, {you}. Isn't it pink?"),
    (Phase::Day, "Hello, {you}!"),
    (Phase::Day, "Oh! Hello there, {you}."),
    (Phase::Day, "Lovely to see a face, {you}."),
    (Phase::Dusk, "Evening, {you}. The sky's doing its thing."),
    (Phase::Dusk, "Evening! Nearly lamp time."),
];

const WET: &[&str] = &["Lovely weather for ducks.", "Rain's good for the garden, they say. They don't say whose.", "I don't mind a drop of rain. I mind a bucket."];

fn season_says(s: Season) -> &'static [&'static str] {
    match s {
        Season::Spring => &["Smell that? Spring.", "Everything's waking up. Even me, just about."],
        Season::Summer => &["Too warm to hurry. Not that I ever do.", "Long days. Good for wandering."],
        Season::Autumn => &["Leaves everywhere. Crunchy ones are the best.", "Autumn's my favourite. I say that every season."],
        Season::Winter => &["Nippy, isn't it?", "Cold nose, warm heart, that's what my gran said."],
    }
}

const REPLY_HELLO: &[&str] = &["Hello yourself!", "Oh, hello, {you}.", "Well, hello! What a nice surprise.", "Hello, hello."];
const REPLY_ASKED: &[&str] = &["Couldn't tell you, love. Ask the tide.", "Hmm. Now there's a question.", "I'll have a think and get back to you. Might be a while.", "My gran would've known.", "Yes. Or no. One of those."];
const REPLY_OTHER: &[&str] = &["Mm-hm.", "You don't say!", "Is that so.", "Ha! Quite right.", "I'll bear that in mind.", "Funny you should say that."];
const HELLOS: &[&str] = &["hi", "hello", "hey", "hiya", "howdy", "morning", "afternoon", "evening", "greetings", "yo", "ahoy"];

/// Two of them meeting: one says the first, the other the second.
const CHATS: &[(&str, &str)] = &[
    ("Wind's turning.", "Always is."),
    ("Seen the heron today?", "Twice. It pretended not to see me."),
    ("Tea later?", "Always."),
    ("You've got sand on your nose.", "I know. I'm keeping it."),
    ("Did you hear the owl last night?", "I did. Rude, it was."),
    ("Lost my other boot again.", "It'll turn up. They always do."),
    ("Your turn to bring the cake.", "Is it? Drat."),
    ("Busy day?", "Oh, terribly. I've watched three clouds."),
    ("Those rabbits have been at it again.", "Cheeky things. Good for them."),
    ("What's that you're humming?", "No idea. It came with the weather."),
];

const PARTY_DOING: &[&str] = &[
    "{f} hands round slices of seed cake.",
    "{f} strings bunting between two sticks. It sags in the middle. Perfect.",
    "{f} declares this the best party the island has ever had.",
    "{f} raises a cup of tea to everyone here.",
    "{f} does a little dance, entirely without embarrassment.",
];

const GIVING: &[&str] = &["Found two of these. You have one.", "Here. It looked like yours.", "For you. No, go on.", "I was saving this for someone. Turns out it was you."];

#[derive(Serialize, Deserialize, Clone)]
pub struct Party {
    event: i64,
    at: i64,
    x: i64,
    y: i64,
    name: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Folk {
    pub name: String,
    trade: String,
    island: u8,
    pub x: i64,
    pub y: i64,
    leaves: i64,
    /// What they've made and not yet tidied away: (thing id, until).
    #[serde(default)]
    made: Option<(i64, i64)>,
    #[serde(default)]
    party: Option<Party>,
    /// Whether they've pinned a note up this visit.
    #[serde(default)]
    posted: bool,
    /// Who they've given something to, and on which day.
    #[serde(default)]
    gave: HashMap<String, i64>,
    /// Who they've told about their party.
    #[serde(default)]
    invited: Vec<String>,
    #[serde(skip)]
    goal: Option<(i64, i64)>,
    #[serde(skip)]
    next_step: Option<Instant>,
    #[serde(skip)]
    last_doing: Option<Instant>,
    /// When each person (by username) last heard from them.
    #[serde(skip)]
    spoke: HashMap<String, Instant>,
    #[serde(skip)]
    asleep: bool,
}

impl Folk {
    pub fn trade(&self) -> &'static Trade {
        TRADES.iter().find(|t| t.id == self.trade).unwrap_or(&TRADES[0])
    }

    fn island(&self) -> &'static Island {
        &ISLANDS[self.island as usize % ISLANDS.len()]
    }

    fn partying(&self, now: i64) -> Option<&Party> {
        self.party.as_ref().filter(|p| now >= p.at - 15 * 60 && now < p.at + PARTY_LASTS)
    }
}

fn fill(text: &str, f: &str, you: &str) -> String {
    text.replace("{f}", f).replace("{you}", you)
}

impl Game {
    pub(super) fn load_folk(&mut self) {
        self.folk = self
            .world
            .db
            .meta(META)
            .and_then(|j| serde_json::from_str::<Vec<Folk>>(&j).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(|f| TRADES.iter().any(|t| t.id == f.trade))
            .collect();
    }

    fn save_folk(&self) {
        if let Ok(j) = serde_json::to_string(&self.folk) {
            let _ = self.world.db.set_meta(META, &j);
        }
    }

    /// Whether (x, y) is somewhere they'd stand: dry land, out of anyone's
    /// home, out of doors.
    fn folk_fits(&self, x: i64, y: i64) -> bool {
        let t = self.world.terrain(x, y);
        t.walkable() && !t.is_wet() && self.world.home_at(x, y).is_none() && room_of(x, y).is_none()
    }

    /// Something they did, for whoever's at (x, y) and about, and hasn't
    /// muted any of `who`.
    fn folk_tell(&self, x: i64, y: i64, who: &[&str], line: Line) {
        for p in self.players.values() {
            if (p.x, p.y) == (x, y) && p.about() && who.iter().all(|w| p.hears(w)) {
                let _ = p.tx.send(crate::protocol::ToClient::Lines { lines: vec![line.clone()] });
            }
        }
    }

    fn folk_watched(&self, x: i64, y: i64, who: &str) -> bool {
        self.players.values().any(|p| (p.x, p.y) == (x, y) && p.about() && p.hears(who))
    }

    /// A random spot on their island of the kind they like.
    fn folk_spot(&mut self, island: &Island, likes: &[T]) -> Option<(i64, i64)> {
        for _ in 0..60 {
            let x = island.ox + (self.rng.next() % REGION as u64) as i64;
            let y = island.oy + (self.rng.next() % REGION as u64) as i64;
            if likes.contains(&self.world.terrain(x, y)) && self.folk_fits(x, y) {
                return Some((x, y));
            }
        }
        None
    }

    fn folk_dock(&self, island: &Island) -> Option<(i64, i64)> {
        self.world.things.iter().find(|t| t.is_dock() && island.covers(t.x, t.y)).map(|t| (t.x, t.y))
    }

    /// The folk called `name` standing at (x, y), or the only one there.
    fn folk_here(&self, x: i64, y: i64, name: &str) -> Option<usize> {
        let name = crate::server::world::strip_article(name.trim()).to_lowercase();
        let here: Vec<usize> = (0..self.folk.len()).filter(|&i| (self.folk[i].x, self.folk[i].y) == (x, y)).collect();
        if name.is_empty() {
            return (here.len() == 1).then(|| here[0]);
        }
        here.into_iter().find(|&i| {
            let f = &self.folk[i];
            let n = f.name.to_lowercase();
            name == n || name.starts_with(&format!("{n} ")) || name.split_whitespace().next() == Some(n.as_str()) || name.contains(f.trade().title)
        })
    }

    /// Someone on the island by that name, for muting.
    pub(super) fn folk_named(&self, name: &str) -> Option<String> {
        let name = name.trim().to_lowercase();
        self.folk.iter().find(|f| f.name.to_lowercase() == name).map(|f| f.name.clone())
    }

    /// The world turning, for the folk.
    pub(super) fn folk_tick(&mut self) {
        let now = super::boards::now();
        let at = Instant::now();
        let mut changed = self.folk_comings_and_goings(now);
        changed |= self.folk_parties(now);
        let night = Clock::phase() == Phase::Night;
        for i in 0..self.folk.len() {
            changed |= self.folk_sleep(i, night);
            if self.folk[i].asleep {
                continue;
            }
            self.folk_walk(i, now, at);
            self.folk_speak(i, now, at);
            self.folk_busy(i, now, at);
            changed |= self.folk_make(i, now);
            changed |= self.folk_post(i, now);
        }
        changed |= self.folk_tidy(now);
        // Where everyone's got to, now and then, so a restart doesn't scatter them.
        if changed || self.rng.chance(1.0 / 40.0) {
            self.save_folk();
        }
    }

    /// Arrivals and departures, by boat.
    fn folk_comings_and_goings(&mut self, now: i64) -> bool {
        let mut changed = false;
        // Off they go: down to the dock, then away.
        let mut i = 0;
        while i < self.folk.len() {
            let f = &self.folk[i];
            let staying = f.party.as_ref().is_some_and(|p| now < p.at + PARTY_LASTS);
            if now < f.leaves || staying {
                i += 1;
                continue;
            }
            let dock = self.folk_dock(f.island());
            let there = dock.is_none_or(|d| d == (f.x, f.y));
            if !there && now < f.leaves + HOUR {
                self.folk[i].goal = dock;
                i += 1;
                continue;
            }
            let f = self.folk.remove(i);
            self.folk_unmake(&f, false);
            self.folk_tell(f.x, f.y, &[&f.name], Line::col(Col::Cyan, format!("{} waves to nobody in particular, climbs into a little boat and rows off. Off somewhere new.", f.name)));
            changed = true;
        }
        // And someone new rows in, now and then, where there's room.
        for island in &ISLANDS {
            let n = self.folk.iter().filter(|f| f.island == island.id).count();
            let chance = if n == 0 { 1.0 / 40.0 } else { 1.0 / 600.0 };
            if n >= MOST_PER_ISLAND || !self.rng.chance(chance) {
                continue;
            }
            let taken: Vec<String> = self.folk.iter().map(|f| f.name.to_lowercase()).chain(self.world.nicks.values().map(|n| n.to_lowercase())).collect();
            let names: Vec<&str> = NAMES.iter().copied().filter(|n| !taken.contains(&n.to_lowercase())).collect();
            if names.is_empty() {
                continue;
            }
            // Mixed with the clock, so a restart doesn't bring the same faces round again.
            let pick = |g: &mut Self, len: usize| ((g.rng.next() ^ now as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 33) as usize % len;
            let name = names[pick(self, names.len())].to_string();
            let trades: Vec<&Trade> = TRADES.iter().filter(|t| !self.folk.iter().any(|f| f.island == island.id && f.trade == t.id)).collect();
            let trade = trades[pick(self, trades.len())];
            let Some((x, y)) = self.folk_dock(island).or_else(|| self.folk_spot(island, trade.likes)) else { continue };
            let leaves = now + self.rng.range(STAY.0, STAY.1) as i64;
            let with = *self.rng.pick(trade.with);
            self.folk.push(Folk {
                name: name.clone(), trade: trade.id.into(), island: island.id, x, y, leaves, made: None, party: None, posted: false,
                gave: HashMap::new(), invited: vec![], goal: None, next_step: None, last_doing: None, spoke: HashMap::new(), asleep: false,
            });
            self.folk_tell(x, y, &[&name], Line::col(Col::Cyan, format!("{} A little boat bumps against the shore and somebody hops out: {name}, a {}, with {with}.", trade.icon, trade.title)));
            changed = true;
        }
        changed
    }

    /// Island hours: off to sleep at nightfall, up again at dawn.
    fn folk_sleep(&mut self, i: usize, night: bool) -> bool {
        let f = &self.folk[i];
        // Nobody sleeps through their own party.
        if f.asleep == night || f.partying(super::boards::now()).is_some() {
            return false;
        }
        let line = if night {
            format!("{} yawns enormously, wraps up in a blanket and settles down for the night.", f.name)
        } else {
            format!("{} wakes, stretches, and says good morning to the sky.", f.name)
        };
        self.folk_tell(f.x, f.y, &[&f.name], Line::col(Col::Cyan, line));
        self.folk[i].asleep = night;
        true
    }

    fn folk_walk(&mut self, i: usize, now: i64, at: Instant) {
        // Just arrived (or the world's just woken up): settle in first.
        let Some(next) = self.folk[i].next_step else {
            self.folk[i].next_step = Some(at + Duration::from_secs_f64(self.rng.range(60.0, 240.0)));
            return;
        };
        if next > at {
            return;
        }
        let (x, y) = (self.folk[i].x, self.folk[i].y);
        let party = self.folk[i].partying(now).map(|p| (p.x, p.y));
        if party.is_some() {
            self.folk[i].goal = party;
        }
        if self.folk[i].goal.is_none_or(|g| g == (x, y)) {
            if party == Some((x, y)) {
                self.folk[i].next_step = Some(at + Duration::from_secs(30));
                return;
            }
            if self.folk[i].goal.is_some() {
                // Arrived: stay a while.
                self.folk[i].goal = None;
                self.folk[i].next_step = Some(at + Duration::from_secs_f64(self.rng.range(120.0, 420.0)));
                return;
            }
            let f = &self.folk[i];
            let (island, likes) = (f.island(), f.trade().likes);
            let roll = self.rng.f();
            let friend = self.folk.iter().enumerate().find(|(j, o)| *j != i && o.island == f.island).map(|(_, o)| (o.x, o.y));
            let someone = self.players.values().filter(|p| p.about() && island.covers(p.x, p.y) && room_of(p.x, p.y).is_none()).map(|p| (p.x, p.y)).next();
            let goal = match (roll, friend, someone) {
                (r, Some(o), _) if r < 0.15 => Some(o),
                (r, _, Some(p)) if r < 0.30 && self.folk_fits(p.0, p.1) => Some(p),
                _ => self.folk_spot(island, likes),
            };
            self.folk[i].goal = goal;
        }
        let Some(goal) = self.folk[i].goal else { return };
        let dist = |a: (i64, i64)| (a.0 - goal.0).abs().max((a.1 - goal.1).abs());
        let mut steps: Vec<(i64, i64)> = DIRS.iter().map(|d| (d.2, d.3)).filter(|&(dx, dy)| self.folk_fits(x + dx, y + dy) && dist((x + dx, y + dy)) <= dist((x, y))).collect();
        steps.sort_by_key(|&(dx, dy)| dist((x + dx, y + dy)));
        // Mostly straight there, sometimes a little meander.
        let step = if steps.len() > 1 && self.rng.chance(0.25) { steps.get(1).copied() } else { steps.first().copied() };
        let Some((dx, dy)) = step else {
            self.folk[i].goal = None;
            return;
        };
        let (nx, ny) = (x + dx, y + dy);
        self.folk[i].x = nx;
        self.folk[i].y = ny;
        self.folk[i].next_step = Some(at + Duration::from_secs_f64(self.rng.range(15.0, 40.0)));
        let f = &self.folk[i];
        let way = |dx: i64, dy: i64| DIRS.iter().find(|q| q.2 == dx && q.3 == dy).map_or("somewhere", |q| q.1);
        let with = *self.rng.pick(f.trade().with);
        self.folk_tell(x, y, &[&f.name], Line::col(Col::Cyan, format!("{} wanders off to the {}.", f.name, way(dx, dy))));
        self.folk_tell(nx, ny, &[&f.name], Line::col(Col::Cyan, format!("{} wanders in from the {}, with {with}.", f.name, way(-dx, -dy))));
    }

    /// Now and then, a word to whoever's about. Never often.
    fn folk_speak(&mut self, i: usize, now: i64, at: Instant) {
        let (x, y, name) = (self.folk[i].x, self.folk[i].y, self.folk[i].name.clone());
        let listeners: Vec<(u64, String, String, chrono_tz::Tz)> = self
            .players
            .iter()
            .filter(|(_, p)| (p.x, p.y) == (x, y) && p.about() && p.hears(&name))
            .filter(|(_, p)| self.folk[i].spoke.get(&p.user).is_none_or(|t| at.duration_since(*t) > QUIET_FOR))
            .map(|(&id, p)| (id, p.user.clone(), p.name.clone(), p.tz))
            .collect();
        for (pid, user, you, tz) in listeners {
            if !self.rng.chance(1.0 / 12.0) {
                continue;
            }
            let first = !self.folk[i].spoke.contains_key(&user);
            self.folk[i].spoke.insert(user.clone(), at);
            let f = &self.folk[i];
            let trade = f.trade();
            let today = now / DAY;
            // A party coming up: they'll want you there.
            if let Some(p) = f.party.as_ref().filter(|p| p.at > now && !f.invited.contains(&user)) {
                let place = self.world.place_title(p.x, p.y);
                let text = format!("I'm having a {} {}, at {place}. Do come!", p.name.split("'s ").nth(1).unwrap_or(&p.name), super::events::when_label(p.at, tz));
                self.folk[i].invited.push(user.clone());
                self.folk_says(x, y, &name, &text);
                continue;
            }
            // A little something, once a day at most.
            if !trade.gives.is_empty() && f.gave.get(&user) != Some(&today) && self.rng.chance(0.25) {
                let sid = *self.rng.pick(trade.gives);
                let full = self.world.db.satchel(&user).map(|s| s.iter().map(|(_, n)| n).sum::<i64>() >= super::gather::SATCHEL_SIZE).unwrap_or(true);
                if let (Some(s), false) = (crate::server::nature::by_id(sid), full) {
                    let _ = self.world.db.satchel_add(&user, s.id, 1);
                    if self.world.db.journal_add(&user, s.id).unwrap_or(false) {
                        self.say1(pid, Line::new().b(Col::Yellow, "📖 New in your journal: ").b(Col::White, crate::server::world::capitalise(s.name)).c(Col::Yellow, s.rarity.label()));
                    }
                    self.folk[i].gave.insert(user.clone(), today);
                    let words = *self.rng.pick(GIVING);
                    let a = crate::server::nature::a(s.name);
                    self.say1(pid, Line::new().c(Col::Pink, "🎁 ").b(Col::Cyan, &name).c(Col::Pink, format!(" presses {a} into your hand. \"{words}\"")));
                    let _ = self.world.db.discovered(&user, "island folk", now);
                    continue;
                }
            }
            let phase = Clock::phase();
            let greet: Vec<&str> = GREET.iter().filter(|(p, _)| *p == phase).map(|(_, g)| *g).collect();
            let wet = self.world.weather.sky_at(x, y).wet();
            let text = if first && !greet.is_empty() {
                *self.rng.pick(&greet)
            } else if wet && self.rng.chance(0.5) {
                *self.rng.pick(WET)
            } else if self.rng.chance(0.3) {
                *self.rng.pick(season_says(self.season))
            } else {
                *self.rng.pick(trade.says)
            };
            self.folk_says(x, y, &name, &fill(text, &name, &you));
            if !user.contains('@') {
                let _ = self.world.db.discovered(&user, "island folk", now);
            }
        }
    }

    fn folk_says(&self, x: i64, y: i64, name: &str, text: &str) {
        let line = Line::new().b(Col::Cyan, name).c(Col::Dim, " says, ").c(Col::White, format!("\"{text}\""));
        for p in self.players.values() {
            if (p.x, p.y) == (x, y) && !p.bot && p.hears(name) {
                let _ = p.tx.send(crate::protocol::ToClient::Lines { lines: vec![line.clone()] });
            }
        }
    }

    /// With someone watching: their own business, a chat with a friend, a
    /// word with an animal, a tune.
    fn folk_busy(&mut self, i: usize, now: i64, at: Instant) {
        let (x, y, name) = (self.folk[i].x, self.folk[i].y, self.folk[i].name.clone());
        let partying = self.folk[i].partying(now).is_some_and(|p| (p.x, p.y) == (x, y));
        let (wait, chance) = if partying { (35, 1.0 / 6.0) } else { (60, 1.0 / 12.0) };
        if self.folk[i].last_doing.is_some_and(|t| at.duration_since(t) < Duration::from_secs(wait)) || !self.folk_watched(x, y, &name) || !self.rng.chance(chance) {
            return;
        }
        self.folk[i].last_doing = Some(at);
        let trade = self.folk[i].trade();
        let friend = self.folk.iter().enumerate().find(|(j, o)| *j != i && (o.x, o.y) == (x, y) && !o.asleep).map(|(j, o)| (j, o.name.clone()));
        let animal = self.wild.iter().find(|w| (w.x, w.y) == (x, y)).map(|w| w.animal.name);
        let pet = self
            .players
            .values()
            .filter(|p| (p.x, p.y) == (x, y))
            .find_map(|p| p.companion.as_ref().and_then(|(a, n)| super::companions::animal(a).map(|an| super::companions::label(an, n))));
        let thing = self.world.things_at(x, y).filter(|t| t.kind.as_deref() != Some(KIND)).map(|t| super::the(&t.name)).next();
        let roll = self.rng.f();
        if partying && roll < 0.6 {
            let doing = *self.rng.pick(PARTY_DOING);
            let line = fill(doing, &name, "");
            self.folk_tell(x, y, &[&name], Line::col(Col::Yellow, format!("🎉 {line}")));
            return;
        }
        if let Some((j, other)) = friend.filter(|_| roll < 0.35) {
            let (a, b) = *self.rng.pick(CHATS);
            self.folk_says_to(x, y, &name, &other, a);
            self.folk_says_to(x, y, &other, &name, b);
            self.folk[j].last_doing = Some(at);
            return;
        }
        let line = match (animal, pet, thing, trade.music) {
            (Some(a), _, _, _) if roll < 0.5 => {
                let how = *self.rng.pick(&["crouches down and holds out a hand to the {a}", "tells the {a} a long story. The {a} listens politely", "and the {a} regard each other for a while"]);
                format!("{name} {}.", how.replace("{a}", a))
            }
            (_, Some(p), _, _) if roll < 0.55 => format!("{name} {}.", self.rng.pick(&["scratches {p} behind the ears", "slips {p} a crumb from a pocket", "says hello to {p}, very properly"]).replace("{p}", &p)),
            (_, _, Some(t), _) if roll < 0.65 => format!("{name} {}.", self.rng.pick(&["admires {t}", "gives {t} an approving nod", "peers closely at {t}"]).replace("{t}", &t)),
            (_, _, _, Some(how)) if roll < 0.8 => {
                let tune = *self.rng.pick(TUNES);
                let line = Line::new().b(Col::Cyan, &name).c(Col::Dim, format!(" {how} ")).c(Col::Pink, format!("♪ {tune} ♪"));
                self.folk_tell(x, y, &[&name], line);
                return;
            }
            _ => format!("{name} {}.", self.rng.pick(trade.busy)),
        };
        self.folk_tell(x, y, &[&name], Line::col(Col::Cyan, line));
    }

    fn folk_says_to(&self, x: i64, y: i64, name: &str, other: &str, text: &str) {
        let line = Line::new().b(Col::Cyan, name).c(Col::Dim, format!(" says to {other}, ")).c(Col::White, format!("\"{text}\""));
        self.folk_tell(x, y, &[name, other], line);
    }

    /// Something small that won't last, now and then, while they're settled.
    fn folk_make(&mut self, i: usize, now: i64) -> bool {
        let f = &self.folk[i];
        if f.made.is_some() || f.goal.is_some() || !self.rng.chance(1.0 / 300.0) {
            return false;
        }
        let (x, y, name) = (f.x, f.y, f.name.clone());
        let trade = f.trade();
        let ground = self.world.terrain(x, y);
        // A sandcastle, for the tide to take.
        if trade.id == "beachcomber" && ground == T::Sand && self.rng.chance(0.4) {
            if let Some(bit) = self.sandcastle_by(x, y, &name) {
                self.folk_tell(x, y, &[&name], Line::col(Col::Yellow, format!("🏰 {name} kneels down and adds {bit} to a sandcastle.")));
            }
            return false;
        }
        let mine = self.world.things.iter().filter(|t| t.kind.as_deref() == Some(KIND)).count();
        let crowded = self.world.things_at(x, y).count() >= 10;
        let near = self.world.things.iter().any(|t| t.kind.as_deref() == Some(KIND) && (t.x - x).abs().max((t.y - y).abs()) <= 4);
        let options: Vec<&Make> = trade.makes.iter().filter(|m| m.ground.contains(&ground)).collect();
        if mine >= MOST_MADE || crowded || near || options.is_empty() || !self.folk_fits(x, y) {
            return false;
        }
        let m = *self.rng.pick(&options);
        let Ok(tid) = self.world.add_thing(x, y, m.name.into(), m.action.map(Into::into), m.trigger, &name, Some(KIND)) else { return false };
        let until = now + (self.rng.range(m.hours.0, m.hours.1) * 3600.0) as i64;
        self.folk[i].made = Some((tid, until));
        self.folk_tell(x, y, &[&name], Line::col(Col::Cyan, fill(m.start, &name, "")));
        self.status_all();
        true
    }

    /// Their makings go when their time's up (or when they do).
    fn folk_tidy(&mut self, now: i64) -> bool {
        let due: Vec<usize> = (0..self.folk.len()).filter(|&i| self.folk[i].made.is_some_and(|(_, until)| until <= now)).collect();
        for &i in &due {
            let f = self.folk[i].clone();
            self.folk_unmake(&f, true);
            self.folk[i].made = None;
        }
        !due.is_empty()
    }

    fn folk_unmake(&mut self, f: &Folk, say: bool) {
        let Some((tid, _)) = f.made else { return };
        let Some(i) = self.world.things.iter().position(|t| t.id == tid) else { return };
        let (tx, ty, tname) = (self.world.things[i].x, self.world.things[i].y, self.world.things[i].name.clone());
        if self.world.remove_thing(i, &f.name).is_err() {
            return;
        }
        let done = f.trade().makes.iter().find(|m| m.name == tname).map(|m| m.done);
        if let (true, true, Some(done)) = (say, (f.x, f.y) == (tx, ty), done) {
            self.folk_tell(tx, ty, &[&f.name], Line::col(Col::Cyan, fill(done, &f.name, "")));
        }
        self.status_all();
    }

    /// Once a visit at most, a note on the village noticeboard.
    fn folk_post(&mut self, i: usize, now: i64) -> bool {
        let f = &self.folk[i];
        if f.posted || f.trade().notes.is_empty() || !self.rng.chance(1.0 / 2500.0) {
            return false;
        }
        let Some(board) = self.village_board() else { return false };
        let note = *self.rng.pick(f.trade().notes);
        let text = fill(note, &f.name, "");
        let name = f.name.clone();
        let _ = self.world.db.add_note(board, &name, &text, now, &super::boards::week_key(now), None);
        self.folk[i].posted = true;
        true
    }

    fn village_board(&self) -> Option<i64> {
        self.world.things.iter().find(|t| t.is_board() && t.name == "the village noticeboard").map(|t| t.id)
    }

    /// Every few days, one of them throws a little celebration: an event at
    /// a spot on their island, pinned on the village noticeboard.
    fn folk_parties(&mut self, now: i64) -> bool {
        // Over and done with.
        let mut changed = false;
        for f in self.folk.iter_mut() {
            if f.party.as_ref().is_some_and(|p| now >= p.at + PARTY_LASTS) {
                f.party = None;
                f.invited.clear();
                changed = true;
            }
        }
        let next = self.world.db.meta(NEXT_PARTY).and_then(|n| n.parse::<i64>().ok());
        let Some(next) = next else {
            let _ = self.world.db.set_meta(NEXT_PARTY, &(now + self.rng.range(1.0, 3.0) as i64 * DAY).to_string());
            return changed;
        };
        if now < next || self.folk.is_empty() || self.folk.iter().any(|f| f.party.is_some()) {
            return changed;
        }
        let i = (self.rng.next() % self.folk.len() as u64) as usize;
        let (island, trade, name) = (self.folk[i].island(), self.folk[i].trade(), self.folk[i].name.clone());
        let Some((x, y)) = self.folk_spot(island, trade.likes) else { return changed };
        // A friendly hour, island-wide: between 11 and 7, in the world's own time.
        let tz = super::world_zone();
        let mut at = (now + self.rng.range(4.0, 30.0) as i64 * HOUR) / HOUR * HOUR;
        for _ in 0..24 {
            let h = chrono::TimeZone::timestamp_opt(&tz, at, 0).single().map(|t| chrono::Timelike::hour(&t)).unwrap_or(12);
            if (11..=19).contains(&h) {
                break;
            }
            at += HOUR;
        }
        let what = *self.rng.pick(trade.parties);
        let title = format!("{name}'s {what}");
        let Ok(event) = self.world.db.add_event(at, &title, x, y, &name, None, true) else { return changed };
        if let Some(board) = self.village_board() {
            let place = self.world.place_title(x, y);
            let note = format!("📅 {title} · {} at {place}. All welcome! — {name}", super::events::when_zoned(at, tz));
            let _ = self.world.db.add_note(board, &name, &note, now, &super::boards::week_key(now), None);
        }
        let f = &mut self.folk[i];
        f.leaves = f.leaves.max(at + PARTY_LASTS + HOUR);
        f.party = Some(Party { event, at, x, y, name: title });
        let _ = self.world.db.set_meta(NEXT_PARTY, &(at + self.rng.range(2.0, 5.0) as i64 * DAY).to_string());
        println!("🎉 {name} is throwing a party");
        true
    }

    /// For `look`: whoever's here.
    pub(super) fn folk_lines(&self, id: u64) -> Vec<Line> {
        let Some(me) = self.players.get(&id) else { return vec![] };
        let now = super::boards::now();
        self.folk
            .iter()
            .filter(|f| (f.x, f.y) == (me.x, me.y) && me.hears(&f.name))
            .map(|f| {
                let t = f.trade();
                let doing = if f.asleep {
                    "curled up asleep under a blanket, snoring softly".to_string()
                } else if f.partying(now).is_some_and(|p| (p.x, p.y) == (f.x, f.y)) {
                    "here, and it's a party!".to_string()
                } else {
                    format!("the {}, is here", t.title)
                };
                let text = if f.asleep { format!("{} {} is {doing}.", t.icon, f.name) } else if doing.starts_with("here") { format!("{} {} is {doing}", t.icon, f.name) } else { format!("{} {}, {doing}.", t.icon, f.name) };
                Line::col(Col::Cyan, text)
            })
            .collect()
    }

    /// Folk-shaped commands: talking to them, looking at them, giving them
    /// things.
    pub(super) fn folk_command(&mut self, id: u64, verb: &str, rest: &str) -> Option<anyhow::Result<()>> {
        let (x, y) = self.players.get(&id).map(|p| (p.x, p.y))?;
        let r = rest.trim().to_lowercase();
        match verb {
            "talk" | "greet" | "speak" => {
                let who = r.trim_start_matches("to ").trim_start_matches("with ");
                let i = self.folk_here(x, y, who)?;
                Some(self.folk_reply(id, i, "hello"))
            }
            "look" | "l" | "examine" | "x" if !r.is_empty() => {
                let i = self.folk_here(x, y, r.trim_start_matches("at "))?;
                Some(self.folk_look(id, i))
            }
            "give" | "offer" => {
                let (what, to) = rest.rsplit_once(" to ")?;
                let i = self.folk_here(x, y, to)?;
                Some(self.folk_given(id, i, what))
            }
            _ => None,
        }
    }

    fn folk_look(&mut self, id: u64, i: usize) -> anyhow::Result<()> {
        let f = &self.folk[i];
        let t = f.trade();
        let with = *self.rng.pick(t.with);
        let mut text = format!("{} {}, the island's {} for now, with {with}.", t.icon, f.name, t.title);
        if f.asleep {
            text.push_str(" Fast asleep.");
        }
        if let Some(p) = f.party.as_ref().filter(|p| p.at > super::boards::now()) {
            let tz = self.players.get(&id).map_or(chrono_tz::UTC, |p| p.tz);
            text.push_str(&format!(" Planning {} {}.", p.name.split("'s ").nth(1).map_or(p.name.clone(), |n| format!("a {n}")), super::events::when_label(p.at, tz)));
        }
        self.say1(id, Line::col(Col::Cyan, text));
        Ok(())
    }

    fn folk_given(&mut self, id: u64, i: usize, what: &str) -> anyhow::Result<()> {
        let user = self.user(id);
        let Some(s) = self.carried(&user, what)? else {
            self.say1(id, Line::plain(format!("You don't have any {} in your satchel.", what.trim())));
            return Ok(());
        };
        let Some((x, y, me)) = self.here(id) else { return Ok(()) };
        let name = self.folk[i].name.clone();
        let thing = crate::server::nature::a(s.name);
        if self.folk[i].asleep {
            self.say1(id, Line::plain(format!("{name} is fast asleep. Maybe in the morning?")));
            return Ok(());
        }
        if s.kind.alive() {
            self.say1(id, Line::col(Col::Pink, format!("🎁 {name} admires {thing}, then lets it go. \"Better off free, that one.\"")));
        } else {
            let thanks = *self.rng.pick(&["Ooh. I know just where this goes.", "For me? You shouldn't have. No, you should.", "Thank you! I'll think of you whenever I see it.", "Well, isn't that lovely."]);
            self.say1(id, Line::new().c(Col::Pink, "🎁 ").b(Col::Cyan, &name).c(Col::Pink, format!(" takes {thing} and tucks it away. \"{thanks}\"")));
        }
        self.world.db.satchel_add(&user, s.id, -1)?;
        self.tell_here(x, y, id, Line::col(Col::Pink, format!("🎁 {me} gives {name} {thing}.")));
        Ok(())
    }

    /// Someone said something at a spot where folk are: they might answer.
    pub(super) fn folk_hear(&mut self, id: u64, msg: &str) {
        let Some((x, y)) = self.players.get(&id).map(|p| (p.x, p.y)) else { return };
        let lower = msg.to_lowercase();
        let words: Vec<&str> = lower.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect();
        let alone = self.players.values().filter(|p| (p.x, p.y) == (x, y) && !p.bot).count() == 1;
        let here: Vec<usize> = (0..self.folk.len()).filter(|&i| (self.folk[i].x, self.folk[i].y) == (x, y)).collect();
        for i in here {
            let named = words.contains(&self.folk[i].name.to_lowercase().as_str());
            let hello = words.iter().any(|w| HELLOS.contains(w));
            // Asleep, only their own name stirs them.
            let awake = !self.folk[i].asleep;
            if named || (awake && (hello || (alone && self.rng.chance(0.5)))) {
                let _ = self.folk_reply(id, i, &lower);
                // One answer's enough.
                break;
            }
        }
    }

    fn folk_reply(&mut self, id: u64, i: usize, said: &str) -> anyhow::Result<()> {
        let Some(p) = self.players.get(&id) else { return Ok(()) };
        let (you, user) = (p.name.clone(), p.user.clone());
        let (x, y, name) = (self.folk[i].x, self.folk[i].y, self.folk[i].name.clone());
        if self.folk[i].asleep {
            self.say1(id, Line::col(Col::Cyan, format!("{name} mumbles something about seaweed and goes back to snoring.")));
            return Ok(());
        }
        let words: Vec<&str> = said.split(|c: char| !c.is_alphanumeric()).collect();
        let pool = if words.iter().any(|w| HELLOS.contains(w)) {
            REPLY_HELLO
        } else if said.contains('?') {
            REPLY_ASKED
        } else {
            REPLY_OTHER
        };
        let reply = *self.rng.pick(pool);
        let text = fill(reply, &name, &you);
        self.folk[i].spoke.insert(user.clone(), Instant::now());
        self.folk_says(x, y, &name, &text);
        if !user.contains('@') {
            let _ = self.world.db.discovered(&user, "island folk", super::boards::now());
        }
        Ok(())
    }

    /// A hint for someone near (but not with) one of the folk.
    pub(super) fn folk_nearby(&self, x: i64, y: i64) -> bool {
        self.folk.iter().any(|f| !f.asleep && (f.x, f.y) != (x, y) && (f.x - x).abs().max((f.y - y).abs()) <= 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plenty_of_names_and_no_repeats() {
        assert!(NAMES.len() >= 120);
        let mut seen = std::collections::HashSet::new();
        for n in NAMES {
            assert!(seen.insert(n.to_lowercase()), "{n} is listed twice");
        }
    }

    #[test]
    fn every_trade_has_something_to_say_and_do() {
        for t in TRADES {
            assert!(!t.with.is_empty() && !t.busy.is_empty() && !t.says.is_empty() && !t.parties.is_empty(), "{}", t.id);
            for g in t.gives {
                assert!(crate::server::nature::by_id(g).is_some(), "{} gives {g}", t.id);
            }
            for m in t.makes {
                assert!(m.start.contains("{f}") && m.done.contains("{f}"), "{}", m.name);
            }
        }
    }

    #[test]
    fn their_tunes_are_tunes() {
        for t in TUNES {
            assert_eq!(super::super::music::clean_tune(t), *t);
        }
    }
}
