//! Events: keepers schedule things at a place and time ("event sat 4pm Beach
//! party"). They go in the Island Journal, everyone hears a reminder ten
//! minutes before and a shout when they start, and 'go event' walks you there.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::db::EventRow;
use chrono::{Datelike, Duration, Local, NaiveTime, TimeZone, Weekday};

const REMIND_BEFORE: i64 = 10 * 60;
const KEEP_AFTER: i64 = 3 * 3600;

/// Read "sat 4pm", "tomorrow 10:30am", "16:00", "noon" from the start of the
/// text. Returns when, and the rest of the text (the event's name).
pub fn parse_when(text: &str, now: chrono::DateTime<Local>) -> Option<(i64, String)> {
    let mut words: Vec<&str> = text.split_whitespace().collect();
    let day = words.first().and_then(|w| day_word(w, now.weekday()));
    if day.is_some() {
        words.remove(0);
    }
    let time = words.first().and_then(|w| time_word(w))?;
    words.remove(0);
    let name = words.join(" ");
    if name.trim().is_empty() {
        return None;
    }
    let today = now.date_naive();
    let date = match day {
        Some(ahead) => today + Duration::days(ahead),
        None => today,
    };
    let mut at = Local.from_local_datetime(&date.and_time(time)).earliest()?;
    if day.is_none() && at <= now {
        at += Duration::days(1); // no day given: the next time it's that hour
    }
    (at > now).then(|| (at.timestamp(), name))
}

/// How many days ahead a day word means.
fn day_word(w: &str, today: Weekday) -> Option<i64> {
    let w = w.to_lowercase();
    match w.as_str() {
        "today" | "tonight" => return Some(0),
        "tomorrow" | "tmrw" => return Some(1),
        _ => {}
    }
    let days = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];
    let i = days.iter().position(|d| w.starts_with(d))? as i64;
    Some((i - today.num_days_from_monday() as i64).rem_euclid(7))
}

fn time_word(w: &str) -> Option<NaiveTime> {
    let w = w.to_lowercase();
    match w.as_str() {
        "noon" | "midday" => return NaiveTime::from_hms_opt(12, 0, 0),
        "midnight" => return NaiveTime::from_hms_opt(0, 0, 0),
        _ => {}
    }
    let (digits, pm) = if let Some(d) = w.strip_suffix("pm") {
        (d, Some(true))
    } else if let Some(d) = w.strip_suffix("am") {
        (d, Some(false))
    } else {
        (w.as_str(), None)
    };
    let (h, m) = match digits.split_once([':', '.']) {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (digits.parse::<u32>().ok()?, 0),
    };
    let h = match pm {
        Some(true) if h < 12 => h + 12,
        Some(false) if h == 12 => 0,
        None if digits.contains([':', '.']) || h > 12 => h,
        None => return None, // "4" on its own is too vague
        _ => h,
    };
    NaiveTime::from_hms_opt(h, m, 0)
}

fn with_call(line: Line, link: &Option<String>) -> Line {
    match link {
        Some(l) => line.c(Col::Cyan, " 📞 ").c(Col::Cyan, l.clone()),
        None => line,
    }
}

/// "Sat 7 Nov, 4:00pm"
pub fn when_label(at: i64) -> String {
    let t = Local.timestamp_opt(at, 0).single().unwrap_or_else(Local::now);
    t.format("%a %-d %b, %-I:%M%P").to_string()
}

/// How far away people hear about a place event: about as far as a shout.
const NEARBY: i64 = 10;

/// "here fri 10am Standup" → a place event; anything else is island-wide.
pub fn place_event(text: &str) -> (bool, &str) {
    let t = text.trim_start();
    for p in ["here ", "local ", "place "] {
        if t.len() >= p.len() && t[..p.len()].eq_ignore_ascii_case(p) {
            return (true, t[p.len()..].trim_start());
        }
    }
    (false, t)
}

/// "Standup with Poppy, Sam and Lu" → ("Standup", their usernames), when
/// every name is someone; "Tea with cake" (nobody's called cake) stays as it
/// is (None); some names but not others: Err(the unknown ones).
pub fn invited(name: &str, lookup: impl Fn(&str) -> Vec<String>) -> Option<Result<(String, Vec<String>), Vec<String>>> {
    let lower = name.to_lowercase();
    let at = lower.rfind(" with ")?;
    let (what, who) = (name[..at].trim(), &name[at + " with ".len()..]);
    let names: Vec<&str> = who
        .split([',', '&'])
        .flat_map(|part| part.split(" and "))
        .map(|n| n.trim().trim_start_matches('@').trim())
        .filter(|n| !n.is_empty())
        .collect();
    if what.is_empty() || names.is_empty() {
        return None;
    }
    let mut users = Vec::new();
    let mut unknown = Vec::new();
    for n in &names {
        match lookup(n).as_slice() {
            [one] => users.push(one.clone()),
            _ => unknown.push(n.to_string()),
        }
    }
    match (users.is_empty(), unknown.is_empty()) {
        (true, _) => None,
        (false, true) => Some(Ok((what.to_string(), users))),
        (false, false) => Some(Err(unknown)),
    }
}

impl Game {
    /// Who hears about an event: everyone online, or for a place event its
    /// host, whoever's going, followers of a noticeboard on that spot, and
    /// anyone nearby.
    fn audience(&self, e: &EventRow) -> Vec<u64> {
        let insiders = self.insiders(e);
        self.players.iter().filter(|(_, p)| self.in_on(e, &insiders, &p.user, p.x, p.y)).map(|(&id, _)| id).collect()
    }

    /// For a place event: who's going, and who follows a board on its spot.
    fn insiders(&self, e: &EventRow) -> Vec<String> {
        if !e.local {
            return vec![];
        }
        let mut people = self.world.db.going(e.id).unwrap_or_default();
        if let Some((board, false)) = self.board_here(e.x, e.y) {
            people.extend(self.world.db.followers(board).unwrap_or_default());
        }
        people
    }

    /// Whether someone (standing at x, y) is in on an event: hears about it,
    /// and sees its call link.
    fn in_on(&self, e: &EventRow, insiders: &[String], user: &str, x: i64, y: i64) -> bool {
        use super::super::islands::island_at;
        !e.local
            || user == e.host
            || insiders.iter().any(|u| u == user)
            || ((x - e.x).abs().max((y - e.y).abs()) <= NEARBY && island_at(x, y).id == island_at(e.x, e.y).id)
    }

    fn tell_audience(&self, e: &EventRow, except: Option<u64>, line: Line) {
        for id in self.audience(e) {
            if Some(id) != except {
                self.say1(id, line.clone());
            }
        }
    }

    pub(super) fn cmd_event(&mut self, id: u64, text: &str) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        let host = self.user(id);
        let (local, text) = place_event(text);
        if !local && !self.is_keeper(&host) {
            self.say1(id, Line::plain("Only island keepers can plan events for the whole island. Anyone can plan one just for here, though: event here fri 10am Standup"));
            return Ok(());
        }
        let (text, link) = super::meetings::split_link(text);
        let link = link.or_else(|| self.meeting_link_at(x, y));
        let Some((at, name)) = parse_when(&text, Local::now()) else {
            self.say1(id, Line::plain("When, and what? e.g. 'event sat 4pm Beach party', or just for this place: 'event here fri 10am Standup'. It happens right here."));
            return Ok(());
        };
        // "Standup with Poppy, Sam and Lu": those three are going too.
        let (name, invited) = match local.then(|| invited(&name, |n| self.world.db.matching_users(n).unwrap_or_default())).flatten() {
            Some(Ok((name, users))) => (name, users),
            Some(Err(unknown)) => {
                self.say1(id, Line::plain(format!("Who's {}? Check the names with 'users' or 'who' (or use their usernames), then plan it again.", super::super::world::join_and(&unknown))));
                return Ok(());
            }
            None => (name, vec![]),
        };
        let name: String = name.chars().take(80).collect();
        let place = self.world.place_title(x, y);
        let event = self.world.db.add_event(at, &name, x, y, &host, link.as_deref(), local)?;
        for u in &invited {
            self.world.db.set_going(event, u, true)?;
        }
        let when = when_label(at);
        let call = link.as_ref().map_or(String::new(), |l| format!(" · 📞 {l}"));
        let now = super::boards::now();
        let who = if local {
            self.world.db.set_going(event, &host, true)?;
            "Whoever's going, people following a board here, and anyone nearby will get a reminder."
        } else {
            "Everyone will get a reminder."
        };
        self.say1(id, Line::new().c(Col::Yellow, "📅 Planned: ").b(Col::Yellow, &name).c(Col::Yellow, format!(" · {when} at {place}{call}. {who}")));
        if !invited.is_empty() {
            let names: Vec<String> = invited.iter().map(|u| self.world.nick(u)).collect();
            self.say1(id, Line::col(Col::Dim, format!("   {} {} going too, and will hear about it wherever they are.", super::super::world::join_and(&names), if names.len() == 1 { "is" } else { "are" })));
        }
        // Written down where people will find it: the Island Journal for the
        // whole island, or the noticeboard here for a place event.
        let board = if local { self.board_here(x, y).filter(|b| !b.1).map(|b| b.0) } else { self.journal_board };
        if let Some(board) = board {
            // Anyone can read a board, so a place event's call link stays off it.
            let call = if local { "" } else { call.as_str() };
            self.world.db.add_note(board, &host, &format!("📅 {name} · {when} at {place}{call}"), now, &super::boards::week_key(now), None)?;
        }
        let row = self.world.db.upcoming_events(at)?.into_iter().find(|e| e.id == event);
        let invite = if local { " (going to say you'll come)" } else { "" };
        let line = Line::new().c(Col::Yellow, "📅 Coming up: ").b(Col::Yellow, &name).c(Col::Yellow, format!(" · {when} at {place}{call}")).c(Col::Dim, invite);
        if let Some(e) = row {
            self.tell_audience(&e, Some(id), line);
            // The invited who are away: a note waiting for when they arrive.
            let hostname = self.world.nick(&host);
            for u in &invited {
                if *u != host && !self.players.values().any(|p| p.user == *u) {
                    let note = format!("📅 invited you to {name} · {when} at {place}{call} (not going drops out)");
                    self.world.db.send_mail(u, &host, &note, now)?;
                }
            }
            let line = Line::col(Col::Yellow, format!("📅 {hostname} has invited you to {name}. (not going drops out)"));
            for (&pid, p) in &self.players {
                if pid != id && invited.contains(&p.user) {
                    self.say1(pid, line.clone());
                }
            }
        }
        Ok(())
    }

    pub(super) fn cmd_events(&mut self, id: u64) -> anyhow::Result<()> {
        let events = self.world.db.upcoming_events(super::boards::now() - KEEP_AFTER)?;
        let me = self.user(id);
        let (mx, my) = self.players.get(&id).map_or((-1, -1), |p| (p.x, p.y));
        let mut lines = vec![Line::new().b(Col::Magenta, "📅 Coming up on the island")];
        if events.is_empty() {
            lines.push(Line::col(Col::Dim, "  Nothing planned yet. Plan one just for here: event here fri 10am Standup"));
        }
        for (i, e) in events.iter().enumerate() {
            let place = self.world.place_title(e.x, e.y);
            let now = super::boards::now();
            let when = if e.at <= now { "happening now!".to_string() } else { when_label(e.at) };
            let pin = if e.local { "📍 " } else { "" };
            lines.push(
                Line::new()
                    .c(Col::Yellow, format!("  {}. {pin}", i + 1))
                    .b(Col::Yellow, e.name.clone())
                    .t(format!(" · {when} at {place}"))
                    .c(Col::Dim, format!(" (hosted by {})", self.world.nick(&e.host))),
            );
            if let Some(l) = &e.link {
                if self.in_on(e, &self.insiders(e), &me, mx, my) {
                    lines.push(Line::new().c(Col::Cyan, "     📞 ").c(Col::Cyan, l.clone()));
                } else {
                    lines.push(Line::col(Col::Dim, "     📞 has a call link, for whoever's going"));
                }
            }
            if e.local {
                let going = self.world.db.going(e.id)?;
                let names: Vec<String> = going.iter().map(|u| if *u == me { "you".to_string() } else { self.world.nick(u) }).collect();
                let verb = if names.len() == 1 && names[0] != "you" { "is" } else { "are" };
                let text = if names.is_empty() { "nobody's said they're going yet".to_string() } else { format!("{} {verb} going", super::super::world::join_and(&names)) };
                lines.push(Line::col(Col::Dim, format!("     {text}")));
            }
        }
        if !events.is_empty() {
            lines.push(Line::col(Col::Dim, "  go event 2 walks you there · going 2 / not going 2 for 📍 events just for one place"));
        }
        self.send(id, lines);
        Ok(())
    }

    fn pick_event(&self, which: &str) -> anyhow::Result<Option<EventRow>> {
        let events = self.world.db.upcoming_events(super::boards::now() - KEEP_AFTER)?;
        let n: usize = which.trim().trim_start_matches("to").trim().trim_start_matches("event").trim().parse().unwrap_or(1);
        Ok(events.into_iter().nth(n.saturating_sub(1)))
    }

    /// `going 2` / `not going 2`: say you'll come to a place event, so you
    /// hear its reminder wherever you are.
    pub(super) fn cmd_going(&mut self, id: u64, which: &str, going: bool) -> anyhow::Result<()> {
        let Some(e) = self.pick_event(which)? else {
            self.say1(id, Line::plain("Going to which? 'events' lists them, then e.g. 'going 2'."));
            return Ok(());
        };
        let me = self.user(id);
        if !e.local {
            self.say1(id, Line::plain(format!("{} is for the whole island: everyone hears about it anyway.", e.name)));
            return Ok(());
        }
        let changed = self.world.db.set_going(e.id, &me, going)?;
        let place = self.world.place_title(e.x, e.y);
        let msg = match (going, changed) {
            (true, true) => format!(
                "📅 You're going to {} ({} at {place}{}). You'll get a reminder wherever you are.",
                e.name,
                when_label(e.at),
                e.link.as_ref().map_or(String::new(), |l| format!(" · 📞 {l}"))
            ),
            (true, false) => format!("📅 You're already going to {}.", e.name),
            (false, true) => format!("📅 You're not going to {} any more.", e.name),
            (false, false) => format!("📅 You weren't going to {} anyway.", e.name),
        };
        self.say1(id, Line::col(Col::Yellow, msg));
        if going && changed && e.host != me {
            if let Some((&host, _)) = self.players.iter().find(|(_, p)| p.user == e.host) {
                let name = self.world.nick(&me);
                self.say1(host, Line::col(Col::Yellow, format!("📅 {name} is going to {}.", e.name)));
            }
        }
        Ok(())
    }

    pub(super) fn cmd_go_event(&mut self, id: u64, which: &str) -> anyhow::Result<()> {
        let Some(e) = self.pick_event(which)? else {
            self.say1(id, Line::plain("There's nothing coming up to go to. ('events' lists them)"));
            return Ok(());
        };
        let place = self.world.place_title(e.x, e.y);
        if let Some(p) = self.players.get_mut(&id) {
            p.travel = None;
            p.walk_to = Some((e.x, e.y, place.clone()));
        }
        self.say1(id, Line::col(Col::Cyan, format!("🚶 You set off for {} at {place}. (stop to stop)", e.name)));
        self.step_travellers();
        Ok(())
    }

    pub(super) fn cmd_cancel_event(&mut self, id: u64, which: &str) -> anyhow::Result<()> {
        let Some(e) = self.pick_event(which)? else {
            self.say1(id, Line::plain("Cancel which? 'events' lists them, then e.g. 'cancel event 2'."));
            return Ok(());
        };
        let me = self.user(id);
        if e.host != me && !self.is_keeper(&me) {
            self.say1(id, Line::plain("Only its host or a keeper can cancel that."));
            return Ok(());
        }
        // Tell the same people who'd have heard about it, before it's gone.
        self.tell_audience(&e, None, Line::col(Col::Dim, format!("📅 {} has been cancelled.", e.name)));
        if !self.audience(&e).contains(&id) {
            self.say1(id, Line::col(Col::Dim, format!("📅 {} has been cancelled.", e.name)));
        }
        self.world.db.remove_event(e.id)?;
        Ok(())
    }

    /// The call link of an event happening at (x, y) right now, for `meet`.
    pub(super) fn event_link_now(&self, x: i64, y: i64) -> Option<String> {
        let now = super::boards::now();
        let events = self.world.db.upcoming_events(now - KEEP_AFTER).ok()?;
        events.into_iter().filter(|e| (e.x, e.y) == (x, y) && e.at - now <= REMIND_BEFORE).find_map(|e| e.link)
    }

    /// Reminders shortly before, and a shout when things start.
    pub(super) fn events_tick(&mut self) {
        let now = super::boards::now();
        let Ok(events) = self.world.db.upcoming_events(now - KEEP_AFTER) else { return };
        for e in events {
            let place = self.world.place_title(e.x, e.y);
            let line = if !e.started && e.at <= now {
                let _ = self.world.db.mark_event(e.id, true, true);
                with_call(Line::new().b(Col::Yellow, format!("🎉 {} is starting now at {place}!", e.name)), &e.link).c(Col::Dim, " (go event to head there)")
            } else if !e.reminded && e.at - now <= REMIND_BEFORE && e.at > now {
                let _ = self.world.db.mark_event(e.id, true, false);
                let mins = ((e.at - now) / 60).max(1);
                let mins = if mins == 1 { "1 minute".to_string() } else { format!("{mins} minutes") };
                with_call(Line::new().c(Col::Yellow, format!("📅 In {mins}: ")).b(Col::Yellow, e.name.clone()).c(Col::Yellow, format!(" at {place}")), &e.link).c(Col::Dim, " (go event)")
            } else {
                continue;
            };
            self.tell_audience(&e, None, line);
        }
        let _ = self.world.db.forget_events_before(now - KEEP_AFTER);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_friendly_times() {
        // A Wednesday morning.
        let now = Local.with_ymd_and_hms(2026, 11, 4, 9, 0, 0).unwrap();
        let (at, name) = parse_when("sat 4pm Beach party", now).unwrap();
        let t = Local.timestamp_opt(at, 0).unwrap();
        assert_eq!((t.weekday(), t.format("%H:%M").to_string(), name.as_str()), (Weekday::Sat, "16:00".into(), "Beach party"));
        let (at, _) = parse_when("8am Breakfast", now).unwrap();
        assert_eq!(Local.timestamp_opt(at, 0).unwrap().day(), 5, "8am has passed today, so tomorrow");
        let (at, _) = parse_when("tomorrow 10:30am Treasure hunt", now).unwrap();
        assert_eq!(Local.timestamp_opt(at, 0).unwrap().format("%d %H:%M").to_string(), "05 10:30");
        assert!(parse_when("sat Beach party", now).is_none(), "needs a time");
        assert!(parse_when("4pm", now).is_none(), "needs a name");
        assert!(parse_when("4 Beach", now).is_none(), "a bare number is too vague");
    }

    #[test]
    fn with_names_invites_them() {
        let people = |n: &str| match n.to_lowercase().as_str() {
            "poppy" => vec!["sunny-otter-42".to_string()],
            "sam" => vec!["plucky-wren-17".to_string()],
            "lu" => vec!["calm-heron-3".to_string()],
            "twin" => vec!["a".to_string(), "b".to_string()],
            _ => vec![],
        };
        assert_eq!(
            invited("Standup with Poppy, Sam and @Lu", people),
            Some(Ok(("Standup".to_string(), vec!["sunny-otter-42".to_string(), "plucky-wren-17".to_string(), "calm-heron-3".to_string()])))
        );
        assert_eq!(invited("Tea with cake", people), None, "nobody's called cake");
        assert_eq!(invited("Standup", people), None);
        assert_eq!(invited("Walk with Poppy & Bob", people), Some(Err(vec!["Bob".to_string()])));
        assert_eq!(invited("Chat with Twin", people), None, "ambiguous names aren't guessed");
        assert_eq!(invited("Tea with milk with Sam", people), Some(Ok(("Tea with milk".to_string(), vec!["plucky-wren-17".to_string()]))));
    }

    #[test]
    fn here_makes_it_a_place_event() {
        assert_eq!(place_event("here fri 10am Standup"), (true, "fri 10am Standup"));
        assert_eq!(place_event("Here 4pm Tea"), (true, "4pm Tea"));
        assert_eq!(place_event("sat 4pm Beach party"), (false, "sat 4pm Beach party"));
        assert_eq!(place_event("hereford 4pm cattle show").0, false);
    }
}
