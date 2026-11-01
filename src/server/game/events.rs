//! Events: keepers schedule things at a place and time ("event sat 4pm Beach
//! party"). They go in the Island Journal, everyone hears a reminder ten
//! minutes before and a shout when they start, and 'go event' walks you there.

use super::Game;
use crate::protocol::{Col, Line, ToClient};
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

/// "Sat 7 Nov, 4:00pm"
pub fn when_label(at: i64) -> String {
    let t = Local.timestamp_opt(at, 0).single().unwrap_or_else(Local::now);
    t.format("%a %-d %b, %-I:%M%P").to_string()
}

impl Game {
    pub(super) fn cmd_event(&mut self, id: u64, text: &str) -> anyhow::Result<()> {
        let Some((x, y, _)) = self.here(id) else { return Ok(()) };
        let host = self.user(id);
        if !self.is_keeper(&host) {
            self.say1(id, Line::plain("Only island keepers can plan events. Ask one! ('events' shows what's coming up)"));
            return Ok(());
        }
        let Some((at, name)) = parse_when(text, Local::now()) else {
            self.say1(id, Line::plain("When, and what? e.g. 'event sat 4pm Beach party' or 'event tomorrow 10:30am Treasure hunt'. It happens right here."));
            return Ok(());
        };
        let name: String = name.chars().take(80).collect();
        let place = self.world.place_title(x, y);
        self.world.db.add_event(at, &name, x, y, &host)?;
        let when = when_label(at);
        self.say1(id, Line::new().c(Col::Yellow, "📅 Planned: ").b(Col::Yellow, &name).c(Col::Yellow, format!(" · {when} at {place}. Everyone will get a reminder.")));
        // On the Island Journal too, so people who aren't here now see it.
        if let Some(board) = self.journal_board {
            let now = super::boards::now();
            self.world.db.add_note(board, &host, &format!("📅 {name} · {when} at {place}"), now, &super::boards::week_key(now), None)?;
        }
        let line = Line::new().c(Col::Yellow, "📅 Coming up: ").b(Col::Yellow, &name).c(Col::Yellow, format!(" · {when} at {place}"));
        for (&pid, p) in &self.players {
            if pid != id {
                let _ = p.tx.send(ToClient::Lines { lines: vec![line.clone()] });
            }
        }
        Ok(())
    }

    pub(super) fn cmd_events(&mut self, id: u64) -> anyhow::Result<()> {
        let events = self.world.db.upcoming_events(super::boards::now() - KEEP_AFTER)?;
        let mut lines = vec![Line::new().b(Col::Magenta, "📅 Coming up on the island")];
        if events.is_empty() {
            lines.push(Line::col(Col::Dim, "  Nothing planned yet. Keepers can plan one: event sat 4pm Beach party"));
        }
        for (i, e) in events.iter().enumerate() {
            let place = self.world.place_title(e.x, e.y);
            let now = super::boards::now();
            let when = if e.at <= now { "happening now!".to_string() } else { when_label(e.at) };
            lines.push(
                Line::new()
                    .c(Col::Yellow, format!("  {}. ", i + 1))
                    .b(Col::Yellow, e.name.clone())
                    .t(format!(" · {when} at {place}"))
                    .c(Col::Dim, format!(" (hosted by {})", self.world.nick(&e.host))),
            );
        }
        if !events.is_empty() {
            lines.push(Line::col(Col::Dim, "  go event (or go event 2) walks you there"));
        }
        self.send(id, lines);
        Ok(())
    }

    fn pick_event(&self, which: &str) -> anyhow::Result<Option<super::super::db::EventRow>> {
        let events = self.world.db.upcoming_events(super::boards::now() - KEEP_AFTER)?;
        let n: usize = which.trim().parse().unwrap_or(1);
        Ok(events.into_iter().nth(n.saturating_sub(1)))
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
        let which = which.trim().trim_start_matches("event").trim();
        let Some(e) = self.pick_event(which)? else {
            self.say1(id, Line::plain("Cancel which? 'events' lists them, then e.g. 'cancel event 2'."));
            return Ok(());
        };
        let me = self.user(id);
        if e.host != me && !self.is_keeper(&me) {
            self.say1(id, Line::plain("Only its host or a keeper can cancel that."));
            return Ok(());
        }
        self.world.db.remove_event(e.id)?;
        let line = Line::col(Col::Dim, format!("📅 {} has been cancelled.", e.name));
        for p in self.players.values() {
            let _ = p.tx.send(ToClient::Lines { lines: vec![line.clone()] });
        }
        Ok(())
    }

    /// Reminders shortly before, and a shout when things start.
    pub(super) fn events_tick(&mut self) {
        let now = super::boards::now();
        let Ok(events) = self.world.db.upcoming_events(now - KEEP_AFTER) else { return };
        for e in events {
            let place = self.world.place_title(e.x, e.y);
            let line = if !e.started && e.at <= now {
                let _ = self.world.db.mark_event(e.id, true, true);
                Line::new().b(Col::Yellow, format!("🎉 {} is starting now at {place}!", e.name)).c(Col::Dim, " (go event to head there)")
            } else if !e.reminded && e.at - now <= REMIND_BEFORE && e.at > now {
                let _ = self.world.db.mark_event(e.id, true, false);
                let mins = ((e.at - now) / 60).max(1);
                let mins = if mins == 1 { "1 minute".to_string() } else { format!("{mins} minutes") };
                Line::new().c(Col::Yellow, format!("📅 In {mins}: ")).b(Col::Yellow, e.name.clone()).c(Col::Yellow, format!(" at {place}")).c(Col::Dim, " (go event)")
            } else {
                continue;
            };
            for p in self.players.values() {
                let _ = p.tx.send(ToClient::Lines { lines: vec![line.clone()] });
            }
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
}
