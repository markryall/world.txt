//! What travels over the websocket. Colours are named, not ANSI, so any
//! client (terminal, browser, ssh) can map them to whatever it can draw.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Col {
    Plain,
    Dim,
    White,
    Blue,
    DeepBlue,
    Cyan,
    Green,
    DarkGreen,
    Yellow,
    Sand,
    Magenta,
    Pink,
    Red,
    Brown,
    Grey,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub t: String,
    pub c: Col,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub b: bool,
    /// Tapping these words runs this command (a client that can't tap just
    /// shows the words).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// Tapping these words puts this in the command box, ready to finish
    /// ("@Sam " to message Sam).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
}

impl Span {
    fn new(t: impl Into<String>, c: Col, b: bool) -> Self {
        Span {
            t: t.into(),
            c,
            b,
            run: None,
            fill: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Line(pub Vec<Span>);

impl Line {
    pub fn new() -> Self {
        Line(Vec::new())
    }
    /// plain text
    pub fn t(mut self, text: impl Into<String>) -> Self {
        self.0.push(Span::new(text, Col::Plain, false));
        self
    }
    /// coloured text
    pub fn c(mut self, col: Col, text: impl Into<String>) -> Self {
        self.0.push(Span::new(text, col, false));
        self
    }
    /// bold coloured text
    pub fn b(mut self, col: Col, text: impl Into<String>) -> Self {
        self.0.push(Span::new(text, col, true));
        self
    }
    /// Bold coloured words that run `cmd` when tapped.
    pub fn run(mut self, col: Col, text: impl Into<String>, cmd: impl Into<String>) -> Self {
        self.0.push(Span {
            run: Some(cmd.into()),
            ..Span::new(text, col, true)
        });
        self
    }
    /// Coloured words that put `fill` in the command box when tapped, ready
    /// to finish or change.
    pub fn fill(mut self, col: Col, text: impl Into<String>, fill: impl Into<String>) -> Self {
        self.0.push(Span {
            fill: Some(fill.into()),
            ..Span::new(text, col, false)
        });
        self
    }
    /// Someone's name, which sets you up to message them when tapped.
    pub fn who(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        let fill = format!("@{name} ");
        self.0.push(Span {
            fill: Some(fill),
            ..Span::new(name, Col::Cyan, true)
        });
        self
    }
    pub fn plain(text: impl Into<String>) -> Self {
        Line::new().t(text)
    }
    pub fn col(col: Col, text: impl Into<String>) -> Self {
        Line::new().c(col, text)
    }
}

/// Something you could do where you are: tapping it runs a command, or puts
/// one in the box ready to finish.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
}

/// One person's part in a turn of a humming circle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CirclePart {
    pub name: String,
    /// It's your own part.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub you: bool,
    /// Which of their sections is playing, counting from 1, and how many
    /// they have.
    pub section: usize,
    pub of: usize,
    /// The tune to play, tempo first (`@110 D3- A2- …`).
    pub tune: String,
}

/// A turn of a humming circle as one line, for clients without a strip to
/// show it in: "🎶 Poppy · Sam 9/10 · you 8/10".
pub fn circle_line(parts: &[CirclePart]) -> Line {
    let mut line = Line::new().c(Col::Pink, "🎶");
    for (i, p) in parts.iter().enumerate() {
        line = line.c(Col::Dim, if i == 0 { " " } else { " · " });
        line = if p.you {
            line.b(Col::Pink, "you")
        } else {
            line.c(Col::Cyan, &p.name)
        };
        if p.of > 1 {
            line = line.c(Col::Dim, format!(" {}/{}", p.section, p.of));
        }
    }
    line
}

/// Someone else out in the world, for the side panel's list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    /// Standing right here with you.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub here: bool,
    /// An island resident (AI), not a person.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bot: bool,
    /// Their timezone ("Europe/London"), if their device has said, so the
    /// client can show what time it is for them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tz: Option<String>,
}

/// A progress bar an outside tool keeps filling, for the side panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    pub label: String,
    pub done: u64,
    pub total: u64,
    /// Where its board is, to tell apart two bars with the same label.
    #[serde(default)]
    pub place: String,
}

impl Bar {
    /// How far along, 0 to 100.
    pub fn percent(&self) -> u64 {
        (self.done.min(self.total) * 100) / self.total.max(1)
    }
    /// What goes beside it: "37%", or "3/10" when it's counting things.
    pub fn amount(&self) -> String {
        if self.total == 100 {
            format!("{}%", self.done)
        } else {
            format!("{}/{}", self.done, self.total)
        }
    }
}

/// What a spot sounds like, for clients that make sound: each from 0
/// (silent) to 1. The browser makes them all itself (web/index.html,
/// "Sounds of the island"); nothing here is audio.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sounds {
    pub rain: f32,
    pub thunder: f32,
    pub wind: f32,
    pub waves: f32,
    pub birds: f32,
    pub crickets: f32,
    pub fire: f32,
}

/// Something outside that's up or down (a build, a website), for the side panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Light {
    pub label: String,
    pub up: bool,
    /// What it last said, e.g. "flat as a millpond".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Where its board is, to tell apart two lights with the same label.
    #[serde(default)]
    pub place: String,
}

/// Labels shared by more than one of these, which need their place beside them.
pub fn shared_labels<'a>(
    labels: impl Iterator<Item = &'a str>,
) -> std::collections::HashSet<&'a str> {
    let mut seen = std::collections::HashSet::new();
    let mut twice = std::collections::HashSet::new();
    for l in labels {
        if !seen.insert(l) {
            twice.insert(l);
        }
    }
    twice
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: i64,
    pub author: String,
    pub text: String,
    /// Already friendly: "5m ago", "yesterday", "Tue".
    pub when: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Week {
    /// Pass to `read week <key>`.
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardMode {
    /// Freshly opened: replace whatever was showing.
    Open,
    /// Older notes, to go above what's showing.
    Older,
    /// Someone just posted: add below, if this board is showing.
    New,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToClient {
    /// Lines appended to the scrolling log.
    Lines { lines: Vec<Line> },
    /// A turn of the humming circle where you are: every part to play now,
    /// together, and who's humming what. No parts means there's no circle
    /// here any more (it ended, or you've walked away).
    Circle {
        /// How long this turn lasts, in milliseconds.
        ms: u64,
        parts: Vec<CirclePart>,
    },
    /// The world is restarting (a redeploy): reconnect in a moment.
    Restarting,
    /// The side panel: minimap and status.
    Status {
        place: String,
        weather: String,
        clock: String,
        map: Vec<Line>,
        here: Vec<String>,
        /// People (not residents) currently in the world.
        #[serde(default)]
        humans: u32,
        /// Everyone else in the world just now, those right here first.
        #[serde(default)]
        people: Vec<Person>,
        /// Progress bars on noticeboards you follow or are near.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        progress: Vec<Bar>,
        /// Up-or-down lights on noticeboards you follow or are near.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        lights: Vec<Light>,
        /// What it sounds like where you are.
        #[serde(default)]
        sounds: Sounds,
        /// What you could do here, most likely first, for tapping.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        actions: Vec<Action>,
    },
    /// The current first-steps nudge, or none to clear it.
    Hint {
        text: Option<String>,
        /// What trying the hint sends, e.g. "plant flowers".
        cmd: Option<String>,
    },
    /// A noticeboard's notes.
    Board {
        board: i64,
        title: String,
        mode: BoardMode,
        notes: Vec<Note>,
        /// Older notes exist (`read more`).
        more: bool,
        /// e.g. "This week (since Mon 2 Nov)" for the Island Journal.
        page: Option<String>,
        /// Past pages, newest first. Only the Island Journal has these.
        weeks: Vec<Week>,
    },
    /// Signed in. A new device token comes with it when an invite was used:
    /// keep it, and send it next time instead of the invite.
    Welcome {
        token: Option<String>,
        user: String,
        nickname: String,
        /// Which version of the browser page the world serves now, so a page
        /// left open across an update can offer to reload.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        page: Option<String>,
    },
    /// Go to another world (or back home): the browser should open this.
    Travel { url: String },
    /// Connection refused (e.g. name taken).
    Bye { reason: String },
    /// You're playing music on this gramophone: start sending it (browser).
    SpinStart { thing: i64 },
    /// Stop sending music for this gramophone.
    SpinStop { thing: i64 },
    /// Music from a gramophone in earshot: Opus frames (base64), and how loud
    /// it is where you stand (0 to 1).
    Audio {
        thing: i64,
        frames: Vec<String>,
        gain: f32,
        title: Option<String>,
    },
    /// That gramophone has stopped.
    AudioEnd { thing: i64 },
    /// A radio you can hear (url and how loud), or can't any more (no url).
    Radio {
        thing: i64,
        url: Option<String>,
        gain: f32,
    },
    /// Open the song picker for this jukebox.
    JukeAdd { thing: i64 },
    /// Your turn on the jukebox: play this song (by its key) to everyone nearby.
    JukeTurn { thing: i64, key: String },
    /// Stop playing your song on this jukebox (skipped, or it's moved on).
    JukeStop { thing: i64 },
}

/// A song waiting on a jukebox: a key the browser knows it by, and its title.
/// The song itself never leaves the browser until it plays.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub key: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToServer {
    Hello {
        /// Not used to sign in any more (invites and device tokens do that).
        #[serde(default)]
        name: String,
        /// True for AI residents, so the world can tell them from people.
        #[serde(default)]
        bot: bool,
        /// A one-use invite from a magic link.
        #[serde(default)]
        invite: Option<String>,
        /// This device's token, from an earlier invite.
        #[serde(default)]
        token: Option<String>,
        /// A passport from another world, for visiting.
        #[serde(default)]
        passport: Option<String>,
        /// Coming straight back after the connection dropped (a restart, a
        /// phone waking up), so there's no need to announce it.
        #[serde(default)]
        back: bool,
        /// Where in the world this person is, as an IANA zone
        /// ("Australia/Sydney"), so times can be theirs.
        #[serde(default)]
        tz: Option<String>,
    },
    Cmd {
        text: String,
    },
    /// Music for the gramophone this browser is playing: Opus frames (base64),
    /// and what's playing.
    Audio {
        frames: Vec<String>,
        #[serde(default)]
        title: Option<String>,
    },
    /// Songs picked for a jukebox (titles only).
    Queue {
        thing: i64,
        songs: Vec<Song>,
    },
    /// This browser's jukebox song has finished.
    TrackDone {
        thing: i64,
        key: String,
    },
}

/// A board as plain lines, for clients that show it in their log. `None`
/// for live additions, which arrive as ordinary lines too.
pub fn board_lines(msg: &ToClient) -> Option<Vec<Line>> {
    let ToClient::Board {
        title,
        mode,
        notes,
        more,
        page,
        weeks,
        ..
    } = msg
    else {
        return None;
    };
    if *mode == BoardMode::New {
        return None;
    }
    let mut out = Vec::new();
    if *mode == BoardMode::Open {
        out.push(Line::new().b(Col::Yellow, title.clone()));
        if let Some(p) = page {
            out.push(Line::col(Col::Dim, format!("  {p}")));
        }
    }
    if notes.is_empty() {
        out.push(Line::col(
            Col::Dim,
            "  Nothing here yet. Be the first: post hello!",
        ));
    }
    for n in notes {
        out.push(
            Line::new()
                .t("  ")
                .b(Col::Cyan, n.author.clone())
                .c(Col::Dim, format!(" ({}): ", n.when))
                .c(Col::White, n.text.clone()),
        );
    }
    let mut hints = Vec::new();
    if *more {
        hints.push("read more for older notes".to_string());
    }
    if !weeks.is_empty() {
        hints.push("archive for past weeks".to_string());
    }
    hints.push("post <message> to add one".to_string());
    out.push(Line::col(Col::Dim, format!("  ({})", hints.join(" · "))));
    Some(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn taps_travel_only_when_there_are_any() {
        use super::{Col, Line};
        let plain = serde_json::to_string(&Line::col(Col::Pink, "hi")).unwrap();
        assert_eq!(
            plain, r#"[{"t":"hi","c":"pink"}]"#,
            "a plain span is just as it was"
        );
        let line = Line::new()
            .who("Sam")
            .run(Col::Pink, "the canon", "hum along canon tune");
        let json = serde_json::to_string(&line).unwrap();
        assert!(
            json.contains(r#""fill":"@Sam ""#) && json.contains(r#""run":"hum along canon tune""#),
            "{json}"
        );
        let back: Line = serde_json::from_str(r#"[{"t":"hi","c":"pink"}]"#).unwrap();
        assert!(back.0[0].run.is_none() && back.0[0].fill.is_none());
    }

    #[test]
    fn a_circle_turn_reads_as_one_line() {
        use super::{CirclePart, circle_line};
        let part = |name: &str, you, section, of| CirclePart {
            name: name.into(),
            you,
            section,
            of,
            tune: "@110 C".into(),
        };
        let line = circle_line(&[
            part("Poppy", false, 1, 1),
            part("Sam", false, 9, 10),
            part("Kiri", true, 8, 10),
        ]);
        let text: String = line.0.iter().map(|s| s.t.as_str()).collect();
        assert_eq!(text, "🎶 Poppy · Sam 9/10 · you 8/10");
    }

    #[test]
    fn spots_shared_labels() {
        let shared = super::shared_labels(["Surf", "Pizza", "Surf"].into_iter());
        assert!(shared.contains("Surf") && !shared.contains("Pizza"));
    }
}
