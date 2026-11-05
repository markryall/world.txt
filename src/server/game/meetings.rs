//! Meeting spots: somewhere to gather, with links for whatever's going on
//! (a call, a list, a song). A meeting table (or a gazebo) holds them; 'meet' hands it to you and
//! tells everyone there you've gone in. Events can carry a link too.

use super::Game;
use crate::protocol::{Col, Line};

const KIND: &str = "meeting";
const MAX_LINK: usize = 300;

pub fn kind() -> &'static str {
    KIND
}

pub fn is_meeting_spot(name: &str) -> bool {
    let n = name.to_lowercase();
    ["meeting", "gazebo"].iter().any(|w| n.contains(w))
}

/// A call link worth keeping: https, one piece, not too long.
pub fn clean_link(raw: &str) -> Option<String> {
    let url = raw.trim().trim_end_matches(['.', ',', '!', '?', ';', ')']);
    let ok = url.starts_with("https://")
        && url.len() > "https://".len() + 3
        && url.chars().count() <= MAX_LINK
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | '"' | '\''));
    ok.then(|| url.to_string())
}

/// How many links one event or meeting spot keeps (a call, a board, notes).
const MAX_LINKS: usize = 3;

/// Take the links out of some words: "Standup https://meet… https://trello…"
/// → ("Standup", "https://meet… https://trello…"). Several are kept
/// together, a space apart.
pub fn split_link(text: &str) -> (String, Option<String>) {
    let mut links: Vec<String> = Vec::new();
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|w| {
            if links.len() < MAX_LINKS && w.starts_with("https://") {
                if let Some(l) = clean_link(w) {
                    links.push(l);
                    return false;
                }
            }
            true
        })
        .collect();
    (words.join(" "), (!links.is_empty()).then(|| links.join(" ")))
}

/// Links kept together, for reading: "https://a · https://b".
pub fn shown(links: &str) -> String {
    links.split_whitespace().collect::<Vec<_>>().join(" · ")
}

impl Game {
    pub(super) fn load_meetings(&mut self) {
        match self.world.db.meeting_links() {
            Ok(links) => self.meeting_links = links.into_iter().collect(),
            Err(e) => eprintln!("couldn't load meeting links: {e}"),
        }
    }

    /// The meeting spot at (x, y), if there is one.
    fn meeting_spot_at(&self, x: i64, y: i64) -> Option<(i64, String, String)> {
        self.world.things_at(x, y).find(|t| t.kind.as_deref() == Some(KIND)).map(|t| (t.id, t.name.clone(), t.maker.clone()))
    }

    pub(super) fn meeting_here(&self, id: u64) -> bool {
        self.players.get(&id).is_some_and(|p| self.meeting_spot_at(p.x, p.y).is_some())
    }

    /// The standing call link at (x, y), from a meeting spot there.
    pub(super) fn meeting_link_at(&self, x: i64, y: i64) -> Option<String> {
        let (thing, _, _) = self.meeting_spot_at(x, y)?;
        self.meeting_links.get(&thing).cloned()
    }

    /// For `look`: the call link here, if any.
    pub(super) fn meeting_lines_here(&self, id: u64) -> Vec<Line> {
        let Some(p) = self.players.get(&id) else { return vec![] };
        let Some((thing, name, _)) = self.meeting_spot_at(p.x, p.y) else { return vec![] };
        match self.meeting_links.get(&thing) {
            Some(link) => vec![Line::new().c(Col::Cyan, format!("🔗 {} has {}: ", super::capitalise(&super::the(&name)), if link.contains(' ') { "links" } else { "a link" })).c(Col::Cyan, shown(link)).c(Col::Dim, " (meet to get them)")],
            None => vec![Line::col(Col::Dim, "🔗 Nobody's left any links here yet (meet https://…)")],
        }
    }

    /// `meet` (go into the call here), `meet https://…` (set the spot's
    /// link) and `meet off` (clear it).
    pub(super) fn cmd_meet(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, name)) = self.here(id) else { return Ok(()) };
        let rest = rest.trim();
        let spot = self.meeting_spot_at(x, y);
        if rest.is_empty() {
            let link = spot.as_ref().and_then(|s| self.meeting_links.get(&s.0).cloned()).or_else(|| self.event_link_now(x, y));
            let Some(link) = link else {
                let msg = if spot.is_some() {
                    "There are no links here yet. Its maker (or a keeper) can add some: meet https://…"
                } else {
                    "There's nothing to join here. Gatherings with links happen at a meeting table: make a meeting table, then meet https://…"
                };
                self.say1(id, Line::plain(msg));
                return Ok(());
            };
            self.say1(id, Line::new().c(Col::Cyan, "🔗 Here you go: ").b(Col::Cyan, shown(&link)).c(Col::Dim, " (the island will be right here when you're back)"));
            let user = self.user(id);
            self.tell_here_from(x, y, id, &user, Line::col(Col::Cyan, format!("🔗 {name} heads off to the meeting.")));
            return Ok(());
        }
        let Some((thing, spot_name, maker)) = spot else {
            self.say1(id, Line::plain("Links live on meeting spots. Make one here first: make a meeting table"));
            return Ok(());
        };
        let user = self.user(id);
        let home_owner = self.world.home_at(x, y).is_some_and(|h| h.owner == user);
        if maker != user && !home_owner && !self.is_keeper(&user) {
            self.say1(id, Line::plain(format!("Only whoever made {} (or a keeper) can change its links.", super::the(&spot_name))));
            return Ok(());
        }
        let the_spot = super::the(&spot_name);
        if matches!(rest.to_lowercase().as_str(), "off" | "clear" | "none" | "stop") {
            self.world.db.set_meeting_link(thing, None, &user, super::boards::now())?;
            self.meeting_links.remove(&thing);
            self.say1(id, Line::col(Col::Cyan, format!("🔗 {} has no links now.", super::capitalise(&the_spot))));
            return Ok(());
        }
        let (_, Some(link)) = split_link(rest) else {
            self.say1(id, Line::plain("That doesn't look like a link. Links need to start https://"));
            return Ok(());
        };
        self.world.db.set_meeting_link(thing, Some(&link), &user, super::boards::now())?;
        self.meeting_links.insert(thing, link.clone());
        self.say1(id, Line::new().c(Col::Cyan, format!("🔗 {} now holds ", super::capitalise(&the_spot))).c(Col::Cyan, shown(&link)).c(Col::Dim, ". Anyone here can type meet to get them."));
        self.tell_here_from(x, y, id, &user, Line::col(Col::Cyan, format!("🔗 {name} leaves some links at {the_spot}. (meet to get them)")));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_the_link_out_of_an_events_name() {
        let (name, link) = split_link("Standup https://meet.google.com/abc-defg-hij");
        assert_eq!((name.as_str(), link.as_deref()), ("Standup", Some("https://meet.google.com/abc-defg-hij")));
        let (name, link) = split_link("Retro (https://zoom.us/j/123) with cake");
        assert_eq!(link, None, "a bracketed link isn't a clean word");
        assert!(name.contains("Retro"));
        let (name, link) = split_link("Standup https://meet.google.com/abc https://trello.com/b/xyz/team");
        assert_eq!((name.as_str(), link.as_deref()), ("Standup", Some("https://meet.google.com/abc https://trello.com/b/xyz/team")));
        assert_eq!(shown(&link.unwrap()), "https://meet.google.com/abc · https://trello.com/b/xyz/team");
        let (_, link) = split_link("x https://a.bc/1 https://a.bc/2 https://a.bc/3 https://a.bc/4");
        assert_eq!(link.unwrap().split(' ').count(), 3, "three at most");
        let (name, link) = split_link("Beach party");
        assert_eq!((name.as_str(), link), ("Beach party", None));
    }

    #[test]
    fn only_tidy_https_links() {
        assert_eq!(clean_link("https://meet.google.com/abc.").as_deref(), Some("https://meet.google.com/abc"));
        assert_eq!(clean_link("http://meet.google.com/abc"), None);
        assert_eq!(clean_link("https://"), None);
        assert_eq!(clean_link("https://a.b/\"><script>"), None);
        assert_eq!(clean_link(&format!("https://x.y/{}", "a".repeat(400))), None);
    }

    #[test]
    fn knows_a_meeting_spot() {
        assert!(is_meeting_spot("a round meeting table"));
        assert!(is_meeting_spot("a little Gazebo"));
        assert!(!is_meeting_spot("a bench"));
    }
}
