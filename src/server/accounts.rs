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

fn random_u64() -> anyhow::Result<u64> {
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

/// The link to send. Without a known address, a placeholder the browser fills in.
pub fn link(url: Option<&str>, token: &str) -> String {
    match url {
        Some(u) => format!("{}/#invite={token}", u.trim_end_matches('/')),
        None => format!("<world address>/#invite={token}"),
    }
}
