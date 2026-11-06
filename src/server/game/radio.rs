//! Radios: tune one to an internet radio stream and everyone nearby hears it,
//! louder the closer they are. The world never touches the sound itself: it
//! tells each nearby browser which stream to play and how loud, and the
//! browser plays it straight from the station.
//!
//! While someone's listening, the world also looks for what's on now and
//! then: the stream's own title (ICY metadata, which many stations fill in),
//! or failing that the show on air from the radio's programme guide, if it
//! has one (AirNet's format, used by many community stations).

use super::Game;
use super::gramophone::gain_at;
use crate::protocol::{Col, Line, ToClient};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const KIND: &str = "radio";
/// How often to look for what's on, while someone's listening.
const NEWS_EVERY: Duration = Duration::from_secs(30);
const MAX_URL: usize = 400;

pub fn is_radio(name: &str) -> bool {
    name.to_lowercase().split(|c: char| !c.is_alphanumeric()).any(|w| w == "radio" || w == "wireless")
}

pub fn kind() -> &'static str {
    KIND
}

fn meta_key(thing: i64) -> String {
    format!("radio:{thing}")
}

fn guide_key(thing: i64) -> String {
    format!("radio_guide:{thing}")
}

/// What a radio's on, as far as the world last looked.
#[derive(Default)]
pub struct News {
    pub title: Option<String>,
    looked: Option<Instant>,
    looking: bool,
}

/// A stream address the browser can play from a secure page: https only.
pub fn clean_stream(raw: &str) -> Result<String, &'static str> {
    let url = raw.trim();
    if url.starts_with("http://") {
        return Err("That's an http:// stream. Browsers only play https:// streams on a secure page; many stations have one.");
    }
    if !url.starts_with("https://") || url.len() <= "https://".len() || url.len() > MAX_URL || url.contains(char::is_whitespace) {
        return Err("Tune it to a stream's https:// address, e.g. 'tune radio https://stream.example.com/live.mp3'");
    }
    Ok(url.to_string())
}

/// A station's name fit to show and type: letters, numbers, spaces and a
/// little punctuation.
pub fn clean_station_name(raw: &str) -> String {
    let kept: String = raw.chars().filter(|c| c.is_alphanumeric() || " -'.&".contains(*c)).take(30).collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Just the station's address, for saying what's on.
pub fn station(url: &str) -> &str {
    url.trim_start_matches("https://").split(['/', '?', ':']).next().unwrap_or(url)
}

impl Game {
    /// Every radio's stream, from the db (at start).
    pub(super) fn load_radios(&mut self) {
        let ids: Vec<i64> = self.world.things.iter().filter(|t| t.kind.as_deref() == Some(KIND)).map(|t| t.id).collect();
        let saved = |key: String| self.world.db.meta(&key).filter(|u| !u.is_empty());
        self.radios = ids.iter().filter_map(|id| saved(meta_key(*id)).map(|u| (*id, u))).collect();
        self.radio_guides = ids.iter().filter_map(|id| saved(guide_key(*id)).map(|u| (*id, u))).collect();
    }

    fn radio_here(&self, x: i64, y: i64) -> Option<usize> {
        self.world.things.iter().position(|t| t.x == x && t.y == y && t.kind.as_deref() == Some(KIND))
    }

    pub(super) fn has_radio_here(&self, id: u64) -> bool {
        self.players.get(&id).is_some_and(|p| self.radio_here(p.x, p.y).is_some())
    }

    /// `tune radio https://…` (its maker, or a keeper).
    pub(super) fn cmd_tune(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let Some(i) = self.radio_here(x, y) else {
            self.say1(id, Line::plain("There's no radio here to tune. Try 'make a radio'!"));
            return Ok(());
        };
        let raw = rest.trim().strip_prefix("radio").unwrap_or(rest.trim()).trim();
        let raw = raw.strip_prefix("to ").unwrap_or(raw).trim().to_lowercase();
        // Radios only play the world's own stations, which keepers choose.
        let stations = self.world.db.stations()?;
        let names: Vec<&str> = stations.iter().map(|(n, _, _)| n.as_str()).collect();
        let exact = stations.iter().find(|(n, _, _)| n.to_lowercase() == raw);
        let partly: Vec<_> = stations.iter().filter(|(n, _, _)| !raw.is_empty() && n.to_lowercase().contains(&raw)).collect();
        let Some((_, url, guide)) = exact.or(if partly.len() == 1 { partly.first().copied() } else { None }) else {
            let msg = if names.is_empty() {
                "There are no stations yet. Island keepers can add some: station add Night Owl FM https://…".to_string()
            } else if raw.is_empty() || partly.is_empty() {
                format!("Tune it to one of the island's stations: {}.", names.join(", "))
            } else {
                let maybe: Vec<&str> = partly.iter().map(|(n, _, _)| n.as_str()).collect();
                format!("Which one? {}.", maybe.join(", "))
            };
            self.say1(id, Line::plain(msg));
            return Ok(());
        };
        let (url, guide) = (url.clone(), guide.clone());
        let thing = self.world.things[i].id;
        self.world.db.set_meta(&meta_key(thing), &url)?;
        // A station brings its own programme guide along (or none).
        self.world.db.set_meta(&guide_key(thing), guide.as_deref().unwrap_or(""))?;
        match &guide {
            Some(g) => self.radio_guides.insert(thing, g.clone()),
            None => self.radio_guides.remove(&thing),
        };
        let on = self.station_label(&url);
        self.radios.insert(thing, url);
        self.radio_news.remove(&thing);
        self.say1(id, Line::new().c(Col::Pink, "📻 You tune the radio to ").b(Col::Pink, on.clone()).c(Col::Pink, ". Everyone nearby with sound on can hear it."));
        self.tell_here(x, y, id, Line::col(Col::Pink, format!("📻 {who} tunes the radio to {on}.")));
        Ok(())
    }

    /// `radio off` (its maker, or a keeper).
    pub(super) fn cmd_radio_off(&mut self, id: u64) -> anyhow::Result<()> {
        let Some((x, y, who)) = self.here(id) else { return Ok(()) };
        let Some(i) = self.radio_here(x, y) else {
            self.say1(id, Line::plain("There's no radio here."));
            return Ok(());
        };
        let user = self.user(id);
        if self.world.things[i].maker != user && !self.is_keeper(&user) {
            self.say1(id, Line::plain("Only its maker (or a keeper) can switch it off. You can always wander off!"));
            return Ok(());
        }
        let thing = self.world.things[i].id;
        self.world.db.set_meta(&meta_key(thing), "")?;
        self.radios.remove(&thing);
        self.say1(id, Line::col(Col::Dim, "📻 Click. The radio goes quiet."));
        self.tell_here(x, y, id, Line::col(Col::Dim, format!("📻 {who} switches the radio off.")));
        Ok(())
    }

    /// What to call a stream: its station's name, if it's one of the world's.
    pub(super) fn station_label(&self, url: &str) -> String {
        self.world
            .db
            .stations()
            .ok()
            .and_then(|s| s.into_iter().find(|(_, u, _)| u == url).map(|(n, _, _)| n))
            .unwrap_or_else(|| station(url).to_string())
    }

    /// `stations`: the world's radio stations.
    pub(super) fn cmd_stations(&mut self, id: u64) -> anyhow::Result<()> {
        let list = self.world.db.stations()?;
        let mut lines = vec![Line::new().b(Col::Magenta, "📻 Radio stations")];
        if list.is_empty() {
            lines.push(Line::col(Col::Dim, "  None yet. Keepers can add some: station add Night Owl FM https://…"));
        }
        for (name, url, guide) in &list {
            let extra = if guide.is_some() { " · says what's on" } else { "" };
            lines.push(Line::new().t("  ").b(Col::Cyan, name).c(Col::Dim, format!("  {}{extra}", station(url))));
        }
        if !list.is_empty() {
            lines.push(Line::col(Col::Dim, "  At a radio: tune radio <station>"));
        }
        self.send(id, lines);
        Ok(())
    }

    /// Keepers: `station add Night Owl FM https://… [guide https://…]`, `station remove Night Owl FM`.
    pub(super) fn cmd_station(&mut self, id: u64, rest: &str) -> anyhow::Result<()> {
        let rest = rest.trim();
        let (verb, args) = rest.split_once(char::is_whitespace).map_or((rest, ""), |(v, a)| (v, a.trim()));
        if !matches!(verb, "add" | "remove" | "rm" | "delete") {
            return self.cmd_stations(id);
        }
        if !self.is_keeper(&self.user(id)) {
            self.say1(id, Line::plain("Only island keepers can change the station list. Anyone can tune to them, though!"));
            return Ok(());
        }
        if verb != "add" {
            let name = clean_station_name(args);
            let msg = if self.world.db.remove_station(&name)? { format!("📻 {name} is off the list.") } else { format!("There's no station called '{name}'.") };
            self.say1(id, Line::col(Col::Dim, msg));
            return Ok(());
        }
        // The name is everything before the stream's address.
        let Some(at) = args.find("https://").or_else(|| args.find("http://")) else {
            self.say1(id, Line::plain("Add which station? e.g. 'station add Night Owl FM https://stream.example.com/live' (and 'guide https://…' after it, for a programme guide)"));
            return Ok(());
        };
        let name = clean_station_name(&args[..at]);
        let mut after = args[at..].split_whitespace();
        let url = after.next().unwrap_or_default();
        let guide = match (after.next(), after.next()) {
            (Some("guide"), Some(g)) => Some(g),
            _ => None,
        };
        if name.is_empty() {
            self.say1(id, Line::plain("Give it a name first, e.g. 'station add Night Owl FM https://…'"));
            return Ok(());
        }
        let (url, guide) = match (clean_stream(url), guide.map(clean_stream).transpose()) {
            (Ok(u), Ok(g)) => (u, g),
            (Err(why), _) | (_, Err(why)) => {
                self.say1(id, Line::plain(why));
                return Ok(());
            }
        };
        self.world.db.set_station(&name, &url, guide.as_deref(), &self.user(id), super::super::accounts::now())?;
        self.say1(id, Line::col(Col::Green, format!("📻 {name} is on the list. Anyone can 'tune radio {name}' at a radio now.")));
        Ok(())
    }

    /// A radio was unmade.
    pub(super) fn forget_radio(&mut self, thing: i64) {
        if self.radios.remove(&thing).is_some() {
            let _ = self.world.db.set_meta(&meta_key(thing), "");
        }
        if self.radio_guides.remove(&thing).is_some() {
            let _ = self.world.db.set_meta(&guide_key(thing), "");
        }
        self.radio_news.remove(&thing);
    }

    /// Look for what's on any radio someone's listening to, now and then.
    fn look_for_news(&mut self) {
        let heard: std::collections::HashSet<i64> = self.players.values().flat_map(|p| p.radios.keys().copied()).collect();
        for thing in heard {
            let Some(url) = self.radios.get(&thing).cloned() else { continue };
            let news = self.radio_news.entry(thing).or_default();
            if news.looking || news.looked.is_some_and(|t| t.elapsed() < NEWS_EVERY) {
                continue;
            }
            news.looking = true;
            let guide = self.radio_guides.get(&thing).cloned();
            let inbox = self.inbox.clone();
            tokio::spawn(async move {
                let title = match stream_title(&url).await {
                    Some(t) => Some(format!("Now playing: {t}")),
                    None => match guide {
                        Some(g) => on_air(&g).await.map(|t| format!("On air: {t}")),
                        None => None,
                    },
                };
                let _ = inbox.send(super::super::Event::RadioNews { thing, title });
            });
        }
    }

    /// What a radio's on, from a look: tell its listeners if it's changed.
    pub(super) fn radio_news(&mut self, thing: i64, title: Option<String>) {
        let news = self.radio_news.entry(thing).or_default();
        news.looking = false;
        news.looked = Some(Instant::now());
        if title == news.title {
            return;
        }
        news.title = title.clone();
        let Some(title) = title else { return };
        for p in self.players.values().filter(|p| p.radios.contains_key(&thing)) {
            let _ = p.tx.send(ToClient::Lines { lines: vec![Line::col(Col::Pink, format!("📻 {title}"))] });
        }
    }

    /// Tell each browser which radios it can hear and how loud, when that
    /// changes (they moved, or a radio was tuned, moved or switched off).
    pub(super) fn update_radios(&mut self) {
        // Each person hears only the nearest sound, so a radio is only theirs
        // if it's nearer than any other radio or gramophone.
        let heard: HashMap<u64, Option<i64>> =
            self.players.iter().map(|(pid, p)| (*pid, self.sound_heard_at(p.x, p.y))).collect();
        let radios: HashMap<i64, (i64, i64, String)> = self
            .radios
            .iter()
            .filter_map(|(id, url)| self.world.things.iter().find(|t| t.id == *id).map(|t| (*id, (t.x, t.y, url.clone()))))
            .collect();
        let news = &self.radio_news;
        let labels: HashMap<i64, String> = radios.iter().map(|(id, (_, _, url))| (*id, self.station_label(url))).collect();
        for (pid, p) in self.players.iter_mut().filter(|(_, p)| !p.bot) {
            let mut hearing: HashMap<i64, (f32, String)> = HashMap::new();
            if let Some((id, (x, y, url))) = heard[pid].and_then(|id| radios.get(&id).map(|r| (id, r)))
                && let Some(gain) = gain_at((p.x - x).abs().max((p.y - y).abs())) {
                    hearing.insert(id, (gain, url.clone()));
                }
            for (id, (gain, url)) in &hearing {
                if p.radios.get(id) != Some(&(*gain, url.clone())) {
                    if p.radios.get(id).is_none_or(|(_, u)| u != url) {
                        let on = news.get(id).and_then(|n| n.title.as_deref()).map(|t| format!(" {t}")).unwrap_or_default();
                        let line = Line::col(Col::Pink, format!("📻 A radio nearby is playing {}.{on}", labels[id]));
                        let _ = p.tx.send(ToClient::Lines { lines: vec![line] });
                    }
                    let _ = p.tx.send(ToClient::Radio { thing: *id, url: Some(url.clone()), gain: *gain });
                }
            }
            for id in p.radios.keys().filter(|id| !hearing.contains_key(id)) {
                let _ = p.tx.send(ToClient::Radio { thing: *id, url: None, gain: 0.0 });
            }
            p.radios = hearing;
        }
        self.look_for_news();
    }
}

fn web() -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent("world.txt radio")
        .build()
        .ok()
}

/// The stream's own "now playing", from its ICY metadata, if it has one.
async fn stream_title(url: &str) -> Option<String> {
    let mut resp = web()?.get(url).header("Icy-MetaData", "1").send().await.ok()?;
    let every: usize = resp.headers().get("icy-metaint")?.to_str().ok()?.parse().ok()?;
    if every == 0 || every > 1 << 20 {
        return None;
    }
    // The audio up to the first metadata block, its length byte, then the block.
    let mut buf: Vec<u8> = Vec::new();
    while buf.len() <= every {
        buf.extend_from_slice(&resp.chunk().await.ok()??);
    }
    let len = buf[every] as usize * 16;
    while buf.len() < every + 1 + len {
        buf.extend_from_slice(&resp.chunk().await.ok()??);
    }
    icy_title(&buf[every + 1..every + 1 + len])
}

/// `StreamTitle='Artist - Song';StreamUrl='…';` → "Artist - Song"
fn icy_title(block: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(block);
    let start = text.find("StreamTitle='")? + "StreamTitle='".len();
    let rest = &text[start..];
    let end = rest.find("';").unwrap_or(rest.trim_end_matches(['\0', '\'']).len());
    super::gramophone::clean_title(&rest[..end])
}

/// The show on air now, from a programme guide in AirNet's format.
async fn on_air(guide: &str) -> Option<String> {
    let shows: Vec<serde_json::Value> = web()?.get(guide).send().await.ok()?.json().await.ok()?;
    shows.iter().find(|s| s["onairnow"] == true).and_then(show_name)
}

/// Guides send text HTML-escaped ("Ian &amp; Adele").
fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&#039;", "'").replace("&amp;", "&")
}

fn show_name(show: &serde_json::Value) -> Option<String> {
    let name = unescape(show["name"].as_str()?.trim());
    let who = show["broadcasters"].as_str().map(|b| unescape(b.trim())).filter(|b| !b.is_empty());
    super::gramophone::clean_title(&match who {
        Some(b) => format!("{name} with {b}"),
        None => name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radios_take_secure_streams_and_name_the_station() {
        assert!(is_radio("an old radio") && is_radio("a crackly wireless"));
        assert!(!is_radio("a radiator"));
        assert_eq!(clean_stream(" https://stream.example.com/live.mp3 ").unwrap(), "https://stream.example.com/live.mp3");
        assert!(clean_stream("http://stream.example.com/live").unwrap_err().contains("https"));
        assert!(clean_stream("not a url").is_err());
        assert!(clean_stream("https://").is_err());
        assert_eq!(station("https://stream.example.com:8000/live.mp3?x=1"), "stream.example.com");
    }

    #[test]
    fn whats_on_comes_from_the_stream_or_the_guide() {
        assert_eq!(icy_title(b"StreamTitle='Artist - A Song';StreamUrl='https://x';\0\0").as_deref(), Some("Artist - A Song"));
        assert_eq!(icy_title(b"StreamTitle='';\0\0\0"), None);
        assert_eq!(icy_title(b"nothing here"), None);
        let show = serde_json::json!({"name": "Night Owls", "broadcasters": "Poppy &amp; Sam", "onairnow": true});
        assert_eq!(show_name(&show).as_deref(), Some("Night Owls with Poppy & Sam"));
        assert_eq!(show_name(&serde_json::json!({"name": "Breakfast", "broadcasters": ""})).as_deref(), Some("Breakfast"));
    }
}
