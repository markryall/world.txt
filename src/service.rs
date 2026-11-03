//! Keep worlds up without a terminal: launchd agents (macOS) that start a
//! world and its Cloudflare tunnel at login and bring them back if they fall
//! over. Several worlds can run side by side, each with a name and a port.

use std::path::{Path, PathBuf};
use std::process::Command;

const PREFIX: &str = "txt.world.";

pub struct Install {
    pub db: String,
    pub url: Option<String>,
    pub backup_dir: Option<PathBuf>,
    /// The named Cloudflare tunnel to run alongside, if any.
    pub tunnel: Option<String>,
    /// Which world this is, when there's more than one (e.g. garden).
    pub name: Option<String>,
    pub port: u16,
}

/// One launchd agent: what it runs, where, and with what.
struct Agent {
    label: String,
    program: Vec<String>,
    dir: PathBuf,
    env: Vec<(&'static str, String)>,
    log: PathBuf,
}

/// A world's agents' labels: txt.world.serve for the unnamed one,
/// txt.world.garden.serve for one called garden.
fn label(name: Option<&str>, part: &str) -> String {
    match name {
        Some(n) => format!("{PREFIX}{n}.{part}"),
        None => format!("{PREFIX}{part}"),
    }
}

/// Which world a label belongs to (None for the unnamed one).
fn world_of(label: &str) -> Option<Option<String>> {
    let rest = label.strip_prefix(PREFIX)?;
    let (world, part) = match rest.rsplit_once('.') {
        Some((w, p)) => (Some(w.to_string()), p),
        None => (None, rest),
    };
    matches!(part, "serve" | "tunnel").then_some(world)
}

fn check_name(name: &str) -> anyhow::Result<()> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        anyhow::bail!("a world's service name is lowercase letters, numbers and dashes, e.g. garden");
    }
    Ok(())
}

fn home() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| anyhow::anyhow!("HOME isn't set"))
}

fn agents_dir() -> anyhow::Result<PathBuf> {
    Ok(home()?.join("Library/LaunchAgents"))
}

fn plist_path(label: &str) -> anyhow::Result<PathBuf> {
    Ok(agents_dir()?.join(format!("{label}.plist")))
}

fn log_dir(name: Option<&str>) -> anyhow::Result<PathBuf> {
    let base = home()?.join("Library/Logs/world.txt");
    Ok(match name {
        Some(n) => base.join(n),
        None => base,
    })
}

/// Where a world's own tunnel settings go, apart from ~/.cloudflared/config.yml
/// so worlds' tunnels never pick up each other's.
fn tunnel_config_path(name: Option<&str>) -> anyhow::Result<PathBuf> {
    Ok(home()?.join("Library/Application Support/world.txt").join(format!("{}.tunnel.yml", name.unwrap_or("world"))))
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
        label = xml(&a.label),
        dir = xml(&a.dir.to_string_lossy()),
    )
}

/// A tunnel's own settings: send this world's address to this world's port.
fn tunnel_config(tunnel_id: &str, credentials: &Path, hostname: &str, port: u16) -> String {
    format!(
        "tunnel: {tunnel_id}\ncredentials-file: {}\ningress:\n  - hostname: {hostname}\n    service: http://localhost:{port}\n  - service: http_status:404\n",
        credentials.display()
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
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    // SAFETY: getuid has no preconditions and can't fail.
    format!("gui/{}", unsafe { getuid() })
}

fn launchctl(args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("launchctl").args(args).output()
}

/// A named tunnel's id, asked of Cloudflare.
fn tunnel_id(cloudflared: &Path, tunnel: &str) -> anyhow::Result<String> {
    let out = Command::new(cloudflared).args(["tunnel", "list", "--output", "json", "--name", tunnel]).output()?;
    let list: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|_| anyhow::anyhow!("couldn't list tunnels: {}", String::from_utf8_lossy(&out.stderr).trim()))?;
    list.as_array()
        .and_then(|l| l.iter().find(|t| t["name"] == tunnel))
        .and_then(|t| t["id"].as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("there's no tunnel called {tunnel} (cloudflared tunnel create {tunnel})"))
}

fn agents(opts: &Install) -> anyhow::Result<Vec<Agent>> {
    let name = opts.name.as_deref();
    let db = std::path::absolute(&opts.db)?;
    if !db.exists() {
        anyhow::bail!("there's no world at {} (pass --db to point at it)", db.display());
    }
    let dir = db.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("/"));
    let logs = log_dir(name)?;
    let serve = vec![
        this_program()?.to_string_lossy().into_owned(),
        "serve".into(),
        "--db".into(),
        db.to_string_lossy().into_owned(),
        "--addr".into(),
        format!("0.0.0.0:{}", opts.port),
    ];
    let mut env = Vec::new();
    if let Some(u) = &opts.url {
        env.push(("WORLD_URL", u.clone()));
    } else {
        eprintln!("⚠️  no --url / WORLD_URL: invite links will need the address filled in");
    }
    if let Some(b) = &opts.backup_dir {
        env.push(("WORLD_BACKUP_DIR", std::path::absolute(b)?.to_string_lossy().into_owned()));
    }
    let mut out = vec![Agent { label: label(name, "serve"), program: serve, dir: dir.clone(), env, log: logs.join("serve.log") }];
    if let Some(t) = &opts.tunnel {
        let Some(url) = &opts.url else { anyhow::bail!("a tunnel needs the world's address: pass --url (or set WORLD_URL)") };
        let cf = on_path("cloudflared").ok_or_else(|| anyhow::anyhow!("cloudflared isn't on PATH (brew install cloudflared)"))?;
        let id = tunnel_id(&cf, t)?;
        let credentials = home()?.join(".cloudflared").join(format!("{id}.json"));
        if !credentials.exists() {
            anyhow::bail!("no credentials for tunnel {t} at {} (was it made on another machine?)", credentials.display());
        }
        let config = tunnel_config_path(name)?;
        std::fs::create_dir_all(config.parent().expect("a folder"))?;
        std::fs::write(&config, tunnel_config(&id, &credentials, &crate::server::federation::host(url), opts.port))?;
        out.push(Agent {
            label: label(name, "tunnel"),
            program: vec![
                cf.to_string_lossy().into_owned(),
                "tunnel".into(),
                "--config".into(),
                config.to_string_lossy().into_owned(),
                "run".into(),
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
    if let Some(n) = &opts.name {
        check_name(n)?;
    }
    // Another world already on this port would stop this one starting.
    for (world, port) in installed_worlds()? {
        if world != opts.name && port == Some(opts.port) {
            let who = world.map_or("the unnamed world".to_string(), |w| format!("the {w} world"));
            anyhow::bail!("{who} already uses port {}: pick another with --port", opts.port);
        }
    }
    let agents = agents(&opts)?;
    std::fs::create_dir_all(log_dir(opts.name.as_deref())?)?;
    std::fs::create_dir_all(agents_dir()?)?;
    let domain = domain();
    // Reinstalling without a tunnel: take away the old one.
    if opts.tunnel.is_none() {
        remove(&domain, &label(opts.name.as_deref(), "tunnel"))?;
    }
    for a in &agents {
        let path = plist_path(&a.label)?;
        // Replace any earlier version cleanly.
        let _ = launchctl(&["bootout", &format!("{domain}/{}", a.label)]);
        std::fs::write(&path, plist(a))?;
        let out = launchctl(&["bootstrap", &domain, &path.to_string_lossy()])?;
        if !out.status.success() {
            anyhow::bail!("launchctl couldn't start {}: {}", a.label, String::from_utf8_lossy(&out.stderr).trim());
        }
        println!("✨ {} is running and will start at login ({})", a.label, a.log.display());
    }
    let named = opts.name.as_deref().map(|n| format!(" --name {n}")).unwrap_or_default();
    println!("💡 world.txt service status to check on them, world.txt service uninstall{named} to stop");
    Ok(())
}

/// Stop an agent and remove its file. Whether there was one.
fn remove(domain: &str, label: &str) -> anyhow::Result<bool> {
    let path = plist_path(label)?;
    let _ = launchctl(&["bootout", &format!("{domain}/{label}")]);
    if path.exists() {
        std::fs::remove_file(&path)?;
        return Ok(true);
    }
    Ok(false)
}

/// Every installed agent's label.
fn installed() -> anyhow::Result<Vec<String>> {
    let Ok(dir) = std::fs::read_dir(agents_dir()?) else { return Ok(Vec::new()) };
    let mut labels: Vec<String> = dir
        .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".plist").map(str::to_string))
        .filter(|l| world_of(l).is_some())
        .collect();
    labels.sort();
    Ok(labels)
}

/// Installed worlds and the port each serves on.
fn installed_worlds() -> anyhow::Result<Vec<(Option<String>, Option<u16>)>> {
    let mut out = Vec::new();
    for l in installed()? {
        if !l.ends_with(".serve") && l != label(None, "serve") {
            continue;
        }
        let text = std::fs::read_to_string(plist_path(&l)?).unwrap_or_default();
        let port = text
            .split("<string>0.0.0.0:")
            .nth(1)
            .and_then(|r| r.split('<').next())
            .and_then(|p| p.parse().ok())
            .or(Some(4040));
        out.push((world_of(&l).flatten(), port));
    }
    Ok(out)
}

/// Stop one world (by name) or, with `all`, every world.
pub fn uninstall(name: Option<String>, all: bool) -> anyhow::Result<()> {
    if let Some(n) = &name {
        check_name(n)?;
    }
    let domain = domain();
    let labels: Vec<String> = if all {
        installed()?
    } else {
        vec![label(name.as_deref(), "serve"), label(name.as_deref(), "tunnel")]
    };
    let mut any = false;
    for l in labels {
        if remove(&domain, &l)? {
            any = true;
            println!("🌙 {l} stopped and removed");
        }
    }
    if !any {
        let which = name.map_or(String::new(), |n| format!(" called {n}"));
        println!("No world{which} was installed. world.txt service status lists them.");
    }
    Ok(())
}

pub fn status() -> anyhow::Result<()> {
    let labels = installed()?;
    if labels.is_empty() {
        println!("·  no worlds installed (world.txt service install)");
        return Ok(());
    }
    let domain = domain();
    let ports: Vec<_> = installed_worlds()?;
    let mut last_world = None;
    for l in labels {
        let world = world_of(&l).flatten();
        if last_world.as_ref() != Some(&world) {
            let port = ports.iter().find(|(w, _)| *w == world).and_then(|(_, p)| *p);
            let title = world.clone().unwrap_or_else(|| "(unnamed)".into());
            println!("🌍 {title}{}", port.map(|p| format!(" · port {p}")).unwrap_or_default());
            last_world = Some(world);
        }
        let out = launchctl(&["print", &format!("{domain}/{l}")])?;
        let text = String::from_utf8_lossy(&out.stdout);
        let field = |k: &str| text.lines().find_map(|line| line.trim().strip_prefix(k).map(|v| v.trim().to_string()));
        match (field("state = "), field("pid = ")) {
            (Some(s), Some(pid)) if s == "running" => println!("   🟢 {l}: running (pid {pid})"),
            (Some(s), _) => println!(
                "   🔴 {l}: {s}{}",
                field("last exit code = ").map(|c| format!(", last exit {c}")).unwrap_or_default()
            ),
            _ => println!("   🔴 {l}: installed but not loaded"),
        }
    }
    println!("📜 logs: {}", log_dir(None)?.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_runs_the_world_and_escapes_paths() {
        let a = Agent {
            label: label(None, "serve"),
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

    #[test]
    fn worlds_get_their_own_labels() {
        assert_eq!(label(Some("garden"), "tunnel"), "txt.world.garden.tunnel");
        assert_eq!(world_of("txt.world.serve"), Some(None));
        assert_eq!(world_of("txt.world.garden.serve"), Some(Some("garden".into())));
        assert_eq!(world_of("txt.world.garden.other"), None);
        assert_eq!(world_of("com.apple.thing"), None);
        assert!(check_name("my-garden2").is_ok());
        assert!(check_name("Garden").is_err() && check_name("a.b").is_err());
    }

    #[test]
    fn a_tunnel_sends_its_world_to_its_port() {
        let c = tunnel_config("abc", Path::new("/h/.cloudflared/abc.json"), "garden.world-txt.com", 4041);
        assert!(c.contains("tunnel: abc\ncredentials-file: /h/.cloudflared/abc.json\n"));
        assert!(c.contains("- hostname: garden.world-txt.com\n    service: http://localhost:4041\n"));
        assert!(c.ends_with("- service: http_status:404\n"));
    }
}
