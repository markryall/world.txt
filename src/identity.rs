//! Signing in from this machine: the terminal client, residents and bridges.

use std::path::PathBuf;


// ---- device tokens ----------------------------------------------------------
//
// Opening an invite gives this device a token for that account on that
// server. They're kept one per (who, server) in ~/.config/world.txt/tokens/,
// where `who` is "player" for the terminal client or a resident's name.

fn token_path(who: &str, server: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let label: String = format!("{who}@{server}")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '@' || c == '-' { c.to_ascii_lowercase() } else { '_' })
        .collect();
    Some(PathBuf::from(home).join(".config").join("world.txt").join("tokens").join(label))
}

pub fn load_token(who: &str, server: &str) -> Option<String> {
    let t = std::fs::read_to_string(token_path(who, server)?).ok()?;
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

pub fn save_token(who: &str, server: &str, token: &str) {
    let Some(path) = token_path(who, server) else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::write(&path, token).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

pub fn forget_token(who: &str, server: &str) {
    if let Some(path) = token_path(who, server) {
        let _ = std::fs::remove_file(path);
    }
}

/// The hello for `who` on `server`: the invite if there is one, otherwise
/// this device's saved token.
pub fn hello(who: &str, server: &str, bot: bool, invite: Option<String>) -> crate::protocol::ToServer {
    // Accept a whole magic link as well as just its code.
    let invite = invite.map(|i| i.rsplit("invite=").next().unwrap_or(&i).split('&').next().unwrap_or("").trim().to_string());
    let token = if invite.is_some() { None } else { load_token(who, server) };
    crate::protocol::ToServer::Hello { name: who.to_string(), bot, invite, token, passport: None }
}

/// What to do with what the world said about signing in.
pub fn on_sign_in(who: &str, server: &str, msg: &crate::protocol::ToClient) {
    match msg {
        crate::protocol::ToClient::Welcome { token: Some(t), .. } => save_token(who, server, t),
        crate::protocol::ToClient::Bye { reason } if reason.contains("isn't signed in") => forget_token(who, server),
        _ => {}
    }
}
