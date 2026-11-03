//! Keep the world up without a terminal: launchd agents (macOS) that start the
//! world and its Cloudflare tunnel at login and bring them back if they fall over.

use std::path::{Path, PathBuf};
use std::process::Command;

const SERVE: &str = "txt.world.serve";
const TUNNEL: &str = "txt.world.tunnel";

pub struct Install {
    pub db: String,
    pub url: Option<String>,
    pub backup_dir: Option<PathBuf>,
    /// The named Cloudflare tunnel to run alongside, if any.
    pub tunnel: Option<String>,
}

/// One launchd agent: what it runs, where, and with what.
struct Agent {
    label: &'static str,
    program: Vec<String>,
    dir: PathBuf,
    env: Vec<(&'static str, String)>,
    log: PathBuf,
}

fn home() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| anyhow::anyhow!("HOME isn't set"))
}

fn plist_path(label: &str) -> anyhow::Result<PathBuf> {
    Ok(home()?.join("Library/LaunchAgents").join(format!("{label}.plist")))
}

fn log_dir() -> anyhow::Result<PathBuf> {
    Ok(home()?.join("Library/Logs/world.txt"))
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn plist(a: &Agent) -> String {
    let args: String = a.program.iter().map(|p| format!("    <string>{}</string>\n", xml(p))).collect();
    let env: String = a
        .env
        .iter()
        .map(|(k, v)| format!("    <key>{k}</key>\n    <string>{}</string>\n", xml(v)))
        .collect();
    let log = xml(&a.log.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label}</string>
  <key>ProgramArguments</key>
  <array>
{args}  </array>
  <key>WorkingDirectory</key>
  <string>{dir}</string>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string>
{env}  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>ThrottleInterval</key>
  <integer>10</integer>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
</dict>
</plist>
"#,
        label = a.label,
        dir = xml(&a.dir.to_string_lossy()),
    )
}

/// Find a program on PATH without following symlinks, so a Homebrew upgrade
/// (which moves the real file) doesn't strand the agent.
fn on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")?.to_str()?.split(':').map(|d| Path::new(d).join(name)).find(|p| p.is_file())
}

/// This program, as the agent should start it.
fn this_program() -> anyhow::Result<PathBuf> {
    let arg0 = std::env::args().next().unwrap_or_default();
    if !arg0.contains('/') {
        if let Some(p) = on_path(&arg0) {
            return Ok(p);
        }
    }
    Ok(std::env::current_exe()?)
}

fn domain() -> String {
    // SAFETY: getuid has no preconditions and can't fail.
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    format!("gui/{}", unsafe { getuid() })
}

fn launchctl(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("launchctl").args(args).output()
}

fn agents(opts: &Install) -> anyhow::Result<Vec<Agent>> {
    let db = std::path::absolute(&opts.db)?;
    if !db.exists() {
        anyhow::bail!("there's no world at {} (pass --db to point at it)", db.display());
    }
    let dir = db.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("/"));
    let logs = log_dir()?;
    let serve = vec![this_program()?.to_string_lossy().into_owned(), "serve".into(), "--db".into(), db.to_string_lossy().into_owned()];
    let mut env = Vec::new();
    if let Some(u) = &opts.url {
        env.push(("WORLD_URL", u.clone()));
    }
    if let Some(b) = &opts.backup_dir {
        env.push(("WORLD_BACKUP_DIR", std::path::absolute(b)?.to_string_lossy().into_owned()));
    }
    if opts.url.is_none() {
        // Without an address the links come out as placeholders; say so, but carry on.
        eprintln!("⚠️  no --url / WORLD_URL: invite links will need the address filled in");
    }
    let mut out = vec![Agent { label: SERVE, program: serve, dir: dir.clone(), env, log: logs.join("serve.log") }];
    if let Some(t) = &opts.tunnel {
        let cf = on_path("cloudflared").ok_or_else(|| anyhow::anyhow!("cloudflared isn't on PATH (brew install cloudflared)"))?;
        out.push(Agent {
            label: TUNNEL,
            program: vec![
                cf.to_string_lossy().into_owned(),
                "tunnel".into(),
                "run".into(),
                "--url".into(),
                "http://localhost:4040".into(),
                t.clone(),
            ],
            dir,
            env: Vec::new(),
            log: logs.join("tunnel.log"),
        });
    }
    Ok(out)
}

pub fn install(opts: Install) -> anyhow::Result<()> {
    if !cfg!(target_os = "macos") {
        anyhow::bail!("service install only knows macOS (launchd) so far; on Linux, a systemd unit running `world.txt serve` does the same");
    }
    let agents = agents(&opts)?;
    std::fs::create_dir_all(log_dir()?)?;
    std::fs::create_dir_all(home()?.join("Library/LaunchAgents"))?;
    let domain = domain();
    for a in &agents {
        let path = plist_path(a.label)?;
        // Replace any earlier version cleanly.
        let _ = launchctl(&["bootout", &format!("{domain}/{}", a.label)]);
        std::fs::write(&path, plist(a))?;
        let out = launchctl(&["bootstrap", &domain, &path.to_string_lossy()])?;
        if !out.status.success() {
            anyhow::bail!("launchctl couldn't start {}: {}", a.label, String::from_utf8_lossy(&out.stderr).trim());
        }
        println!("✨ {} is running and will start at login ({})", a.label, a.log.display());
    }
    println!("💡 world.txt service status to check on them, world.txt service uninstall to stop");
    Ok(())
}

pub fn uninstall() -> anyhow::Result<()> {
    let domain = domain();
    for label in [SERVE, TUNNEL] {
        let path = plist_path(label)?;
        let _ = launchctl(&["bootout", &format!("{domain}/{label}")]);
        if path.exists() {
            std::fs::remove_file(&path)?;
            println!("🌙 {label} stopped and removed");
        }
    }
    Ok(())
}

pub fn status() -> anyhow::Result<()> {
    let domain = domain();
    for label in [SERVE, TUNNEL] {
        if !plist_path(label)?.exists() {
            println!("·  {label}: not installed");
            continue;
        }
        let out = launchctl(&["print", &format!("{domain}/{label}")])?;
        let text = String::from_utf8_lossy(&out.stdout);
        let field = |k: &str| text.lines().find_map(|l| l.trim().strip_prefix(k).map(|v| v.trim().to_string()));
        match (field("state = "), field("pid = ")) {
            (Some(s), Some(pid)) if s == "running" => println!("🟢 {label}: running (pid {pid})"),
            (Some(s), _) => println!(
                "🔴 {label}: {s}{}",
                field("last exit code = ").map(|c| format!(", last exit {c}")).unwrap_or_default()
            ),
            _ => println!("🔴 {label}: installed but not loaded"),
        }
    }
    println!("📜 logs: {}", log_dir()?.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_runs_the_world_and_escapes_paths() {
        let a = Agent {
            label: SERVE,
            program: vec!["/bin/world.txt".into(), "serve".into(), "--db".into(), "/w & co/world.db".into()],
            dir: "/w & co".into(),
            env: vec![("WORLD_URL", "https://home.example.com".into())],
            log: "/logs/serve.log".into(),
        };
        let p = plist(&a);
        assert!(p.contains("<string>txt.world.serve</string>"));
        assert!(p.contains("<string>/w &amp; co/world.db</string>"));
        assert!(p.contains("<key>WORLD_URL</key>\n    <string>https://home.example.com</string>"));
        assert!(p.contains("<key>KeepAlive</key>\n  <true/>"));
        assert!(!p.contains("/w & co"));
    }
}
