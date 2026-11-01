//! `world.txt bridge` and `world.txt send`: let something that can't hold a
//! websocket open (like a Claude Code session) be an island resident.
//!
//! The bridge stays connected and appends everything it sees to
//! `<dir>/<name>.log`. Lines that deserve attention start with `! `, other
//! world text with `  `, and the bridge's own notices with `# `. `send`
//! passes one command through a unix socket and prints what came back.

use crate::agent::trigger;
use crate::protocol::{ToClient, ToServer};
use futures_util::{SinkExt, StreamExt};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::tungstenite::Message;

/// How long `send` keeps listening for the world's response.
const ECHO_WINDOW: Duration = Duration::from_millis(1200);

pub fn paths(dir: &Path, name: &str) -> (PathBuf, PathBuf) {
    let n = name.to_lowercase();
    (dir.join(format!("{n}.log")), dir.join(format!("{n}.sock")))
}

pub async fn run(name: String, server: &str, password: Option<String>, dir: PathBuf, invite: Option<String>) -> anyhow::Result<()> {
    std::fs::create_dir_all(&dir)?;
    let (log_path, sock_path) = paths(&dir, &name);
    let _ = std::fs::remove_file(&sock_path);
    let mut log = std::fs::OpenOptions::new().create(true).append(true).open(&log_path)?;
    let note = |log: &mut std::fs::File, s: &str| {
        let _ = writeln!(log, "# {s}");
        let _ = log.flush();
        println!("# {s}");
    };

    let ws = connect_retry(server).await?;
    let (mut sink, mut source) = ws.split();
    let who = name.to_lowercase();
    let hello = crate::identity::hello(&who, server, password, true, invite);
    sink.send(Message::Text(serde_json::to_string(&hello)?.into())).await?;

    let listener = UnixListener::bind(&sock_path)?;
    note(&mut log, &format!("bridge up: {name} joined {server}; log {}", log_path.display()));

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<String>();
    let (lines_tx, _) = broadcast::channel::<String>(256);
    let me = name.to_lowercase();
    let mut humans: Option<u32> = None;

    loop {
        tokio::select! {
            msg = source.next() => {
                let text = match msg {
                    Some(Ok(Message::Text(t))) => t,
                    Some(Ok(_)) => continue,
                    _ => { note(&mut log, "disconnected from the world"); break }
                };
                match serde_json::from_str::<ToClient>(text.as_str()) {
                    Ok(ToClient::Lines { lines }) => {
                        for line in lines {
                            let t: String = line.0.iter().map(|s| s.t.as_str()).collect();
                            if t.trim().is_empty() { continue }
                            let mark = if trigger(&t, &me).is_some() { "! " } else { "  " };
                            let _ = writeln!(log, "{mark}{t}");
                            let _ = lines_tx.send(t);
                        }
                        let _ = log.flush();
                    }
                    Ok(ToClient::Status { humans: h, .. }) => {
                        if humans != Some(h) {
                            match (humans, h) {
                                (_, 0) => note(&mut log, "no humans online: rest quietly"),
                                (None | Some(0), n) => note(&mut log, &format!("humans online: {n}")),
                                _ => {}
                            }
                            humans = Some(h);
                        }
                    }
                    Ok(msg @ ToClient::Bye { .. }) => {
                        crate::identity::on_sign_in(&who, server, &msg);
                        if let ToClient::Bye { reason } = msg { note(&mut log, &format!("bye: {reason}")); }
                        break
                    }
                    Ok(msg @ ToClient::Welcome { .. }) => crate::identity::on_sign_in(&who, server, &msg),
                    Ok(msg @ ToClient::Board { .. }) => {
                        // Shown as plain lines, so `send <Name> read` prints the board.
                        for line in crate::protocol::board_lines(&msg).unwrap_or_default() {
                            let t: String = line.0.iter().map(|s| s.t.as_str()).collect();
                            let _ = writeln!(log, "  {t}");
                            let _ = lines_tx.send(t);
                        }
                        let _ = log.flush();
                    }
                    Ok(ToClient::Hint { .. }) | Err(_) => {}
                }
            }
            Some(cmd) = cmd_rx.recv() => {
                let _ = writeln!(log, "> {cmd}");
                let msg = serde_json::to_string(&ToServer::Cmd { text: cmd })?;
                if sink.send(Message::Text(msg.into())).await.is_err() {
                    note(&mut log, "disconnected from the world");
                    break;
                }
            }
            conn = listener.accept() => {
                let Ok((stream, _)) = conn else { continue };
                let cmd_tx = cmd_tx.clone();
                let rx = lines_tx.subscribe();
                tokio::spawn(async move { let _ = handle_send(stream, cmd_tx, rx).await; });
            }
        }
    }
    let _ = std::fs::remove_file(&sock_path);
    Ok(())
}

async fn handle_send(
    stream: UnixStream,
    cmd_tx: mpsc::UnboundedSender<String>,
    mut lines: broadcast::Receiver<String>,
) -> anyhow::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut cmd = String::new();
    BufReader::new(read).read_line(&mut cmd).await?;
    let cmd = cmd.trim().to_string();
    if cmd.is_empty() {
        return Ok(());
    }
    cmd_tx.send(cmd)?;
    let deadline = tokio::time::Instant::now() + ECHO_WINDOW;
    while let Ok(Ok(line)) = tokio::time::timeout_at(deadline, lines.recv()).await {
        write.write_all(line.as_bytes()).await?;
        write.write_all(b"\n").await?;
    }
    write.shutdown().await?;
    Ok(())
}

pub async fn send(name: &str, dir: &Path, words: &[String]) -> anyhow::Result<()> {
    let (_, sock_path) = paths(dir, name);
    let mut stream = UnixStream::connect(&sock_path).await.map_err(|e| {
        anyhow::anyhow!("no bridge for {name} at {} ({e}). Start one with: world.txt bridge {name}", sock_path.display())
    })?;
    let cmd = words.join(" ").replace('\n', " ");
    stream.write_all(format!("{cmd}\n").as_bytes()).await?;
    let mut out = String::new();
    tokio::io::AsyncReadExt::read_to_string(&mut stream, &mut out).await?;
    print!("{out}");
    Ok(())
}

/// Connect, retrying for a little while in case the world is still starting.
pub async fn connect_retry(
    server: &str,
) -> anyhow::Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>> {
    let mut tries = 0;
    loop {
        match tokio_tungstenite::connect_async(server).await {
            Ok((ws, _)) => return Ok(ws),
            Err(e) if tries < 20 => {
                tries += 1;
                if tries == 1 {
                    eprintln!("waiting for the world at {server} ({e})...");
                }
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}
