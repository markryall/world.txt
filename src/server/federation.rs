//! Visiting other worlds. Each world has a signing key and publishes its
//! public key at /.well-known/world.txt. A world signs short-lived passports
//! for its people; the world they visit checks the signature against the
//! home world's published key.

use super::db::Db;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::io::Read;

pub const WELL_KNOWN: &str = "/.well-known/world.txt";
/// A passport is good for this long, and only once.
pub const PASSPORT_SECS: i64 = 10 * 60;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// "https://World.Example.com/" → "https://world.example.com"
pub fn normalise(url: &str) -> String {
    url.trim().trim_end_matches('/').to_lowercase()
}

/// "https://world.example.com" → "world.example.com"
pub fn host(url: &str) -> String {
    let u = normalise(url);
    u.split("://").nth(1).unwrap_or(&u).to_string()
}

/// This world's signing key, made the first time it's needed.
pub fn world_key(db: &Db) -> anyhow::Result<SigningKey> {
    if let Some(bytes) = db.meta("signing_key").and_then(|h| unhex(&h)).and_then(|b| <[u8; 32]>::try_from(b).ok()) {
        return Ok(SigningKey::from_bytes(&bytes));
    }
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    db.set_meta("signing_key", &hex(&bytes))?;
    Ok(SigningKey::from_bytes(&bytes))
}

/// What /.well-known/world.txt says about this world.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldInfo {
    pub name: String,
    pub url: String,
    pub public_key: String,
}

impl WorldInfo {
    pub fn new(url: &str, key: &SigningKey) -> WorldInfo {
        WorldInfo { name: host(url), url: normalise(url), public_key: hex(key.verifying_key().as_bytes()) }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Passport {
    /// Their username at home, e.g. sunny-otter-42.
    pub user: String,
    pub nickname: String,
    /// Their home world's address.
    pub home: String,
    /// Where they're going.
    pub dest: String,
    pub expires: i64,
    pub nonce: String,
}

impl Passport {
    /// How they're known away from home: "sunny-otter-42@ryall.island".
    pub fn visitor_name(&self) -> String {
        format!("{}@{}", self.user, host(&self.home))
    }

    pub fn sign(&self, key: &SigningKey) -> anyhow::Result<String> {
        let body = serde_json::to_vec(self)?;
        let sig = key.sign(&body);
        Ok(format!("{}.{}", hex(&body), hex(&sig.to_bytes())))
    }
}

/// Check a passport presented to the world at `my_url`: it's for here, in
/// date, and signed by the key its home world publishes.
pub async fn verify(token: &str, my_url: &str, now: i64) -> anyhow::Result<Passport> {
    let (body_hex, sig_hex) = token.trim().split_once('.').ok_or_else(|| anyhow::anyhow!("that passport is torn"))?;
    let body = unhex(body_hex).ok_or_else(|| anyhow::anyhow!("that passport is smudged"))?;
    let sig = unhex(sig_hex).and_then(|b| <[u8; 64]>::try_from(b).ok()).ok_or_else(|| anyhow::anyhow!("that passport's stamp is smudged"))?;
    let passport: Passport = serde_json::from_slice(&body)?;
    if normalise(&passport.dest) != normalise(my_url) {
        anyhow::bail!("that passport is for a different world");
    }
    if passport.expires < now {
        anyhow::bail!("that passport has run out: sail again from your home world");
    }
    // Ask their home world for its key. Only http(s), and only this path.
    let home = normalise(&passport.home);
    if !(home.starts_with("https://") || (home.starts_with("http://") && normalise(my_url).starts_with("http://"))) {
        anyhow::bail!("that passport comes from a world we can't check");
    }
    let info: WorldInfo = reqwest::Client::new()
        .get(format!("{home}{WELL_KNOWN}"))
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if normalise(&info.url) != home {
        anyhow::bail!("their home world doesn't recognise its own address");
    }
    let key_bytes = unhex(&info.public_key).and_then(|b| <[u8; 32]>::try_from(b).ok()).ok_or_else(|| anyhow::anyhow!("their home world's key is odd"))?;
    VerifyingKey::from_bytes(&key_bytes)?
        .verify(&body, &Signature::from_bytes(&sig))
        .map_err(|_| anyhow::anyhow!("that passport's stamp doesn't match its home world"))?;
    Ok(passport)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passports_round_trip_and_tampering_shows() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let p = Passport {
            user: "sunny-otter-42".into(),
            nickname: "Poppy".into(),
            home: "https://ryall.island".into(),
            dest: "https://other.world".into(),
            expires: 100,
            nonce: "n".into(),
        };
        let token = p.sign(&key).unwrap();
        let (body, sig) = token.split_once('.').unwrap();
        let ok = VerifyingKey::from_bytes(key.verifying_key().as_bytes()).unwrap();
        let sig = Signature::from_bytes(&<[u8; 64]>::try_from(unhex(sig).unwrap()).unwrap());
        assert!(ok.verify(&unhex(body).unwrap(), &sig).is_ok());
        let mut forged = unhex(body).unwrap();
        forged[10] ^= 1;
        assert!(ok.verify(&forged, &sig).is_err());
        assert_eq!(p.visitor_name(), "sunny-otter-42@ryall.island");
        assert_eq!(normalise("HTTPS://Other.World/"), "https://other.world");
    }
}
