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
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Line(pub Vec<Span>);

impl Line {
    pub fn new() -> Self {
        Line(Vec::new())
    }
    /// plain text
    pub fn t(mut self, text: impl Into<String>) -> Self {
        self.0.push(Span { t: text.into(), c: Col::Plain, b: false });
        self
    }
    /// coloured text
    pub fn c(mut self, col: Col, text: impl Into<String>) -> Self {
        self.0.push(Span { t: text.into(), c: col, b: false });
        self
    }
    /// bold coloured text
    pub fn b(mut self, col: Col, text: impl Into<String>) -> Self {
        self.0.push(Span { t: text.into(), c: col, b: true });
        self
    }
    pub fn plain(text: impl Into<String>) -> Self {
        Line::new().t(text)
    }
    pub fn col(col: Col, text: impl Into<String>) -> Self {
        Line::new().c(col, text)
    }
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
    /// Connection refused (e.g. name taken).
    Bye { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToServer {
    Hello {
        name: String,
        #[serde(default)]
        password: Option<String>,
        /// True for AI residents, so the world can tell them from people.
        #[serde(default)]
        bot: bool,
        /// This device's secret: the first device to use a name claims it.
        #[serde(default)]
        key: Option<String>,
    },
    Cmd { text: String },
}

/// A board as plain lines, for clients that show it in their log. `None`
/// for live additions, which arrive as ordinary lines too.
pub fn board_lines(msg: &ToClient) -> Option<Vec<Line>> {
    let ToClient::Board { title, mode, notes, more, page, weeks, .. } = msg else { return None };
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
        out.push(Line::col(Col::Dim, "  Nothing here yet. Be the first: post hello!"));
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
