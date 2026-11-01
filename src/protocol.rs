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
    },
    Cmd { text: String },
}
