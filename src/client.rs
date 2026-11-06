//! `world.txt play`: a cosy terminal client.

use crate::protocol::{Col, Line, ToClient, ToServer};
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
    input: String,
    history: Vec<String>,
    hist: usize,
    /// Lines scrolled up from the bottom.
    scroll: u16,
    closed: Option<String>,
}

pub async fn run(server: &str, invite: Option<String>) -> anyhow::Result<()> {
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

    let result: anyhow::Result<()> = async {
        loop {
            terminal.draw(|f| draw(f, &mut st))?;
            tokio::select! {
                msg = source.next() => match msg {
                    Some(Ok(Message::Text(t))) => {
                        match serde_json::from_str::<ToClient>(t.as_str()) {
                            Ok(ToClient::Lines { lines }) => {
                                st.log.extend(lines);
                                let excess = st.log.len().saturating_sub(2000);
                                st.log.drain(..excess);
                            }
                            Ok(ToClient::Status { place, weather, clock, map, here, progress, lights, .. }) => {
                                st.progress = progress;
                                st.lights = lights;
                                st.place = place;
                                st.weather = weather;
                                st.clock = clock;
                                st.map = map;
                                st.here = here;
                            }
                            Ok(ToClient::Hint { text, .. }) => st.hint = text,
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
                            // Sound is for browsers.
                            Ok(ToClient::SpinStart { .. } | ToClient::SpinStop { .. } | ToClient::Audio { .. } | ToClient::AudioEnd { .. } | ToClient::Radio { .. } | ToClient::JukeAdd { .. } | ToClient::JukeTurn { .. } | ToClient::JukeStop { .. }) | Err(_) => {}
                        }
                    }
                    Some(Ok(_)) => {}
                    _ => break,
                },
                ev = events.next() => {
                    let Some(Ok(Event::Key(key))) = ev else { continue };
                    if key.kind != KeyEventKind::Press { continue }
                    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                    match key.code {
                        KeyCode::Char('c' | 'd') if ctrl => break,
                        KeyCode::Esc => break,
                        KeyCode::Char(c) => st.input.push(c),
                        KeyCode::Backspace => { st.input.pop(); }
                        KeyCode::Enter => {
                            let text = std::mem::take(&mut st.input);
                            if !text.trim().is_empty() {
                                st.history.push(text.clone());
                                st.hist = st.history.len();
                                st.scroll = 0;
                                let msg = serde_json::to_string(&ToServer::Cmd { text })?;
                                if sink.send(Message::Text(msg.into())).await.is_err() { break; }
                            }
                        }
                        KeyCode::Up if st.hist > 0 => {
                            st.hist -= 1;
                            st.input = st.history[st.hist].clone();
                        }
                        KeyCode::Down => {
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
    let mut side_lines: Vec<RLine> = Vec::new();
    side_lines.push(RLine::from(RSpan::styled(st.place.clone(), magenta().add_modifier(Modifier::BOLD))));
    side_lines.push(RLine::from(RSpan::styled(st.weather.clone(), Style::default().fg(colour(Col::Blue)))));
    side_lines.push(RLine::from(RSpan::styled(st.clock.clone(), dim())));
    // The real date and time here, which event times are in.
    side_lines.push(RLine::from(RSpan::styled(chrono::Local::now().format("🕰 %a %-d %b, %-I:%M%P · your time").to_string(), dim())));
    side_lines.push(RLine::default());
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
