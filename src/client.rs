//! `world.txt play`: a cosy terminal client.

use crate::protocol::{Action, Col, Line, ToClient, ToServer};
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::{SinkExt, StreamExt};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line as RLine, Span as RSpan, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio_tungstenite::tungstenite::Message;

#[derive(Default)]
struct State {
    log: Vec<Line>,
    place: String,
    weather: String,
    clock: String,
    map: Vec<Line>,
    here: Vec<String>,
    progress: Vec<crate::protocol::Bar>,
    lights: Vec<crate::protocol::Light>,
    hint: Option<String>,
    /// What you could do here, and which one Tab last put in the box.
    actions: Vec<Action>,
    picked: Option<usize>,
    input: String,
    history: Vec<String>,
    hist: usize,
    /// Lines scrolled up from the bottom.
    scroll: u16,
    closed: Option<String>,
}

/// At most this many actions listed in the side panel (Tab reaches them all).
const SHOWN: usize = 8;

impl State {
    /// Tab (or Shift+Tab, going back) puts the next thing you could do in
    /// the box, ready for Enter. Only over an empty box or one Tab filled:
    /// never over something you've typed.
    fn pick(&mut self, back: bool) {
        let n = self.actions.len();
        if n == 0 || (self.picked.is_none() && !self.input.is_empty()) {
            return;
        }
        let i = match (self.picked, back) {
            (None, false) => 0,
            (None, true) => n - 1,
            (Some(i), false) => (i + 1) % n,
            (Some(i), true) => (i + n - 1) % n,
        };
        let a = &self.actions[i];
        self.input = a.run.clone().or_else(|| a.fill.clone()).unwrap_or_default();
        self.picked = Some(i);
    }

    /// New things to do: what Tab was on may have moved, or gone (then
    /// what's in the box stays, as if typed).
    fn set_actions(&mut self, actions: Vec<Action>) {
        if self.picked.is_some() {
            self.picked = actions.iter().position(|a| a.run.as_ref().or(a.fill.as_ref()) == Some(&self.input));
        }
        self.actions = actions;
    }
}

pub async fn run(server: &str, invite: Option<String>, sound_on: bool) -> anyhow::Result<()> {
    if invite.is_none() && crate::identity::load_token("player", server).is_none() {
        anyhow::bail!(
            "world.txt is invite-only 💌 Ask an island keeper for a magic link, then: world.txt play --invite <link or code>"
        );
    }

    let (ws, _) = tokio_tungstenite::connect_async(server).await?;
    let (mut sink, mut source) = ws.split();
    let hello = serde_json::to_string(&crate::identity::hello("player", server, false, invite))?;
    sink.send(Message::Text(hello.into())).await?;

    let mut terminal = ratatui::init();
    let mut events = EventStream::new();
    let mut st = State::default();
    let mut sound = crate::sound::Sound::new();
    let mut told_sound = false;
    if sound_on && let Err(e) = sound.set_on(true) {
        st.log.push(Line::col(Col::Pink, e));
    }
    let (mut records, mut played) = crate::records::Records::new();
    let mut deck = Deck::default();

    let result: anyhow::Result<()> = async {
        loop {
            terminal.draw(|f| draw(f, &mut st))?;
            tokio::select! {
                msg = source.next() => match msg {
                    Some(Ok(Message::Text(t))) => {
                        match serde_json::from_str::<ToClient>(t.as_str()) {
                            Ok(ToClient::Lines { lines }) => {
                                for l in &lines {
                                    if let Some(tune) = hummed(l) {
                                        sound.tune(tune);
                                    }
                                }
                                st.log.extend(lines);
                                let excess = st.log.len().saturating_sub(2000);
                                st.log.drain(..excess);
                            }
                            Ok(ToClient::Status { place, weather, clock, map, here, progress, lights, actions, sounds, .. }) => {
                                sound.levels(sounds);
                                st.set_actions(actions);
                                st.progress = progress;
                                st.lights = lights;
                                st.place = place;
                                st.weather = weather;
                                st.clock = clock;
                                st.map = map;
                                st.here = here;
                            }
                            Ok(ToClient::Hint { text, .. }) => st.hint = text,
                            // No sound here: one short line a turn says who's humming.
                            Ok(ToClient::Circle { parts, .. }) if !parts.is_empty() => {
                                for p in &parts {
                                    sound.tune(&p.tune);
                                }
                                st.log.push(crate::protocol::circle_line(&parts));
                            }
                            Ok(ToClient::Circle { .. }) => {}
                            Ok(ToClient::Travel { url }) => st.log.push(crate::protocol::Line::new()
                                .c(crate::protocol::Col::Blue, "⛵ Visiting other worlds happens in a browser: open ")
                                .b(crate::protocol::Col::Yellow, url)),
                            Ok(msg @ ToClient::Board { .. }) => {
                                if let Some(lines) = crate::protocol::board_lines(&msg) {
                                    st.log.extend(lines);
                                    st.scroll = 0;
                                }
                            }
                            Ok(msg @ ToClient::Bye { .. }) => {
                                crate::identity::on_sign_in("player", server, &msg);
                                if let ToClient::Bye { reason } = msg { st.closed = Some(reason); }
                            }
                            Ok(msg @ ToClient::Welcome { .. }) => crate::identity::on_sign_in("player", server, &msg),
                            Ok(ToClient::Restarting) => {
                                st.closed = Some("🌙 The world is restarting, just for a moment. Run world.txt play again in a few seconds.".into());
                            }
                            // Music in earshot, through mpv if there is one.
                            Ok(ToClient::Radio { thing, url, gain }) => {
                                let heard = sound.radio(thing, url.as_deref(), gain);
                                tell_sound(&mut st, &mut told_sound, heard, "📻 There's a radio playing nearby.");
                            }
                            Ok(ToClient::Audio { thing, frames, gain, .. }) => {
                                let heard = sound.audio(thing, &frames, gain);
                                tell_sound(&mut st, &mut told_sound, heard, "🎶 There's music playing nearby.");
                            }
                            Ok(ToClient::AudioEnd { thing }) => sound.end(thing),
                            // Playing your own songs: you're the record player.
                            Ok(ToClient::SpinStart { thing }) => {
                                deck.spin = Some(thing);
                                match deck.want_play.take() {
                                    Some(files) => records.gramophone(files),
                                    None => st.log.push(Line::col(Col::Pink, "🎶 Which songs? play music <a file or a folder> (dragging them in works too)")),
                                }
                            }
                            Ok(ToClient::SpinStop { thing }) => {
                                if deck.spin == Some(thing) {
                                    deck.spin = None;
                                    records.stop();
                                }
                            }
                            Ok(ToClient::JukeAdd { thing }) => {
                                deck.jukebox = Some(thing);
                                match deck.want_queue.take() {
                                    Some(songs) => {
                                        let msg = serde_json::to_string(&ToServer::Queue { thing, songs })?;
                                        if sink.send(Message::Text(msg.into())).await.is_err() { break; }
                                    }
                                    None => st.log.push(Line::col(Col::Pink, format!("📀 Which songs? add songs <files or a folder> (up to {})", crate::records::JUKEBOX_MAX))),
                                }
                            }
                            Ok(ToClient::JukeTurn { thing, key }) => match deck.queued.remove(&key) {
                                Some(path) => records.jukebox(path, thing, key),
                                None => {
                                    let msg = serde_json::to_string(&ToServer::TrackDone { thing, key })?;
                                    if sink.send(Message::Text(msg.into())).await.is_err() { break; }
                                }
                            },
                            Ok(ToClient::JukeStop { .. }) => records.stop(),
                            Err(_) => {}
                        }
                    }
                    Some(Ok(_)) => {}
                    _ => break,
                },
                Some(p) = played.recv() => {
                    use crate::records::Played;
                    let msg = match p {
                        Played::Track { title } => { st.log.push(Line::col(Col::Pink, format!("🎶 Playing {title}"))); None }
                        Played::Note(text) => { st.log.push(Line::col(Col::Pink, text)); None }
                        Played::Frames { frames, title } => Some(ToServer::Audio { frames, title: Some(title) }),
                        Played::Done { thing, key } => Some(ToServer::TrackDone { thing, key }),
                    };
                    if let Some(m) = msg
                        && sink.send(Message::Text(serde_json::to_string(&m)?.into())).await.is_err()
                    {
                        break;
                    }
                }
                ev = events.next() => {
                    let Some(Ok(Event::Key(key))) = ev else { continue };
                    if key.kind != KeyEventKind::Press { continue }
                    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                    match key.code {
                        KeyCode::Char('c' | 'd') if ctrl => break,
                        KeyCode::Esc => break,
                        KeyCode::Tab => st.pick(false),
                        KeyCode::BackTab => st.pick(true),
                        KeyCode::Char(c) => { st.picked = None; st.input.push(c); }
                        KeyCode::Backspace => { st.picked = None; st.input.pop(); }
                        KeyCode::Enter => {
                            st.picked = None;
                            let text = std::mem::take(&mut st.input);
                            if !text.trim().is_empty() {
                                st.history.push(text.clone());
                                st.hist = st.history.len();
                                st.scroll = 0;
                                // Songs from this computer: the world only hears the start.
                                let to_world = match local(&text) {
                                    Some(Local::PlayMusic(paths)) => deck.play_music(&paths, &mut records, &mut st),
                                    Some(Local::AddSongs(paths)) => deck.add_songs(&paths, &records, &mut st),
                                    Some(Local::Sound(on)) => {
                                        let on = on.unwrap_or(!sound.is_on());
                                        let said = match sound.set_on(on) {
                                            Ok(()) if on => "🔊 Sound on: music, hums and the island's own sounds. 'sound off' to stop.".to_string(),
                                            Ok(()) => "🔇 Sound off.".to_string(),
                                            Err(e) => e,
                                        };
                                        st.log.push(Line::col(Col::Pink, said));
                                        None
                                    }
                                    None => Some(ToServer::Cmd { text }),
                                };
                                let Some(to_world) = to_world else { continue };
                                let msg = serde_json::to_string(&to_world)?;
                                if sink.send(Message::Text(msg.into())).await.is_err() { break; }
                            }
                        }
                        KeyCode::Up if st.hist > 0 => {
                            st.picked = None;
                            st.hist -= 1;
                            st.input = st.history[st.hist].clone();
                        }
                        KeyCode::Down => {
                            st.picked = None;
                            st.hist = (st.hist + 1).min(st.history.len());
                            st.input = st.history.get(st.hist).cloned().unwrap_or_default();
                        }
                        KeyCode::PageUp => st.scroll = st.scroll.saturating_add(10),
                        KeyCode::PageDown => st.scroll = st.scroll.saturating_sub(10),
                        _ => {}
                    }
                }
            }
            if st.closed.is_some() {
                break;
            }
        }
        Ok(())
    }
    .await;

    ratatui::restore();
    let _ = sink.close().await;
    println!("{}", st.closed.unwrap_or_else(|| "✧ You drift out of the world. Come back soon. 💜".into()));
    result
}

/// Typed commands the terminal handles itself, at least to begin with.
#[derive(Debug, PartialEq)]
enum Local {
    /// play music ~/Music/Rumours
    PlayMusic(String),
    /// add songs a.mp3 b.mp3
    AddSongs(String),
    /// sound on, sound off, or just sound to flip it.
    Sound(Option<bool>),
}

fn local(text: &str) -> Option<Local> {
    let t = text.trim();
    match t.to_lowercase().as_str() {
        "sound" => return Some(Local::Sound(None)),
        "sound on" => return Some(Local::Sound(Some(true))),
        "sound off" => return Some(Local::Sound(Some(false))),
        _ => {}
    }
    let (verb, rest) = t.split_once(char::is_whitespace)?;
    let (what, paths) = rest.trim_start().split_once(char::is_whitespace)?;
    let (verb, what, paths) = (verb.to_lowercase(), what.to_lowercase(), paths.trim().to_string());
    if paths.is_empty() {
        return None;
    }
    match (verb.as_str(), what.as_str()) {
        ("play" | "wind", "music" | "songs" | "records") => Some(Local::PlayMusic(paths)),
        ("add" | "queue", "songs" | "music") => Some(Local::AddSongs(paths)),
        _ => None,
    }
}

/// The record player's side of things: which gramophone or jukebox you're
/// at, and the songs waiting to go on.
#[derive(Default)]
struct Deck {
    /// The gramophone you've wound up.
    spin: Option<i64>,
    want_play: Option<Vec<std::path::PathBuf>>,
    /// The jukebox you last added songs to, and what you've queued, by key.
    jukebox: Option<i64>,
    want_queue: Option<Vec<crate::protocol::Song>>,
    queued: std::collections::HashMap<String, std::path::PathBuf>,
}

impl Deck {
    /// `play music <songs>`: wind up the gramophone first if it isn't.
    fn play_music(&mut self, paths: &str, records: &mut crate::records::Records, st: &mut State) -> Option<ToServer> {
        let files = self.songs(paths, 500, records, st)?;
        if self.spin.is_some() {
            records.gramophone(files);
            return None;
        }
        self.want_play = Some(files);
        Some(ToServer::Cmd { text: "play music".into() })
    }

    /// `add songs <songs>`: only their titles go to the world.
    fn add_songs(&mut self, paths: &str, records: &crate::records::Records, st: &mut State) -> Option<ToServer> {
        let files = self.songs(paths, crate::records::JUKEBOX_MAX, records, st)?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        let songs: Vec<crate::protocol::Song> = files
            .into_iter()
            .enumerate()
            .map(|(i, f)| {
                let key = format!("t{stamp:x}{i}");
                let title = crate::records::title(&f);
                self.queued.insert(key.clone(), f);
                crate::protocol::Song { key, title }
            })
            .collect();
        match self.jukebox {
            Some(thing) => Some(ToServer::Queue { thing, songs }),
            None => {
                self.want_queue = Some(songs);
                Some(ToServer::Cmd { text: "add songs".into() })
            }
        }
    }

    fn songs(&self, paths: &str, max: usize, records: &crate::records::Records, st: &mut State) -> Option<Vec<std::path::PathBuf>> {
        if !records.can_play() {
            st.log.push(Line::col(Col::Pink, format!("🎶 To play songs from the terminal: {}, then world.txt play again.", crate::records::INSTALL)));
            return None;
        }
        match crate::records::songs(paths, max) {
            Ok(files) => Some(files),
            Err(e) => {
                st.log.push(Line::col(Col::Pink, format!("🎶 {e}")));
                None
            }
        }
    }
}

/// The first music you can't hear says how you could.
fn tell_sound(st: &mut State, told: &mut bool, heard: crate::sound::Heard, what: &str) {
    use crate::sound::Heard;
    if *told || !matches!(heard, Heard::Off | Heard::NoPlayer) {
        return;
    }
    *told = true;
    st.log.push(match heard {
        Heard::Off => Line::new().c(Col::Pink, format!("{what} Type ")).b(Col::Pink, "sound on").c(Col::Pink, " to hear it."),
        _ => Line::new().c(Col::Pink, format!("{what} To hear it here: ")).b(Col::Pink, crate::sound::INSTALL).c(Col::Pink, ", then world.txt play again."),
    });
}

/// A hum's tune, from the ♪ … ♪ in a line (as the browser finds it).
fn hummed(l: &Line) -> Option<&str> {
    let text = l.0.iter().find(|s| s.t.contains('♪'))?;
    let inner = text.t.split_once("♪ ")?.1;
    inner.rsplit_once(" ♪").map(|(t, _)| t)
}

fn draw(f: &mut Frame, st: &mut State) {
    let [main, input] = Layout::vertical([Constraint::Min(5), Constraint::Length(3)]).areas(f.area());
    let [log_area, side] = Layout::horizontal([Constraint::Min(30), Constraint::Length(34)]).areas(main);

    // The log, pinned to the bottom unless scrolled.
    let text = Text::from(st.log.iter().map(to_rline).collect::<Vec<_>>());
    let para = Paragraph::new(text).wrap(Wrap { trim: false });
    let inner_w = log_area.width.saturating_sub(2).max(1);
    let total = para.line_count(inner_w) as u16;
    let visible = log_area.height.saturating_sub(2);
    let max_scroll = total.saturating_sub(visible);
    st.scroll = st.scroll.min(max_scroll);
    let title = if st.scroll > 0 { " ✨ world.txt  (scrolled ↑) " } else { " ✨ world.txt " };
    f.render_widget(
        para.scroll((max_scroll - st.scroll, 0)).block(
            Block::default().borders(Borders::ALL).border_style(dim()).title(RSpan::styled(title, magenta())),
        ),
        log_area,
    );

    // Side panel.
    let mut side_lines: Vec<RLine> = vec![
        RLine::from(RSpan::styled(st.place.clone(), magenta().add_modifier(Modifier::BOLD))),
        RLine::from(RSpan::styled(st.weather.clone(), Style::default().fg(colour(Col::Blue)))),
        RLine::from(RSpan::styled(st.clock.clone(), dim())),
        // The real date and time here, which event times are in.
        RLine::from(RSpan::styled(chrono::Local::now().format("🕰 %a %-d %b, %-I:%M%P · your time").to_string(), dim())),
        RLine::default(),
    ];
    side_lines.extend(st.map.iter().map(|l| to_rline(l).centered()));
    side_lines.push(RLine::default());
    if !st.here.is_empty() {
        side_lines.push(RLine::from(RSpan::styled(
            format!("☺ {}", st.here.join(", ")),
            Style::default().fg(colour(Col::Cyan)),
        )));
    }
    // Progress bars from boards you follow or are near: ▓▓▓▓░░░░ Pizza 37%
    // Two with the same label (on different boards) say where they are.
    let where_ = |label: &str, place: &str, shared: &std::collections::HashSet<&str>| {
        if shared.contains(label) && !place.is_empty() { format!("{label} · {place}") } else { label.to_string() }
    };
    let shared_bars = crate::protocol::shared_labels(st.progress.iter().map(|b| b.label.as_str()));
    let shared_lights = crate::protocol::shared_labels(st.lights.iter().map(|l| l.label.as_str()));
    for b in &st.progress {
        const WIDTH: u64 = 10;
        let filled = (b.percent() * WIDTH / 100) as usize;
        side_lines.push(RLine::from(vec![
            RSpan::styled("▓".repeat(filled), Style::default().fg(colour(Col::Green))),
            RSpan::styled("░".repeat(WIDTH as usize - filled), dim()),
            RSpan::raw(format!(" {} ", where_(&b.label, &b.place, &shared_bars))),
            RSpan::styled(b.amount(), dim()),
        ]));
    }
    // Up-or-down lights: ● Surf down · flat as a millpond
    for l in &st.lights {
        let (col, word) = if l.up { (Col::Green, "up") } else { (Col::Red, "down") };
        let mut spans = vec![
            RSpan::styled("● ", Style::default().fg(colour(col))),
            RSpan::raw(format!("{} ", where_(&l.label, &l.place, &shared_lights))),
            RSpan::styled(word, Style::default().fg(colour(col))),
        ];
        if let Some(n) = l.note.as_ref().filter(|_| !l.up) {
            spans.push(RSpan::styled(format!(" · {}", crate::links::labels(n)), dim()));
        }
        side_lines.push(RLine::from(spans));
    }
    if !st.progress.is_empty() || !st.lights.is_empty() {
        side_lines.push(RLine::default());
    }
    // What you could do here: Tab puts each in the box.
    if !st.actions.is_empty() {
        side_lines.push(RLine::from(RSpan::styled("You could (Tab):", dim())));
        for (i, a) in st.actions.iter().enumerate().take(SHOWN) {
            let style = if st.picked == Some(i) { magenta().add_modifier(Modifier::BOLD | Modifier::REVERSED) } else { Style::default().fg(colour(Col::Pink)) };
            side_lines.push(RLine::from(vec![RSpan::raw("  "), RSpan::styled(a.label.clone(), style)]));
        }
        if st.actions.len() > SHOWN {
            let more = st.picked.filter(|&i| i >= SHOWN).map_or_else(|| format!("  +{} more", st.actions.len() - SHOWN), |i| format!("  {}", st.actions[i].label));
            side_lines.push(RLine::from(RSpan::styled(more, if st.picked.is_some_and(|i| i >= SHOWN) { magenta().add_modifier(Modifier::BOLD | Modifier::REVERSED) } else { dim() })));
        }
        side_lines.push(RLine::default());
    }
    if let Some(h) = &st.hint {
        side_lines.push(RLine::from(RSpan::styled(
            h.clone(),
            Style::default().fg(colour(Col::Yellow)).add_modifier(Modifier::BOLD),
        )));
        side_lines.push(RLine::from(RSpan::styled("(skip to hide)", dim())));
        side_lines.push(RLine::default());
    }
    side_lines.push(RLine::from(RSpan::styled("PgUp/PgDn scroll · Esc leave", dim())));
    f.render_widget(
        Paragraph::new(side_lines)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).border_style(dim())),
        side,
    );

    // Input.
    f.render_widget(
        Paragraph::new(RLine::from(vec![
            RSpan::styled("› ", magenta()),
            RSpan::raw(st.input.clone()),
        ]))
        .block(Block::default().borders(Borders::ALL).border_style(dim())),
        input,
    );
    let cx = input.x + 3 + st.input.chars().count() as u16;
    f.set_cursor_position((cx.min(input.right().saturating_sub(2)), input.y + 1));
}

fn to_rline(l: &Line) -> RLine<'static> {
    RLine::from(
        l.0.iter()
            .map(|s| {
                let mut style = Style::default().fg(colour(s.c));
                if s.b {
                    style = style.add_modifier(Modifier::BOLD);
                }
                RSpan::styled(crate::links::plain(&s.t), style)
            })
            .collect::<Vec<_>>(),
    )
}

fn dim() -> Style {
    Style::default().fg(colour(Col::Dim))
}

fn magenta() -> Style {
    Style::default().fg(colour(Col::Magenta))
}

fn colour(c: Col) -> Color {
    match c {
        Col::Plain => Color::Reset,
        Col::Dim => Color::Rgb(141, 132, 158),
        Col::White => Color::Rgb(255, 255, 255),
        Col::Blue => Color::Rgb(106, 168, 255),
        Col::DeepBlue => Color::Rgb(59, 91, 219),
        Col::Cyan => Color::Rgb(95, 224, 224),
        Col::Green => Color::Rgb(126, 216, 126),
        Col::DarkGreen => Color::Rgb(63, 154, 79),
        Col::Yellow => Color::Rgb(255, 216, 102),
        Col::Sand => Color::Rgb(232, 207, 154),
        Col::Magenta => Color::Rgb(255, 121, 198),
        Col::Pink => Color::Rgb(255, 167, 214),
        Col::Red => Color::Rgb(255, 107, 107),
        Col::Brown => Color::Rgb(192, 138, 90),
        Col::Grey => Color::Rgb(160, 160, 168),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn act(label: &str, run: Option<&str>, fill: Option<&str>) -> Action {
        Action { label: label.into(), run: run.map(Into::into), fill: fill.map(Into::into) }
    }

    #[test]
    fn tab_goes_round_what_you_could_do() {
        let mut st = State { actions: vec![act("👋 wave", Some("wave"), None), act("💬 Sam…", None, Some("@Sam ")), act("📖 read", Some("read"), None)], ..Default::default() };
        st.pick(false);
        assert_eq!(st.input, "wave");
        st.pick(false);
        assert_eq!(st.input, "@Sam ", "a fill starts a command for you");
        st.pick(false);
        st.pick(false);
        assert_eq!(st.input, "wave", "and round again");
        st.pick(true);
        assert_eq!(st.input, "read", "Shift+Tab goes back");
    }

    #[test]
    fn songs_from_here_stay_here() {
        assert_eq!(local("play music ~/Music/Rumours"), Some(Local::PlayMusic("~/Music/Rumours".into())));
        assert_eq!(local("Play  songs a.mp3 b.mp3"), Some(Local::PlayMusic("a.mp3 b.mp3".into())));
        assert_eq!(local("add songs ~/Music/A\\ Little\\ Tune.mp3"), Some(Local::AddSongs("~/Music/A\\ Little\\ Tune.mp3".into())));
        assert_eq!(local("play music"), None, "plain, it's for the world");
        assert_eq!(local("add songs"), None);
        assert_eq!(local("add Sam to picnic"), None);
        assert_eq!(local("play"), None);
        assert_eq!(local("Sound On"), Some(Local::Sound(Some(true))));
        assert_eq!(local("sound"), Some(Local::Sound(None)));
        assert_eq!(local("sound of the sea"), None);
    }

    #[test]
    fn hums_are_found_in_lines() {
        let l = Line::new().b(Col::Cyan, "Sam").c(Col::Dim, " hums ").c(Col::Pink, "♪ @90 C E G- ♪");
        assert_eq!(hummed(&l), Some("@90 C E G-"));
        assert_eq!(hummed(&Line::col(Col::Pink, "♪ You hear someone hums nearby.")), None);
        assert_eq!(hummed(&Line::plain("hello")), None);
    }

    #[test]
    fn tab_leaves_your_own_typing_alone() {
        let mut st = State { actions: vec![act("👋 wave", Some("wave"), None)], input: "hello th".into(), ..Default::default() };
        st.pick(false);
        assert_eq!(st.input, "hello th");
        assert_eq!(st.picked, None);
    }

    #[test]
    fn new_actions_follow_or_forget_the_pick() {
        let mut st = State { actions: vec![act("👋 wave", Some("wave"), None), act("📖 read", Some("read"), None)], ..Default::default() };
        st.pick(false);
        st.set_actions(vec![act("🎣 fish", Some("fish"), None), act("👋 wave", Some("wave"), None)]);
        assert_eq!(st.picked, Some(1), "followed to where it went");
        st.pick(false);
        assert_eq!(st.input, "fish");
        st.set_actions(vec![act("📖 read", Some("read"), None)]);
        assert_eq!(st.picked, None, "gone");
        st.pick(false);
        assert_eq!(st.input, "fish", "so the box is left as it is");
    }
}
