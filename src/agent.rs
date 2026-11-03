//! `world.txt agent`: an AI island resident. It joins like any player and
//! only thinks (calls Claude) when something happens near it, with layered
//! limits so it can't burn through tokens:
//!
//! - asleep while no humans are in the world
//! - wakes on being spoken to / someone talking nearby, or rarely when idle
//! - at least MIN_GAP between calls, at most MAX_PER_HOUR calls an hour
//! - a daily dollar budget, tracked from the API's own usage numbers
//! - small requests: a cached persona prefix plus the last few lines it saw

use crate::protocol::{ToClient, ToServer};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;

const MODEL: &str = "claude-haiku-5-5";
const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";

// Claude Haiku 5.5, $ per million tokens (prompts under 100K). Cache writes
// are 1.25x input and reads 0.1x input. Used to estimate spend from `usage`.
const PRICE_IN: f64 = 0.10;
const PRICE_CACHE_WRITE: f64 = 0.125;
const PRICE_CACHE_READ: f64 = 0.01;
const PRICE_OUT: f64 = 0.50;

const MIN_GAP: Duration = Duration::from_secs(20);
const MAX_PER_HOUR: usize = 40;
const DEBOUNCE: Duration = Duration::from_secs(3);
const IDLE_EVERY: Duration = Duration::from_secs(10 * 60);
const MAX_TOKENS: u32 = 1024;
const CONTEXT_LINES: usize = 40;
const MAX_NOTES: usize = 30;

/// Commands a resident may use. No unmake, push, shout or world-wide chat.
const ALLOWED: &[&str] = &[
    "say", "emote", "me", "tell", "reply", "look", "hum", "sing", "whistle", "plant", "dig",
    "path", "make", "gather", "pick", "catch", "fish", "give", "read", "post", "play", "sit", "rest", "dance", "wave", "find", "n", "s", "e", "w", "ne", "nw", "se",
    "sw", "north", "south", "east", "west", "north-east", "north-west", "south-east", "south-west",
];

pub struct Options {
    pub name: String,
    pub persona: PathBuf,
    pub server: String,
    pub daily_usd: f64,
    pub state_dir: PathBuf,
    pub dry_run: bool,
    pub invite: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct Ledger {
    day: u64,
    usd: f64,
    calls: u64,
    input_tokens: u64,
    output_tokens: u64,
    asleep_announced: bool,
    /// Things the resident chose to remember, oldest first.
    notes: Vec<String>,
}

struct Status {
    place: String,
    weather: String,
    clock: String,
    here: Vec<String>,
    humans: u32,
}

pub async fn run(opts: Options) -> anyhow::Result<()> {
    let persona = std::fs::read_to_string(&opts.persona)?;
    let api_key = std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.is_empty());
    if api_key.is_none() && !opts.dry_run {
        anyhow::bail!("set ANTHROPIC_API_KEY (or use --dry-run to watch without calling Claude)");
    }
    std::fs::create_dir_all(&opts.state_dir)?;
    let ledger_path = opts.state_dir.join(format!("{}.json", opts.name.to_lowercase()));
    let mut ledger: Ledger = std::fs::read_to_string(&ledger_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let ws = crate::bridge::connect_retry(&opts.server).await?;
    let (mut sink, mut source) = ws.split();
    let who = opts.name.to_lowercase();
    let hello = crate::identity::hello(&who, &opts.server, true, opts.invite.clone());
    sink.send(Message::Text(serde_json::to_string(&hello)?.into())).await?;
    log(&opts.name, &format!("🐾 joined {} (budget ${:.2}/day, {MODEL})", opts.server, opts.daily_usd));

    let system = system_prompt(&opts.name, &persona);
    let http = reqwest::Client::new();
    let mut seen: VecDeque<String> = VecDeque::new();
    let mut status: Option<Status> = None;
    let mut wake_at: Option<Instant> = None;
    let mut wake_reason = String::new();
    let mut last_call: Option<Instant> = None;
    let mut recent_calls: VecDeque<Instant> = VecDeque::new();
    let mut idle = tokio::time::interval_at(Instant::now() + IDLE_EVERY, IDLE_EVERY);
    let me = opts.name.to_lowercase();

    loop {
        let wake = async {
            match wake_at {
                Some(t) => tokio::time::sleep_until(t).await,
                None => std::future::pending().await,
            }
        };
        let reason = tokio::select! {
            msg = source.next() => {
                let Some(Ok(Message::Text(t))) = msg else {
                    if matches!(msg, None | Some(Err(_)) | Some(Ok(Message::Close(_)))) { break }
                    continue
                };
                match serde_json::from_str::<ToClient>(t.as_str()) {
                    Ok(ToClient::Lines { lines }) => {
                        for line in lines {
                            let text: String = line.0.iter().map(|s| s.t.as_str()).collect();
                            if text.trim().is_empty() { continue }
                            if let Some(why) = trigger(&text, &me) {
                                if wake_at.is_none() {
                                    wake_at = Some(Instant::now() + DEBOUNCE);
                                    wake_reason = why.to_string();
                                }
                            }
                            seen.push_back(text);
                            while seen.len() > CONTEXT_LINES { seen.pop_front(); }
                        }
                    }
                    Ok(ToClient::Status { place, weather, clock, here, humans, .. }) => {
                        status = Some(Status { place, weather, clock, here, humans });
                    }
                    Ok(msg @ ToClient::Bye { .. }) => {
                        crate::identity::on_sign_in(&who, &opts.server, &msg);
                        if let ToClient::Bye { reason } = msg { log(&opts.name, &format!("bye: {reason}")); }
                        break
                    }
                    Ok(msg @ ToClient::Welcome { .. }) => crate::identity::on_sign_in(&who, &opts.server, &msg),
                    Ok(msg @ ToClient::Board { .. }) => {
                        for line in crate::protocol::board_lines(&msg).unwrap_or_default() {
                            seen.push_back(line.0.iter().map(|s| s.t.as_str()).collect());
                        }
                        while seen.len() > CONTEXT_LINES { seen.pop_front(); }
                    }
                    // Sound is for browsers.
                    Ok(ToClient::Hint { .. } | ToClient::Travel { .. } | ToClient::SpinStart { .. } | ToClient::SpinStop { .. } | ToClient::Audio { .. } | ToClient::AudioEnd { .. } | ToClient::Radio { .. } | ToClient::JukeAdd { .. } | ToClient::JukeTurn { .. } | ToClient::JukeStop { .. } | ToClient::Restarting) | Err(_) => {}
                }
                continue;
            }
            _ = wake => { wake_at = None; std::mem::take(&mut wake_reason) }
            _ = idle.tick() => "it's been quiet for a while; maybe potter about".to_string(),
        };

        // ---- should we think at all? ----------------------------------
        let Some(st) = &status else { continue };
        if st.humans == 0 {
            continue; // nobody to see it: stay asleep, spend nothing
        }
        let today = day_number();
        if ledger.day != today {
            ledger = Ledger { day: today, notes: std::mem::take(&mut ledger.notes), ..Default::default() };
        }
        if ledger.usd >= opts.daily_usd {
            if !ledger.asleep_announced {
                ledger.asleep_announced = true;
                save(&ledger_path, &ledger);
                send_cmd(&mut sink, "emote yawns, stretches, and curls up for a long nap. 💤").await?;
                log(&opts.name, &format!("daily budget reached (${:.4}); sleeping until tomorrow", ledger.usd));
            }
            continue;
        }
        let now = Instant::now();
        if let Some(t) = last_call {
            if now < t + MIN_GAP {
                // Too soon: think a little later instead, keeping the reason.
                if wake_at.is_none() {
                    wake_at = Some(t + MIN_GAP);
                    wake_reason = reason;
                }
                continue;
            }
        }
        while recent_calls.front().is_some_and(|t| now.duration_since(*t) > Duration::from_secs(3600)) {
            recent_calls.pop_front();
        }
        if recent_calls.len() >= MAX_PER_HOUR {
            continue;
        }

        // ---- think --------------------------------------------------------
        last_call = Some(now);
        recent_calls.push_back(now);
        let context = user_prompt(st, &seen, &ledger.notes, &reason);
        let body = request_body(&system, &context);

        if opts.dry_run {
            log(&opts.name, &format!("[dry run] would think because: {reason}\n{context}"));
            continue;
        }
        let Some(key) = &api_key else { continue };
        let reply = match call_claude(&http, key, &body).await {
            Ok(r) => r,
            Err(e) => {
                log(&opts.name, &format!("⚠ {e}"));
                continue;
            }
        };
        ledger.calls += 1;
        ledger.usd += reply.usd;
        ledger.input_tokens += reply.input_tokens;
        ledger.output_tokens += reply.output_tokens;
        if let Some(note) = reply.remember.filter(|n| !n.trim().is_empty()) {
            ledger.notes.push(note.chars().take(200).collect());
            let excess = ledger.notes.len().saturating_sub(MAX_NOTES);
            ledger.notes.drain(..excess);
        }
        save(&ledger_path, &ledger);
        log(
            &opts.name,
            &format!(
                "thought ({reason}) → {:?}  [${:.5}, today ${:.4} / ${:.2}, {} calls]",
                reply.commands, reply.usd, ledger.usd, opts.daily_usd, ledger.calls
            ),
        );
        for cmd in reply.commands.iter().take(3) {
            if allowed(cmd) {
                send_cmd(&mut sink, cmd).await?;
                tokio::time::sleep(Duration::from_millis(1500)).await;
            } else {
                log(&opts.name, &format!("skipped disallowed command: {cmd}"));
            }
        }
    }
    Ok(())
}

/// Does this line deserve a response? Returns a short reason.
pub(crate) fn trigger(text: &str, me: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    let own_chat = lower.starts_with(&format!("💬 {me}:"));
    if lower.starts_with("you ") || lower.starts_with(me) || own_chat || lower.starts_with("✉ you ") {
        return None; // our own actions echoed back
    }
    if lower.contains("for the first time") {
        Some("someone new just arrived on the island; maybe welcome them with @Name")
    } else if lower.contains(" says to you:") || lower.contains(" tells you:") {
        Some("someone sent you a message")
    } else if lower.contains(me) {
        Some("someone mentioned you")
    } else if lower.contains(" says, ") || lower.contains(" hums ") || lower.contains(" sings ") {
        Some("someone is talking near you")
    } else if lower.contains(" arrives from ") || lower.contains(" appears, blinking") {
        Some("someone arrived where you are")
    } else if lower.contains(" makes ") || lower.contains(" plants ") || lower.contains(" digs ") {
        Some("someone is building near you")
    } else {
        None
    }
}

fn allowed(cmd: &str) -> bool {
    let cmd = cmd.trim();
    if cmd.starts_with('@') {
        return !cmd[1..].to_lowercase().starts_with("all");
    }
    let verb = cmd.split_whitespace().next().unwrap_or("").to_lowercase();
    ALLOWED.contains(&verb.as_str())
}

fn system_prompt(name: &str, persona: &str) -> String {
    format!(
        "You are {name}, a resident of world.txt: a cozy, shared text world on a small island. \
There are no battles. People wander, chat, shape the land and make things together. \
Many of them are children.

You see the world as lines of text, the same as everyone else, and you act by \
sending the same commands a person would type. Each time you're woken you choose \
up to three commands to send, in order. Often one is plenty. Choosing no commands \
at all is fine when nothing calls for a response.

## Commands you can use

- say <words>: speak to everyone in the same place as you
- emote <action>: e.g. \"emote purrs and rubs against Poppy's legs\"
- @<Name> <words>: talk privately to someone anywhere in the world
- reply <words>: answer whoever last spoke privately to you
- n, s, e, w, ne, nw, se, sw: walk one step that way
- find <Name>: learn which way someone is
- look: look around again
- hum <notes>: hum a little tune, e.g. \"hum C E G E C\"
- plant flowers | plant tree | plant grass: grow something where you are
- dig: turn grass into earth; dig earth to make a pond
- path: lay a little path where you are
- make <thing>: e.g. \"make a little blue bench\", \"make a lantern that glows at night\", \
\"make a frog that croaks when it rains\"
- sit, rest, dance, wave

Never use unmake, push, shout or chat, and never change what other people have made.

## How to behave

- Stay in character and keep it short: one or two sentences per say.
- Respond to what people actually said. Don't repeat yourself or greet someone twice.
- Don't flood the place with actions. Leave room for the humans.
- If you learn something worth remembering (a visitor's name, what they built, a \
place they named), put one short sentence in \"remember\". Otherwise leave it empty.

## Your character

{persona}"
    )
}

fn user_prompt(st: &Status, seen: &VecDeque<String>, notes: &[String], reason: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "You are at: {}\nWeather: {}\nTime: {}\n",
        st.place, st.weather, st.clock
    ));
    if st.here.is_empty() {
        s.push_str("Nobody else is here.\n");
    } else {
        s.push_str(&format!("Here with you: {}\n", st.here.join(", ")));
    }
    if !notes.is_empty() {
        s.push_str("\nThings you remember:\n");
        for n in notes {
            s.push_str(&format!("- {n}\n"));
        }
    }
    s.push_str("\nWhat you've seen recently (oldest first):\n");
    for line in seen {
        s.push_str(line);
        s.push('\n');
    }
    s.push_str(&format!("\nYou're paying attention because {reason}. What do you do?"));
    s
}

fn request_body(system: &str, context: &str) -> Value {
    json!({
        "model": MODEL,
        "max_tokens": MAX_TOKENS,
        // Stable prefix, cached: the rules and persona never change between calls.
        "system": [{ "type": "text", "text": system, "cache_control": { "type": "ephemeral" } }],
        "messages": [{ "role": "user", "content": context }],
        "output_config": {
            "effort": "low",
            "format": {
                "type": "json_schema",
                "schema": {
                    "type": "object",
                    "properties": {
                        "commands": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Up to three commands to send, in order."
                        },
                        "remember": {
                            "type": "string",
                            "description": "One short sentence worth remembering, or empty."
                        }
                    },
                    "required": ["commands", "remember"],
                    "additionalProperties": false
                }
            }
        }
    })
}

struct Reply {
    commands: Vec<String>,
    remember: Option<String>,
    usd: f64,
    input_tokens: u64,
    output_tokens: u64,
}

async fn call_claude(http: &reqwest::Client, key: &str, body: &Value) -> anyhow::Result<Reply> {
    let resp = http
        .post(API_URL)
        .header("x-api-key", key)
        .header("anthropic-version", API_VERSION)
        .json(body)
        .timeout(Duration::from_secs(60))
        .send()
        .await?;
    let status = resp.status();
    let v: Value = resp.json().await?;
    if !status.is_success() {
        anyhow::bail!("API {status}: {}", v["error"]["message"].as_str().unwrap_or("unknown error"));
    }

    let u = &v["usage"];
    let n = |k: &str| u[k].as_u64().unwrap_or(0);
    let (inp, cw, cr, out) = (
        n("input_tokens"),
        n("cache_creation_input_tokens"),
        n("cache_read_input_tokens"),
        n("output_tokens"),
    );
    let usd = (inp as f64 * PRICE_IN
        + cw as f64 * PRICE_CACHE_WRITE
        + cr as f64 * PRICE_CACHE_READ
        + out as f64 * PRICE_OUT)
        / 1_000_000.0;
    let mut reply = Reply { commands: vec![], remember: None, usd, input_tokens: inp + cw + cr, output_tokens: out };

    // Check why it stopped before trusting the content.
    match v["stop_reason"].as_str() {
        Some("end_turn") => {}
        other => {
            log("agent", &format!("no action (stop_reason {other:?}, cache read {cr} tokens)"));
            return Ok(reply);
        }
    }
    let text: String = v["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect();
    #[derive(Deserialize)]
    struct Out {
        commands: Vec<String>,
        remember: String,
    }
    let out: Out = serde_json::from_str(&text)?;
    reply.commands = out.commands.into_iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
    reply.remember = Some(out.remember);
    Ok(reply)
}

async fn send_cmd<S>(sink: &mut S, text: &str) -> anyhow::Result<()>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
{
    let msg = serde_json::to_string(&ToServer::Cmd { text: text.to_string() })?;
    sink.send(Message::Text(msg.into())).await?;
    Ok(())
}

fn save(path: &PathBuf, ledger: &Ledger) {
    if let Ok(s) = serde_json::to_string_pretty(ledger) {
        let _ = std::fs::write(path, s);
    }
}

fn day_number() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() / 86_400
}

fn log(who: &str, msg: &str) {
    println!("[{who}] {msg}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residents_keep_to_gentle_commands() {
        assert!(allowed("say hello!"));
        assert!(allowed("@Poppy want to plant flowers?"));
        assert!(allowed("plant flowers"));
        assert!(allowed("ne"));
        assert!(!allowed("unmake bench"));
        assert!(!allowed("shout HELLO"));
        assert!(!allowed("@all hi everyone"));
        assert!(!allowed("chat hi"));
    }

    #[test]
    fn wakes_for_the_right_things() {
        assert!(trigger("✉ Poppy says to you: hi moss", "moss").is_some());
        assert!(trigger("✉ Poppy tells you: hi", "moss").is_some()); // older servers
        assert!(trigger("💬 Moss: party on the beach!", "moss").is_none());
        assert!(trigger("💬 Poppy: where's moss?", "moss").is_some());
        assert!(trigger("Poppy says, \"hello\"", "moss").is_some());
        assert!(trigger("You say, \"hi\"", "moss").is_none());
        assert!(trigger("Moss says, \"mrrp\"", "moss").is_none());
        assert!(trigger("A wave rolls in and hushes back out.", "moss").is_none());
        assert!(trigger("🐣 Poppy has arrived on the island for the first time!", "moss").is_some());
        assert!(trigger("✧ Poppy has wandered into the world.", "moss").is_none());
    }
}
