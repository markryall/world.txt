//! The island since you were last here: arriving after a while away, a few
//! lines about what other people made, named, planted and posted, so a quiet
//! island still shows signs of them. Only what anyone walking past could
//! have seen (and who dropped by your own home); nothing from indoors, no
//! messages, nothing of your own and nothing from anyone you've muted.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::db::HappeningRow;
use crate::server::gardens::{self, Kind as PlantKind};
use crate::server::islands::island_at;
use crate::server::nature::a;
use crate::server::world::{in_room_space, join_and};

/// Away less than this, and there's no recap.
pub const AWAY: i64 = 3600;
/// At most this many lines, then a hint there's more.
const MOST: usize = 6;

pub const HOME: &str = "home";
pub const PAINT: &str = "paint";
pub const BUILT: &str = "built";
pub const NAMED: &str = "named";
pub const MADE: &str = "made";
pub const VISIT: &str = "visit";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    Home,
    Paint,
    Built,
    Named,
    Made,
    Planted,
    Grew,
    Notes,
    Visit,
    /// Something in pieces that came together; `what` is already worded.
    Whole,
}

impl Kind {
    fn from(s: &str) -> Option<Kind> {
        Some(match s {
            HOME => Kind::Home,
            PAINT => Kind::Paint,
            BUILT => Kind::Built,
            NAMED => Kind::Named,
            MADE => Kind::Made,
            VISIT => Kind::Visit,
            super::quests::WHOLE => Kind::Whole,
            _ => return None,
        })
    }

    /// People's doings first, then the garden, the boards, and who called.
    fn section(self) -> u8 {
        match self {
            Kind::Planted | Kind::Grew => 1,
            Kind::Notes => 2,
            Kind::Visit => 3,
            _ => 0,
        }
    }
}

/// One thing to mention, before grouping.
#[derive(Debug, Clone)]
struct Item {
    at: i64,
    /// Their nickname (or the board's title, for notes).
    who: String,
    kind: Kind,
    /// The thing, name, colour or plan; for notes, where the board is.
    what: String,
    place: String,
    island: &'static str,
    /// What tapping the line does.
    tap: Option<String>,
    /// For notes: how many.
    n: i64,
    icon: &'static str,
}

/// Group what happened by person and kind, and word it.
fn summarise(mut items: Vec<Item>) -> Vec<(String, Option<String>)> {
    items.sort_by_key(|i| (i.kind.section(), -i.at));
    let mut groups: Vec<Vec<Item>> = Vec::new();
    for i in items {
        match groups
            .iter_mut()
            .find(|g| g[0].who == i.who && g[0].kind == i.kind)
        {
            Some(g) => g.push(i),
            None => groups.push(vec![i]),
        }
    }
    // A new home and a lick of paint are one piece of news.
    let paints: Vec<(String, String)> = groups
        .iter()
        .filter(|g| g[0].kind == Kind::Paint)
        .map(|g| (g[0].who.clone(), g[0].what.clone()))
        .collect();
    let homes: Vec<String> = groups
        .iter()
        .filter(|g| g[0].kind == Kind::Home)
        .map(|g| g[0].who.clone())
        .collect();
    groups.retain(|g| !(g[0].kind == Kind::Paint && homes.contains(&g[0].who)));

    let mut out: Vec<(String, Option<String>)> = groups
        .iter()
        .map(|g| {
            let (i, n) = (&g[0], g.len());
            let who = &i.who;
            let text = match i.kind {
                Kind::Home => {
                    let paint = paints
                        .iter()
                        .find(|(w, _)| w == who)
                        .map_or(String::new(), |(_, c)| format!(", and painted it {c}"));
                    format!("🏡 {who} made a home on {}{paint}.", i.island)
                }
                Kind::Paint => format!("🎨 {who} painted their home {}.", i.what),
                Kind::Built if n == 1 => {
                    format!("🔨 {who} finished {} at {}.", a(&i.what), i.place)
                }
                Kind::Built => {
                    let what: Vec<String> = g.iter().map(|i| a(&i.what)).collect();
                    format!("🔨 {who} finished {n} buildings: {}.", few(&what))
                }
                Kind::Named if n == 1 => format!("🪧 {who} named a spot {}.", i.what),
                Kind::Named => {
                    let names: Vec<String> = g.iter().map(|i| i.what.clone()).collect();
                    format!("🪧 {who} named some spots: {}.", few(&names))
                }
                Kind::Made if n == 1 => format!("✨ {who} made {} at {}.", i.what, i.place),
                Kind::Made => format!("✨ {who} made {n} things on {}.", i.island),
                Kind::Planted if n == 1 => {
                    format!("🌱 {who} planted {} at {}.", i.what, i.place)
                }
                Kind::Planted => format!("🌱 {who} planted {n} things on {}.", i.island),
                Kind::Grew if n == 1 => format!("{} {who}'s {}.", i.icon, i.what),
                Kind::Grew => format!("🌳 {n} of {who}'s plantings have grown."),
                Kind::Notes => {
                    let s = if i.n == 1 { "" } else { "s" };
                    format!("{} {} new note{s} {}.", i.icon, i.n, i.what)
                }
                Kind::Visit => format!("👣 {who} dropped by your home."),
                Kind::Whole => i.what.clone(),
            };
            (text, i.tap.clone())
        })
        .collect();
    if out.len() > MOST {
        out.truncate(MOST);
        out.push(("…and more besides, out there to find.".into(), None));
    }
    out
}

/// "A, B and C", or "A, B and 2 more".
fn few(items: &[String]) -> String {
    if items.len() <= 3 {
        return join_and(items);
    }
    let mut shown = items[..2].to_vec();
    shown.push(format!("{} more", items.len() - 2));
    join_and(&shown)
}

impl Game {
    /// Note something done out in the open, for the recap. Indoors stays indoors.
    pub(super) fn happened(&self, who: &str, kind: &str, x: i64, y: i64, what: &str) {
        if in_room_space(x, y) || what.trim().is_empty() {
            return;
        }
        let row = HappeningRow {
            at: super::boards::now(),
            who: who.to_string(),
            kind: kind.to_string(),
            x,
            y,
            what: what.to_string(),
        };
        if let Err(e) = self.world.db.add_happening(&row) {
            eprintln!("noting a happening: {e}");
        }
    }

    /// After a command: anything new it made out in the open.
    pub(super) fn note_made(&self, id: u64, newest_before: i64) {
        let Some(user) = self.players.get(&id).map(|p| p.user.clone()) else {
            return;
        };
        for t in self.world.things.iter().filter(|t| {
            t.id > newest_before
                && t.maker == user
                // Scaffolding is news once it's finished.
                && t.kind.as_deref() != Some(super::builds::KIND)
        }) {
            self.happened(&user, MADE, t.x, t.y, &t.name);
        }
    }

    pub(super) fn newest_thing(&self) -> i64 {
        self.world.things.iter().map(|t| t.id).max().unwrap_or(0)
    }

    /// On arriving after a while away: what other people got up to.
    pub(super) fn since_news(&mut self, id: u64, since: i64) {
        let Some(p) = self.players.get(&id) else {
            return;
        };
        let me = p.user.clone();
        let now = super::boards::now();
        let places = self.world.places();
        let tap = |x: i64, y: i64| {
            places
                .iter()
                .find(|p| (p.0, p.1) == (x, y))
                .map(|p| format!("go to {}", p.2))
        };
        let mut items = Vec::new();
        let rows = self.world.db.happenings_since(since).unwrap_or_default();
        for r in rows {
            let Some(kind) = Kind::from(&r.kind) else {
                continue;
            };
            if r.who == me || !p.hears(&r.who) || (kind == Kind::Visit && r.what != me) {
                continue;
            }
            // A home goes by where it is now.
            let at_home = |who: &str| {
                self.world
                    .homes
                    .iter()
                    .find(|h| h.owner == who)
                    .map(|h| (h.x, h.y))
            };
            let (x, y) = match kind {
                Kind::Home | Kind::Paint => at_home(&r.who).unwrap_or((r.x, r.y)),
                _ => (r.x, r.y),
            };
            let what = if kind == Kind::Whole {
                let place = self.world.place_title(x, y);
                let Some(words) = self.whole_words(&me, &r.what, &place) else {
                    continue;
                };
                words
            } else {
                r.what
            };
            items.push(Item {
                at: r.at,
                who: self.world.nick(&r.who),
                kind,
                what,
                place: self.world.place_title(x, y),
                island: island_at(x, y).name,
                tap: tap(x, y),
                n: 1,
                icon: "",
            });
        }
        // Other people's gardens: planted since, or grown a stage since.
        for r in self.world.plantings.values() {
            if r.by == me || !p.hears(&r.by) {
                continue;
            }
            let Some((plant, stage)) = gardens::stage_of(r, now) else {
                continue;
            };
            let place = self.world.place_title(r.x, r.y);
            let (kind, what) = if r.at > since {
                let what = match plant.kind {
                    PlantKind::Tree => a(plant.name),
                    PlantKind::Flowers => plant.name.to_string(),
                };
                (Kind::Planted, what)
            } else if stage > gardens::stage(plant.kind, since - r.at) {
                let what = format!("{} at {place} {}", plant.name, gardens::grew(plant, stage));
                (Kind::Grew, what)
            } else {
                continue;
            };
            items.push(Item {
                at: r.at,
                who: self.world.nick(&r.by),
                kind,
                what,
                place,
                island: island_at(r.x, r.y).name,
                tap: tap(r.x, r.y),
                n: 1,
                icon: match plant.kind {
                    PlantKind::Tree => "🌳",
                    PlantKind::Flowers => "🌼",
                },
            });
        }
        // Boards they follow.
        for (board, seen) in self.world.db.followed(&me).unwrap_or_default() {
            let n = self.world.db.notes_since(board, since, seen, &me);
            let Some(t) = self.world.things.iter().find(|t| t.id == board) else {
                continue;
            };
            if n == 0 {
                continue;
            }
            let journal = t.kind.as_deref() == Some(super::boards::JOURNAL_KIND);
            let (icon, what) = if journal {
                ("📖", "in the Island Journal".to_string())
            } else {
                (
                    "📌",
                    format!("on the noticeboard at {}", self.world.place_title(t.x, t.y)),
                )
            };
            items.push(Item {
                at: now,
                who: format!("board {board}"),
                kind: Kind::Notes,
                what,
                place: String::new(),
                island: "",
                tap: tap(t.x, t.y),
                n,
                icon,
            });
        }
        let lines = summarise(items);
        if lines.is_empty() {
            return;
        }
        let mut out = vec![Line::new().b(Col::Cyan, "🏝 Since you were last here:")];
        for (text, tap) in lines {
            out.push(match tap {
                Some(cmd) => Line::new().t("  ").run(Col::Cyan, text, cmd),
                None => Line::col(Col::Cyan, format!("  {text}")),
            });
        }
        self.send(id, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(at: i64, who: &str, kind: Kind, what: &str) -> Item {
        Item {
            at,
            who: who.into(),
            kind,
            what: what.into(),
            place: "The Duck Pond".into(),
            island: "Mossy Holm",
            tap: Some("go to The Duck Pond".into()),
            n: 1,
            icon: "🌳",
        }
    }

    fn texts(items: Vec<Item>) -> Vec<String> {
        summarise(items).into_iter().map(|l| l.0).collect()
    }

    #[test]
    fn nothing_happened_says_nothing() {
        assert!(summarise(vec![]).is_empty());
    }

    #[test]
    fn a_new_home_and_its_paint_are_one_line() {
        let t = texts(vec![
            item(10, "Poppy", Kind::Home, ""),
            item(20, "Poppy", Kind::Paint, "lilac"),
            item(30, "Sam", Kind::Paint, "sky blue"),
        ]);
        assert_eq!(
            t,
            vec![
                "🎨 Sam painted their home sky blue.",
                "🏡 Poppy made a home on Mossy Holm, and painted it lilac.",
            ]
        );
    }

    #[test]
    fn lots_of_making_is_grouped_not_itemised() {
        let t = texts(vec![
            item(1, "Sam", Kind::Made, "a bench"),
            item(2, "Sam", Kind::Made, "a frog"),
            item(3, "Sam", Kind::Made, "a lantern"),
            item(4, "Lu", Kind::Made, "a kite"),
        ]);
        assert_eq!(
            t,
            vec![
                "✨ Lu made a kite at The Duck Pond.",
                "✨ Sam made 3 things on Mossy Holm.",
            ]
        );
        let t = texts(vec![
            item(1, "Poppy", Kind::Named, "The Old Oak"),
            item(2, "Poppy", Kind::Named, "The Willow Bank"),
            item(3, "Poppy", Kind::Named, "Frog Hollow"),
            item(4, "Poppy", Kind::Named, "Sunny Steps"),
        ]);
        assert_eq!(
            t,
            vec!["🪧 Poppy named some spots: Sunny Steps, Frog Hollow, and 2 more."]
        );
    }

    #[test]
    fn people_first_then_gardens_boards_and_visits() {
        let mut notes = item(
            99,
            "board 7",
            Kind::Notes,
            "on the noticeboard at The Village Green",
        );
        notes.n = 3;
        notes.icon = "📌";
        let t = texts(vec![
            item(50, "Poppy", Kind::Visit, "me"),
            notes,
            item(
                5,
                "Lu",
                Kind::Grew,
                "birch at The Duck Pond has grown into a tall birch",
            ),
            item(9, "Sam", Kind::Built, "cottage"),
        ]);
        assert_eq!(
            t,
            vec![
                "🔨 Sam finished a cottage at The Duck Pond.",
                "🌳 Lu's birch at The Duck Pond has grown into a tall birch.",
                "📌 3 new notes on the noticeboard at The Village Green.",
                "👣 Poppy dropped by your home.",
            ]
        );
    }

    #[test]
    fn a_busy_spell_is_kept_short() {
        let names = ["Ana", "Bo", "Cy", "Di", "Ed", "Flo", "Gus", "Hal"];
        let items = names
            .iter()
            .enumerate()
            .map(|(i, n)| item(i as i64, n, Kind::Made, "a bench"))
            .collect();
        let lines = summarise(items);
        assert_eq!(lines.len(), MOST + 1);
        assert_eq!(
            lines[0].0, "✨ Hal made a bench at The Duck Pond.",
            "newest first"
        );
        assert_eq!(
            lines[MOST],
            ("…and more besides, out there to find.".into(), None)
        );
    }
}
