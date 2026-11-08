//! Events: keepers schedule things at a place and time ("event sat 4pm Beach
//! party"). They go in the Island Journal, everyone hears a reminder ten
//! minutes before and a shout when they start, and 'go event' walks you there.

use super::Game;
use crate::protocol::{Col, Line};
use crate::server::db::EventRow;
use chrono::{Datelike, Duration, NaiveTime, TimeZone, Weekday};
use chrono_tz::Tz;

const REMIND_BEFORE: i64 = 10 * 60;
const KEEP_AFTER: i64 = 3 * 3600;

/// Read "sat 4pm", "tomorrow 10:30am", "16:00", "noon" from the start of the
/// text. Returns when, and the rest of the text (the event's name).
pub fn parse_when<Z: TimeZone>(text: &str, now: chrono::DateTime<Z>) -> Option<(i64, String)> {
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
    let mut at = now.timezone().from_local_datetime(&date.and_time(time)).earliest()?;
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
        Some(l) => line.c(Col::Cyan, " 🔗 ").c(Col::Cyan, super::meetings::shown(l)),
        None => line,
    }
}

/// "Sat 7 Nov, 4:00pm", in someone's own time.
pub fn when_label(at: i64, tz: Tz) -> String {
    tz.timestamp_opt(at, 0).single().map_or_else(String::new, |t| t.format("%a %-d %b, %-I:%M%P").to_string())
}

/// "Sat 7 Nov, 4:00pm AEDT": for words everyone reads alike (a board's note).
pub fn when_zoned(at: i64, tz: Tz) -> String {
    tz.timestamp_opt(at, 0).single().map_or_else(String::new, |t| t.format("%a %-d %b, %-I:%M%P %Z").to_string())
}

/// A repeating event moves on to its next time this long after it starts.
const ROLL_AFTER: i64 = 3600;

const DAY_NAMES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// "weekdays 9:20am Standup" → (the days it repeats on, "9:20am Standup").
/// Also weekends, daily / every day, and every mon wed fri (or mondays).
pub fn repeat_rule(text: &str) -> (Option<Vec<Weekday>>, String) {
    let words: Vec<&str> = text.split_whitespace().collect();
    let lower: Vec<String> = words.iter().map(|w| w.to_lowercase().trim_end_matches(',').to_string()).collect();
    let day = |w: &str| -> Option<Weekday> {
        let w = w.trim_end_matches('s');
        let i = DAY_NAMES.iter().position(|d| w.starts_with(d) && (w.len() == 3 || "monday tuesday wednesday thursday friday saturday sunday tues weds thur thurs".split(' ').any(|full| full == w)))?;
        Weekday::try_from(i as u8).ok()
    };
    let all = || (0..7u8).filter_map(|i| Weekday::try_from(i).ok()).collect::<Vec<_>>();
    let rest = |n: usize| words[n.min(words.len())..].join(" ");
    let first = lower.first().map(String::as_str).unwrap_or("");
    let second = lower.get(1).map(String::as_str).unwrap_or("");
    match (first, second) {
        ("weekdays" | "weekday", _) => return (Some(all()[..5].to_vec()), rest(1)),
        ("every", "weekday" | "weekdays") => return (Some(all()[..5].to_vec()), rest(2)),
        ("weekends" | "weekend", _) => return (Some(all()[5..].to_vec()), rest(1)),
        ("every", "weekend") => return (Some(all()[5..].to_vec()), rest(2)),
        ("daily" | "everyday", _) => return (Some(all()), rest(1)),
        ("every", "day") => return (Some(all()), rest(2)),
        _ => {}
    }
    // "every mon wed and fri", or just "mondays".
    let (start, needs_every) = if first == "every" { (1, true) } else { (0, false) };
    let mut days = Vec::new();
    let mut n = start;
    while n < lower.len() {
        let w = lower[n].as_str();
        if w == "and" || w == "&" {
            n += 1;
            continue;
        }
        // Without "every", only plurals ("mondays") mean repeating.
        let plural = w.ends_with('s') && !w.ends_with("ss");
        match day(w) {
            Some(d) if needs_every || plural => {
                if !days.contains(&d) {
                    days.push(d);
                }
                n += 1;
            }
            _ => break,
        }
    }
    if days.is_empty() {
        return (None, text.to_string());
    }
    days.sort_by_key(|d| d.num_days_from_monday());
    (Some(days), rest(n))
}

/// "mon,tue,wed,thu,fri" for keeping.
fn days_code(days: &[Weekday]) -> String {
    days.iter().map(|d| DAY_NAMES[d.num_days_from_monday() as usize]).collect::<Vec<_>>().join(",")
}

fn days_from_code(code: &str) -> Vec<Weekday> {
    code.split(',').filter_map(|c| DAY_NAMES.iter().position(|d| *d == c)).filter_map(|i| Weekday::try_from(i as u8).ok()).collect()
}

/// "weekdays", "every day", "every mon, wed and fri"
pub fn days_label(code: &str) -> String {
    let days = days_from_code(code);
    match code {
        "mon,tue,wed,thu,fri" => "weekdays".into(),
        "sat,sun" => "weekends".into(),
        "mon,tue,wed,thu,fri,sat,sun" => "every day".into(),
        _ => format!("every {}", super::super::world::join_and(&days.iter().map(|d| DAY_NAMES[d.num_days_from_monday() as usize]).collect::<Vec<_>>())),
    }
}

/// The next time after `after` that falls on one of `days`, at `time` of day
/// where `tz` is.
pub fn next_at(days: &[Weekday], time: NaiveTime, tz: Tz, after: i64) -> Option<i64> {
    let from = tz.timestamp_opt(after, 0).single()?.date_naive();
    (0..9).find_map(|ahead| {
        let date = from + Duration::days(ahead);
        if !days.contains(&date.weekday()) {
            return None;
        }
        let at = tz.from_local_datetime(&date.and_time(time)).earliest()?.timestamp();
        (at > after).then_some(at)
    })
}

/// "Poppy, Sam and @Lu" → ["Poppy", "Sam", "Lu"].
fn split_names(who: &str) -> Vec<String> {
    who.split([',', '&']).flat_map(|p| p.split(" and ")).map(|n| n.trim().trim_start_matches('@').trim().to_string()).filter(|n| !n.is_empty()).collect()
}

/// "Poppy, Sam to event 2" → ("Poppy, Sam", "2"); the word is "to" or "from".
pub fn names_and_event<'a>(rest: &'a str, word: &str) -> Option<(&'a str, &'a str)> {
    let lower = rest.to_lowercase();
    let marker = format!(" {word} event");
    let at = lower.rfind(&marker)?;
    Some((rest[..at].trim(), rest[at + marker.len()..].trim()))
}

/// How a repeating event's next time reads: "🔁 weekdays · next Fri 6 Nov, 9:20am".
fn repeat_when(e: &EventRow, tz: Tz) -> Option<String> {
    let code = e.repeat.as_deref()?;
    Some(format!("🔁 {} · next {}", days_label(code), when_label(e.at, tz)))
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

/// An event's name without its guests, and their usernames.
pub type Invited = (String, Vec<String>);

/// "Standup with Poppy, Sam and Lu" → ("Standup", their usernames), when
/// every name is someone; "Tea with cake" (nobody's called cake) stays as it
/// is (None); some names but not others: Err(the unknown ones).
pub fn invited(name: &str, lookup: impl Fn(&str) -> Vec<String>) -> Option<Result<Invited, Vec<String>>> {
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

/// Which of `events` someone means by `which` (see `Game::pick_event`).
fn pick(events: Vec<EventRow>, which: &str) -> Option<EventRow> {
    let w = which.trim().trim_start_matches("to").trim().trim_start_matches("event").trim().to_lowercase();
    if w.is_empty() {
        return if events.len() == 1 { events.into_iter().next() } else { None };
    }
    if let Ok(n) = w.parse::<usize>() {
        return events.into_iter().nth(n.checked_sub(1)?);
    }
    let mut named: Vec<EventRow> = events.into_iter().filter(|e| e.name.to_lowercase().contains(&w)).collect();
    if named.len() == 1 { named.pop() } else { None }
}

impl Game {
    /// Who hears about an event: everyone online, or for a place event its
    /// host, whoever's going, followers of a noticeboard on that spot, and
    /// anyone nearby.
    fn audience(&self, e: &EventRow) -> Vec<u64> {
        let insiders = self.insiders(e);
        // Skipping this one: only if they happen to be standing nearby.
        let skipping = self.world.db.skipping(e.id, e.at).unwrap_or_default();
        self.players
            .iter()
            .filter(|(_, p)| self.in_on(e, &insiders, &p.user, p.x, p.y))
            .filter(|(_, p)| !skipping.contains(&p.user) || self.in_on(e, &[], "", p.x, p.y))
            .map(|(&id, _)| id)
            .collect()
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

    /// Tell everyone who's in on an event, each in their own time.
    fn tell_audience(&self, e: &EventRow, except: Option<u64>, line: impl Fn(Tz) -> Line) {
        for id in self.audience(e) {
            if Some(id) != except
                && let Some(p) = self.players.get(&id) {
                    self.say1(id, line(p.tz));
                }
        }
    }

    fn my_zone(&self, id: u64) -> Tz {
        self.players.get(&id).map_or(chrono_tz::UTC, |p| p.tz)
    }

    pub(super) fn cmd_event(&mut self, id: u64, text: &str) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        let host = self.user(id);
        let (local, text) = place_event(text);
        let (repeat, text) = repeat_rule(text);
        let text = text.as_str();
        if !local && !self.is_keeper(&host) {
            self.say1(id, Line::plain("Only island keepers can plan events for the whole island. Anyone can plan one just for here, though: event here fri 4pm Tea party"));
            return Ok(());
        }
        let (text, link) = super::meetings::split_link(text);
        let link = link.or_else(|| self.meeting_link_at(x, y));
        // Times are the planner's own: 2pm is 2pm where they are.
        let tz = self.my_zone(id);
        let now_there = chrono::Utc::now().with_timezone(&tz);
        let Some((at, name)) = parse_when(&text, now_there).map(|(at, name)| {
            // Repeating: the same time of day, on the first of its days to come.
            let next = repeat.as_ref().and_then(|days| {
                let time = tz.timestamp_opt(at, 0).single()?.time();
                next_at(days, time, tz, now_there.timestamp())
            });
            (next.unwrap_or(at), name)
        }) else {
            // A time earlier today reads fine from midnight: it's just gone.
            let midnight = now_there.date_naive().and_hms_opt(0, 0, 0).and_then(|m| tz.from_local_datetime(&m).earliest());
            if let Some((at, _)) = midnight.and_then(|m| parse_when(&text, m)) {
                self.say1(id, Line::plain(format!("{} has already gone by (it's {} where you are). Try a later time, or tomorrow.", when_label(at, tz), now_there.format("%-I:%M%P"))));
                return Ok(());
            }
            self.say1(id, Line::plain("When, and what? e.g. 'event sat 4pm Beach party', or just for this place: 'event here fri 4pm Tea party'. It happens right here."));
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
        if let Some(days) = &repeat {
            self.world.db.set_repeat(event, &days_code(days), tz.name())?;
        }
        let every = repeat.as_ref().map(|d| days_label(&days_code(d)));
        let when = match &every {
            Some(every) => format!("🔁 {every} · next {}", when_label(at, tz)),
            None => when_label(at, tz),
        };
        let call = link.as_ref().map_or(String::new(), |l| format!(" · 🔗 {}", super::meetings::shown(l)));
        let now = super::boards::now();
        let who = if local {
            self.world.db.set_going(event, &host, true)?;
            // Say who'll actually hear, so the group is easy to picture.
            match self.board_here(x, y) {
                Some((board, false)) => {
                    let n = self.world.db.followers(board)?.into_iter().filter(|u| *u != host).count();
                    match n {
                        0 => "Nobody follows the board here yet, so for now it's whoever's going and anyone nearby. (add Poppy to board)".to_string(),
                        1 => "The 1 other person following the board here, whoever's going, and anyone nearby will get a reminder.".to_string(),
                        n => format!("The {n} people following the board here, whoever's going, and anyone nearby will get a reminder."),
                    }
                }
                _ => {
                    // Its number, for adding people to it.
                    let n = self.world.db.upcoming_events(super::boards::now() - KEEP_AFTER)?.iter().position(|e| e.id == event).map_or(1, |i| i + 1);
                    if invited.is_empty() {
                        format!("Bring your people and they'll hear about it from anywhere: add Poppy, Sam to event {n}")
                    } else {
                        format!("Whoever's going and anyone nearby will get a reminder. (add more: add Poppy to event {n})")
                    }
                }
            }
        } else {
            "Everyone will get a reminder.".to_string()
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
            // Everyone reads a note alike, so it names the zone.
            let when = match &every {
                Some(every) => format!("🔁 {every} · next {}", when_zoned(at, tz)),
                None => when_zoned(at, tz),
            };
            self.world.db.add_note(board, &host, &format!("📅 {name} · {when} at {place}{call}"), now, &super::boards::week_key(now), None)?;
        }
        let row = self.world.db.upcoming_events(at)?.into_iter().find(|e| e.id == event);
        let invite = if local { " (going to say you'll come)" } else { "" };
        let when_for = |tz: Tz| match &every {
            Some(every) => format!("🔁 {every} · next {}", when_label(at, tz)),
            None => when_label(at, tz),
        };
        let line = |tz: Tz| Line::new().c(Col::Yellow, "📅 Coming up: ").b(Col::Yellow, &name).c(Col::Yellow, format!(" · {} at {place}{call}", when_for(tz))).c(Col::Dim, invite);
        if let Some(e) = row {
            // The invited get their invite (below) instead.
            for pid in self.audience(&e) {
                if let Some(p) = self.players.get(&pid).filter(|p| pid != id && !invited.contains(&p.user)) {
                    self.say1(pid, line(p.tz));
                }
            }
            let invited: Vec<String> = invited.into_iter().filter(|u| *u != host).collect();
            self.send_invites(&host, &e, &invited)?;
        }
        Ok(())
    }

    /// Tell people they've been invited to an event: straight away if
    /// they're here, or a note waiting for when they arrive.
    fn send_invites(&mut self, from: &str, e: &EventRow, users: &[String]) -> anyhow::Result<()> {
        let place = self.world.place_title(e.x, e.y);
        let call = e.link.as_ref().map_or(String::new(), |l| format!(" · 🔗 {}", super::meetings::shown(l)));
        let fromname = self.world.nick(from);
        let when = |tz: Tz, zoned: bool| {
            let at = if zoned { when_zoned(e.at, tz) } else { when_label(e.at, tz) };
            match &e.repeat {
                Some(code) => format!("🔁 {} · next {at}", days_label(code)),
                None => at,
            }
        };
        let now = super::boards::now();
        for u in users {
            match self.players.iter().find(|(_, p)| p.user == *u) {
                Some((&pid, p)) => {
                    let line = Line::new()
                        .c(Col::Yellow, format!("📅 {fromname} has invited you to "))
                        .b(Col::Yellow, &e.name)
                        .c(Col::Yellow, format!(" · {} at {place}{call}", when(p.tz, false)))
                        .c(Col::Dim, " (events lists it; not going drops out)");
                    self.say1(pid, line);
                }
                None => {
                    let note = format!("📅 invited you to {} · {} at {place}{call} (events lists it; not going drops out)", e.name, when(self.zone_of(u), true));
                    self.world.db.send_mail(u, from, &note, now)?;
                }
            }
        }
        Ok(())
    }

    /// `add Poppy, Sam to event 2`: sign people up to a place event after
    /// it's planned. Its host, a keeper, or anyone already going can.
    pub(super) fn cmd_add_to_event(&mut self, id: u64, who: &str, which: &str) -> anyhow::Result<()> {
        let Some(e) = self.pick_event(which)? else {
            self.say1(id, Line::plain("Add them to which event? 'events' lists them, then e.g. 'add Poppy, Sam to event 2'."));
            return Ok(());
        };
        if !e.local {
            self.say1(id, Line::plain(format!("{} is for the whole island: everyone hears about it anyway.", e.name)));
            return Ok(());
        }
        let me = self.user(id);
        let going = self.world.db.going(e.id)?;
        if e.host != me && !going.contains(&me) && !self.is_keeper(&me) {
            self.say1(id, Line::plain(format!("Only {}'s host, or someone going, can add people to it.", e.name)));
            return Ok(());
        }
        let mut added = Vec::new();
        let mut already = Vec::new();
        for n in split_names(who) {
            let found = self.world.db.matching_users(&n)?;
            let [them] = found.as_slice() else {
                let why = if found.is_empty() { "there's nobody called" } else { "more than one person could be" };
                self.say1(id, Line::plain(format!("Skipping {n}: {why} {n}. (Usernames from 'users' always work.)")));
                continue;
            };
            if self.world.db.set_going(e.id, them, true)? {
                added.push(them.clone());
            } else {
                already.push(self.world.nick(them));
            }
        }
        if !added.is_empty() {
            let names: Vec<String> = added.iter().map(|u| self.world.nick(u)).collect();
            let verb = if names.len() == 1 { "is" } else { "are" };
            self.say1(id, Line::col(Col::Yellow, format!("📅 {} {verb} going to {} now, and will hear about it wherever they are.", super::super::world::join_and(&names), e.name)));
            self.send_invites(&me, &e, &added)?;
        }
        if !already.is_empty() {
            let verb = if already.len() == 1 { "was" } else { "were" };
            self.say1(id, Line::col(Col::Dim, format!("   {} {verb} going already.", super::super::world::join_and(&already))));
        }
        Ok(())
    }

    /// `remove Poppy from event 2`: for its host or a keeper (anyone can
    /// drop themselves with `not going`).
    pub(super) fn cmd_remove_from_event(&mut self, id: u64, who: &str, which: &str) -> anyhow::Result<()> {
        let Some(e) = self.pick_event(which)? else {
            self.say1(id, Line::plain("Remove them from which event? 'events' lists them, then e.g. 'remove Poppy from event 2'."));
            return Ok(());
        };
        let me = self.user(id);
        if e.host != me && !self.is_keeper(&me) {
            self.say1(id, Line::plain(format!("Only {}'s host or a keeper can take people off it. (not going takes yourself off)", e.name)));
            return Ok(());
        }
        let mut gone = Vec::new();
        for n in split_names(who) {
            let Some(them) = self.world.db.resolve_user(&n) else {
                self.say1(id, Line::plain(format!("There's nobody called {n}.")));
                continue;
            };
            if them == e.host {
                self.say1(id, Line::plain(format!("{} is hosting it, so they stay. (cancel event stops it)", self.world.nick(&them))));
                continue;
            }
            if self.world.db.set_going(e.id, &them, false)? {
                gone.push(self.world.nick(&them));
                if let Some((&pid, _)) = self.players.iter().find(|(_, p)| p.user == them) {
                    self.say1(pid, Line::col(Col::Dim, format!("📅 You're not on the list for {} any more.", e.name)));
                }
            } else {
                self.say1(id, Line::col(Col::Dim, format!("{} wasn't going anyway.", self.world.nick(&them))));
            }
        }
        if !gone.is_empty() {
            self.say1(id, Line::col(Col::Yellow, format!("📅 {} won't hear about {} now (unless they follow a board there, or wander by).", super::super::world::join_and(&gone), e.name)));
        }
        Ok(())
    }

    pub(super) fn cmd_events(&mut self, id: u64) -> anyhow::Result<()> {
        let events = self.world.db.upcoming_events(super::boards::now() - KEEP_AFTER)?;
        let me = self.user(id);
        let (mx, my) = self.players.get(&id).map_or((-1, -1), |p| (p.x, p.y));
        let tz = self.my_zone(id);
        let now_there = chrono::Utc::now().with_timezone(&tz);
        let mut lines = vec![
            Line::new().b(Col::Magenta, "📅 Coming up on the island"),
            Line::col(Col::Dim, format!("  It's {} where you are ({}). Times here are yours.", now_there.format("%a %-d %b, %-I:%M%P"), now_there.format("%Z"))),
        ];
        if events.is_empty() {
            lines.push(Line::col(Col::Dim, "  Nothing planned yet. Plan one just for here: event here fri 4pm Tea party"));
        }
        for (i, e) in events.iter().enumerate() {
            let place = self.world.place_title(e.x, e.y);
            let now = super::boards::now();
            let when = if e.at <= now { "happening now!".to_string() } else { repeat_when(e, self.my_zone(id)).unwrap_or_else(|| when_label(e.at, self.my_zone(id))) };
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
                    lines.push(Line::new().c(Col::Cyan, "     🔗 ").c(Col::Cyan, super::meetings::shown(l)));
                } else {
                    lines.push(Line::col(Col::Dim, "     🔗 has links, for whoever's going"));
                }
            }
            if e.local {
                let going = self.world.db.going(e.id)?;
                let skipping = self.world.db.skipping(e.id, e.at)?;
                let nick = |u: &String| if *u == me { "you".to_string() } else { self.world.nick(u) };
                let names: Vec<String> = going.iter().filter(|u| !skipping.contains(u)).map(nick).collect();
                let verb = if names.len() == 1 && names[0] != "you" { "is" } else { "are" };
                let mut text = if names.is_empty() { "nobody's said they're going yet".to_string() } else { format!("{} {verb} going", super::super::world::join_and(&names)) };
                let skips: Vec<String> = going.iter().filter(|u| skipping.contains(u)).map(nick).collect();
                if !skips.is_empty() {
                    let verb = if skips.len() == 1 && skips[0] != "you" { "is" } else { "are" };
                    text.push_str(&format!(" ({} {verb} skipping the next one)", super::super::world::join_and(&skips)));
                }
                lines.push(Line::col(Col::Dim, format!("     {text}")));
            }
        }
        if !events.is_empty() {
            lines.push(Line::col(Col::Dim, "  go event 2 walks you there · going 2 / not going 2 for 📍 events just for one place"));
        }
        self.send(id, lines);
        Ok(())
    }

    /// The event someone means: its number from `events` ("2", "event 2"),
    /// a name only one event has ("Tea party"), or, when there's only one
    /// event, nothing at all. Anything else is no event, never a guess.
    fn pick_event(&self, which: &str) -> anyhow::Result<Option<EventRow>> {
        let events = self.world.db.upcoming_events(super::boards::now() - KEEP_AFTER)?;
        Ok(pick(events, which))
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
        let place = self.world.place_title(e.x, e.y);
        // A repeating event: not going means just the next one.
        if e.repeat.is_some() {
            let on = self.world.db.going(e.id)?.contains(&me);
            let next = when_label(e.at, self.my_zone(id));
            if !going && on {
                self.world.db.set_skip(e.id, &me, e.at, true)?;
                self.say1(id, Line::col(Col::Yellow, format!("📅 You'll skip the next {} ({next}). You're still on for the ones after. (going {which} if you change your mind; leave event {which} to drop out for good)", e.name, which = which.trim())));
                return Ok(());
            }
            if going && self.world.db.set_skip(e.id, &me, e.at, false)? {
                self.say1(id, Line::col(Col::Yellow, format!("📅 You're back on for the next {} ({next}).", e.name)));
                return Ok(());
            }
        }
        let changed = self.world.db.set_going(e.id, &me, going)?;
        let msg = match (going, changed) {
            (true, true) => format!(
                "📅 You're going to {} ({} at {place}{}). You'll get a reminder wherever you are.",
                e.name,
                when_label(e.at, self.my_zone(id)),
                e.link.as_ref().map_or(String::new(), |l| format!(" · 🔗 {}", super::meetings::shown(l)))
            ),
            (true, false) => format!("📅 You're already going to {}.", e.name),
            (false, true) => format!("📅 You're not going to {} any more.", e.name),
            (false, false) => format!("📅 You weren't going to {} anyway.", e.name),
        };
        self.say1(id, Line::col(Col::Yellow, msg));
        if going && changed && e.host != me
            && let Some((&host, _)) = self.players.iter().find(|(_, p)| p.user == e.host) {
                let name = self.world.nick(&me);
                self.say1(host, Line::col(Col::Yellow, format!("📅 {name} is going to {}.", e.name)));
            }
        Ok(())
    }

    /// `leave event 2`: drop out of a repeating event for good.
    pub(super) fn cmd_leave_event(&mut self, id: u64, which: &str) -> anyhow::Result<()> {
        let Some(e) = self.pick_event(which)? else {
            self.say1(id, Line::plain("Leave which? 'events' lists them, then e.g. 'leave event 2'."));
            return Ok(());
        };
        let me = self.user(id);
        if e.host == me {
            self.say1(id, Line::plain(format!("You're hosting {}: cancel event {} stops it for everyone.", e.name, which.trim())));
            return Ok(());
        }
        let msg = if self.world.db.set_going(e.id, &me, false)? {
            let every = if e.repeat.is_some() { " (every one of them)" } else { "" };
            format!("📅 You're not going to {}{every} any more.", e.name)
        } else {
            format!("📅 You weren't going to {} anyway.", e.name)
        };
        self.say1(id, Line::col(Col::Yellow, msg));
        Ok(())
    }

    pub(super) fn cmd_go_event(&mut self, id: u64, which: &str) -> anyhow::Result<()> {
        let Some(e) = self.pick_event(which)? else {
            self.say1(id, Line::plain("There's nothing coming up to go to. ('events' lists them)"));
            return Ok(());
        };
        self.step_out_first(id)?;
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
        self.tell_audience(&e, None, |_| Line::col(Col::Dim, format!("📅 {} has been cancelled.", e.name)));
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
        // Repeating events move on to their next time once they're over
        // (including ones missed while the world was asleep).
        for e in self.world.db.upcoming_events(0).unwrap_or_default() {
            let (Some(code), true) = (e.repeat.as_deref(), e.at < now - ROLL_AFTER) else { continue };
            let tz: Tz = e.tz.as_deref().and_then(|t| t.parse().ok()).unwrap_or(chrono_tz::UTC);
            let next = tz.timestamp_opt(e.at, 0).single().and_then(|t| next_at(&days_from_code(code), t.time(), tz, now));
            match next {
                Some(at) => {
                    let _ = self.world.db.reschedule_event(e.id, at);
                }
                None => {
                    let _ = self.world.db.remove_event(e.id);
                }
            }
        }
        let Ok(events) = self.world.db.upcoming_events(now - KEEP_AFTER) else { return };
        for e in events {
            let place = self.world.place_title(e.x, e.y);
            if e.at < now - ROLL_AFTER && e.repeat.is_some() {
                continue;
            }
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
            self.tell_audience(&e, None, |_| line.clone());
        }
        let _ = self.world.db.forget_events_before(now - KEEP_AFTER);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: i64, name: &str) -> EventRow {
        EventRow { id, at: 0, name: name.into(), x: 0, y: 0, host: String::new(), reminded: false, started: false, link: None, local: true, repeat: None, tz: None }
    }

    #[test]
    fn events_are_picked_by_number_or_name_and_never_guessed() {
        let all = || vec![ev(1, "Sunrise stretch"), ev(2, "Tea party"), ev(3, "Tea tasting")];
        assert_eq!(pick(all(), "2").map(|e| e.id), Some(2));
        assert_eq!(pick(all(), "event 3").map(|e| e.id), Some(3));
        assert_eq!(pick(all(), "tea party").map(|e| e.id), Some(2), "a name only one has");
        assert!(pick(all(), "tea").is_none(), "two events have that in their name");
        assert!(pick(all(), "Tae party").is_none(), "a typo isn't event 1");
        assert!(pick(all(), "").is_none(), "no number, several events");
        assert!(pick(all(), "9").is_none() && pick(all(), "0").is_none());
        assert_eq!(pick(vec![ev(7, "Picnic")], "").map(|e| e.id), Some(7), "the only one");
    }

    #[test]
    fn reads_friendly_times() {
        // A Wednesday morning.
        let now = chrono_tz::Australia::Sydney.with_ymd_and_hms(2026, 11, 4, 9, 0, 0).unwrap();
        let sydney = chrono_tz::Australia::Sydney;
        let (at, name) = parse_when("sat 4pm Beach party", now).unwrap();
        let t = sydney.timestamp_opt(at, 0).unwrap();
        assert_eq!((t.weekday(), t.format("%H:%M").to_string(), name.as_str()), (Weekday::Sat, "16:00".into(), "Beach party"));
        let (at, _) = parse_when("8am Breakfast", now).unwrap();
        assert_eq!(sydney.timestamp_opt(at, 0).unwrap().day(), 5, "8am has passed today, so tomorrow");
        let (at, _) = parse_when("tomorrow 10:30am Treasure hunt", now).unwrap();
        assert_eq!(sydney.timestamp_opt(at, 0).unwrap().format("%d %H:%M").to_string(), "05 10:30");
        assert!(parse_when("sat Beach party", now).is_none(), "needs a time");
        assert!(parse_when("4pm", now).is_none(), "needs a name");
        assert!(parse_when("4 Beach", now).is_none(), "a bare number is too vague");
    }

    #[test]
    fn times_are_the_planners_own_and_shown_in_yours() {
        use chrono_tz::{Australia::Sydney, Europe::London};
        // 10:52am Thursday in Sydney is still Wednesday night in UTC.
        let sydney = Sydney.with_ymd_and_hms(2026, 11, 5, 10, 52, 0).unwrap();
        let (at, name) = parse_when("today 2pm 1:1 with Corey", sydney).expect("2pm today hasn't happened yet in Sydney");
        assert_eq!(name, "1:1 with Corey");
        assert_eq!(when_label(at, Sydney), "Thu 5 Nov, 2:00pm");
        assert_eq!(when_label(at, London), "Thu 5 Nov, 3:00am");
        assert_eq!(when_zoned(at, Sydney), "Thu 5 Nov, 2:00pm AEDT");
        assert!(parse_when("today 9am Standup", sydney).is_none(), "9am has gone by");
    }

    #[test]
    fn reads_how_things_repeat() {
        use Weekday::*;
        let rule = |t: &str| repeat_rule(t);
        assert_eq!(rule("weekdays 9:20am Standup"), (Some(vec![Mon, Tue, Wed, Thu, Fri]), "9:20am Standup".to_string()));
        assert_eq!(rule("every weekday 9:20am Standup").0.map(|d| d.len()), Some(5));
        assert_eq!(rule("daily 8am Tea").0.map(|d| d.len()), Some(7));
        assert_eq!(rule("every day 8am Tea").1, "8am Tea");
        assert_eq!(rule("weekends 10am Brunch").0, Some(vec![Sat, Sun]));
        assert_eq!(rule("every mon, wed and fri 9am Gym"), (Some(vec![Mon, Wed, Fri]), "9am Gym".to_string()));
        assert_eq!(rule("every Tuesday 4pm Retro"), (Some(vec![Tue]), "4pm Retro".to_string()));
        assert_eq!(rule("fridays 5pm Drinks"), (Some(vec![Fri]), "5pm Drinks".to_string()));
        assert_eq!(rule("fri 5pm Drinks").0, None, "one friday, not every");
        assert_eq!(rule("every 5pm Drinks").0, None);
        assert_eq!(rule("tomorrow 9am Standup").0, None);
        assert_eq!(days_label("mon,tue,wed,thu,fri"), "weekdays");
        assert_eq!(days_label("mon,wed,fri"), "every mon, wed, and fri");
        assert_eq!(days_label("tue"), "every tue");
    }

    #[test]
    fn repeats_at_the_same_time_of_day_on_the_right_days() {
        use chrono_tz::{Australia::Brisbane, Australia::Sydney, Europe::London};
        let weekdays = repeat_rule("weekdays x").0.unwrap();
        let t = NaiveTime::from_hms_opt(9, 20, 0).unwrap();
        // Thursday 10:52am in Brisbane: today's has gone, so Friday's.
        let thu = Brisbane.with_ymd_and_hms(2026, 11, 5, 10, 52, 0).unwrap().timestamp();
        let next = next_at(&weekdays, t, Brisbane, thu).unwrap();
        assert_eq!(when_label(next, Brisbane), "Fri 6 Nov, 9:20am");
        // After Friday's, it skips the weekend to Monday.
        let after = next_at(&weekdays, t, Brisbane, next + 3600).unwrap();
        assert_eq!(when_label(after, Brisbane), "Mon 9 Nov, 9:20am");
        // Seen from London, it's the evening before.
        assert_eq!(when_label(next, London), "Thu 5 Nov, 11:20pm");
        // Across Sydney's clocks going back (5 April 2026), still 9:20am there.
        let before = Sydney.with_ymd_and_hms(2026, 4, 3, 10, 0, 0).unwrap().timestamp();
        let n = next_at(&weekdays, t, Sydney, before).unwrap();
        assert_eq!(when_label(n, Sydney), "Mon 6 Apr, 9:20am");
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
    fn reads_who_to_add_to_which_event() {
        assert_eq!(names_and_event("Poppy, Sam to event 2", "to"), Some(("Poppy, Sam", "2")));
        assert_eq!(names_and_event("Lu to event", "to"), Some(("Lu", "")));
        assert_eq!(names_and_event("Poppy from Event 3", "from"), Some(("Poppy", "3")));
        assert_eq!(names_and_event("Poppy to board", "to"), None);
        assert_eq!(split_names("Poppy, Sam and @Lu & Jo"), vec!["Poppy", "Sam", "Lu", "Jo"]);
    }

    #[test]
    fn here_makes_it_a_place_event() {
        assert_eq!(place_event("here fri 10am Standup"), (true, "fri 10am Standup"));
        assert_eq!(place_event("Here 4pm Tea"), (true, "4pm Tea"));
        assert_eq!(place_event("sat 4pm Beach party"), (false, "sat 4pm Beach party"));
        assert!(!place_event("hereford 4pm cattle show").0);
    }
}
