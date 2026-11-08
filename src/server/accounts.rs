//! Invite-only accounts. A keeper (or the server machine) creates someone's
//! account and gets a one-use magic link; opening it swaps it for a device
//! token kept on that device, which is how they get in from then on.

use super::db::Db;
use std::io::Read;

pub const INVITE_DAYS: i64 = 7;

/// 16 random bytes as hex, for invite links and device tokens.
pub fn new_secret() -> anyhow::Result<String> {
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

pub fn random_u64() -> anyhow::Result<u64> {
    let mut bytes = [0u8; 8];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Make an account with a fresh username. Returns the username.
pub fn create_account(db: &Db, nickname: &str, keeper: bool) -> anyhow::Result<String> {
    let user = loop {
        let u = super::names::username(random_u64()?);
        if db.resolve_user(&u).is_none() {
            break u;
        }
    };
    // Off the map until they first arrive, when they're put at the spawn point.
    db.create_user(&user, nickname, -1, -1)?;
    if keeper {
        db.set_keeper(&user, true)?;
    }
    Ok(user)
}

/// A one-use invite for someone's next device. Returns the secret.
pub fn issue_invite(db: &Db, user: &str) -> anyhow::Result<String> {
    let token = new_secret()?;
    db.add_invite(user, &super::names::hash(&token), now() + INVITE_DAYS * 86_400)?;
    Ok(token)
}

/// How to use a magic link, in a browser or a terminal, to go with it.
pub fn how_to_join(url: Option<&str>, token: &str) -> Vec<String> {
    let server = match url.map(|u| u.trim_end_matches('/')) {
        Some(u) if u.starts_with("https://") => format!("wss://{}", &u["https://".len()..]),
        Some(u) if u.starts_with("http://") => format!("ws://{}", &u["http://".len()..]),
        Some(u) => u.to_string(),
        None => "<world address>".to_string(),
    };
    vec![
        "In a browser (phones too): just open the link.".to_string(),
        format!("In a terminal (brew install markryall/tap/world-txt): world.txt play --server {server} --invite {token}"),
        format!("  and after that, world.txt play --server {server} is all it takes."),
    ]
}

/// The link to send. Without a known address, a placeholder the browser fills in.
pub fn link(url: Option<&str>, token: &str) -> String {
    match url {
        Some(u) => format!("{}/#invite={token}", u.trim_end_matches('/')),
        None => format!("<world address>/#invite={token}"),
    }
}

/// How an invite stands, for keepers: "used 2h ago", "waiting (6d left)", "expired".
pub fn invite_state(expires: i64, used_at: Option<i64>, now: i64) -> String {
    fn ago(s: i64) -> String {
        match s {
            s if s < 3600 => "just now".into(),
            s if s < 86_400 => format!("{}h ago", s / 3600),
            s => format!("{}d ago", s / 86_400),
        }
    }
    match used_at {
        Some(0) => "used".into(),
        Some(t) => format!("used {}", ago(now - t)),
        None if expires <= now => "expired".into(),
        None => {
            let left = expires - now;
            if left < 86_400 { format!("waiting ({}h left)", (left / 3600).max(1)) } else { format!("waiting ({}d left)", left / 86_400) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn says_how_to_join_from_a_terminal() {
        let how = how_to_join(Some("https://world.example.com/"), "abc123").join("\n");
        assert!(how.contains("world.txt play --server wss://world.example.com --invite abc123"), "{how}");
        assert!(how.contains("open the link"));
        assert!(how_to_join(Some("http://127.0.0.1:4040"), "x")[1].contains("--server ws://127.0.0.1:4040 "));
        assert!(how_to_join(None, "x")[1].contains("--server <world address> "));
    }
}
