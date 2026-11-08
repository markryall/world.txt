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
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
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
    if let Some(bytes) = db
        .meta("signing_key")
        .and_then(|h| unhex(&h))
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
    {
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
    /// Without a proper name, a world goes by its address.
    pub fn new(url: &str, key: &SigningKey, name: Option<String>) -> WorldInfo {
        WorldInfo {
            name: name.unwrap_or_else(|| host(url)),
            url: normalise(url),
            public_key: hex(key.verifying_key().as_bytes()),
        }
    }
}

/// Where a world's proper name is kept.
pub const NAME_KEY: &str = "world_name";

/// A world name fit to show: letters, numbers, spaces and a little punctuation.
pub fn clean_world_name(raw: &str) -> String {
    let kept: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || " -'.".contains(*c))
        .take(40)
        .collect();
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
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
    let (body_hex, sig_hex) = token
        .trim()
        .split_once('.')
        .ok_or_else(|| anyhow::anyhow!("that passport is torn"))?;
    let body = unhex(body_hex).ok_or_else(|| anyhow::anyhow!("that passport is smudged"))?;
    let sig = unhex(sig_hex)
        .and_then(|b| <[u8; 64]>::try_from(b).ok())
        .ok_or_else(|| anyhow::anyhow!("that passport's stamp is smudged"))?;
    let passport: Passport = serde_json::from_slice(&body)?;
    if normalise(&passport.dest) != normalise(my_url) {
        anyhow::bail!("that passport is for a different world");
    }
    if passport.expires < now {
        anyhow::bail!("that passport has run out: sail again from your home world");
    }
    // Ask their home world for its key. Only http(s), and only this path.
    let home = normalise(&passport.home);
    let trying_out = normalise(my_url).starts_with("http://");
    if !(home.starts_with("https://") || (home.starts_with("http://") && trying_out)) {
        anyhow::bail!("that passport comes from a world we can't check");
    }
    // Anyone can hand over a passport before they're let in, so the details
    // of what went wrong stay in the log rather than going back to them.
    let info = home_info(&home, trying_out).await.map_err(|e| {
        eprintln!("🛂 couldn't check a passport from {home}: {e:#}");
        anyhow::anyhow!("we couldn't reach your home world to check your passport")
    })?;
    if normalise(&info.url) != home {
        anyhow::bail!("their home world doesn't recognise its own address");
    }
    let key_bytes = unhex(&info.public_key)
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
        .ok_or_else(|| anyhow::anyhow!("their home world's key is odd"))?;
    VerifyingKey::from_bytes(&key_bytes)?
        .verify(&body, &Signature::from_bytes(&sig))
        .map_err(|_| anyhow::anyhow!("that passport's stamp doesn't match its home world"))?;
    Ok(passport)
}

/// The most a home world's /.well-known/world.txt may say.
const INFO_MAX: usize = 16 * 1024;
/// How many passports may be checked at once.
const CHECKING_AT_ONCE: usize = 8;
static CHECKING: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(CHECKING_AT_ONCE);

/// Fetch a home world's /.well-known/world.txt. A stranger chooses the
/// address, so it must be out on the internet, not somewhere only this
/// server can reach (its own machine, the network it's hosted on), unless
/// this world is only being tried out locally itself. The address is looked
/// up once and the request pinned to it, so it can't change in between, and
/// redirects aren't followed.
async fn home_info(home: &str, local_ok: bool) -> anyhow::Result<WorldInfo> {
    let _turn = CHECKING
        .try_acquire()
        .map_err(|_| anyhow::anyhow!("too many passports being checked at once"))?;
    let url = reqwest::Url::parse(&format!("{home}{WELL_KNOWN}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow::anyhow!("no host"))?
        .to_string();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| anyhow::anyhow!("no port"))?;
    let lookup = host
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_string();
    let addrs: Vec<std::net::SocketAddr> = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::net::lookup_host((lookup.as_str(), port)),
    )
    .await??
    .collect();
    if addrs.is_empty() {
        anyhow::bail!("{host} has no address");
    }
    if let Some(a) = addrs.iter().find(|a| !local_ok && !public(a.ip())) {
        anyhow::bail!("{host} is at {}, which isn't out on the internet", a.ip());
    }
    let client = reqwest::Client::builder()
        .resolve_to_addrs(&lookup, &addrs)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(8))
        .build()?;
    let mut res = client.get(url).send().await?.error_for_status()?;
    let mut body = Vec::new();
    while let Some(chunk) = res.chunk().await? {
        body.extend_from_slice(&chunk);
        if body.len() > INFO_MAX {
            anyhow::bail!("its world.txt is far too long");
        }
    }
    Ok(serde_json::from_slice(&body)?)
}

/// Whether an address is out on the public internet: not this machine, a
/// private or link-local network, or anything reserved.
fn public(ip: std::net::IpAddr) -> bool {
    use std::net::IpAddr;
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                || a == 0
                || a >= 240
                || (a == 100 && (64..128).contains(&b)) // shared address space
                || (a == 192 && b == 0 && v4.octets()[2] == 0)
                || (a == 198 && (18..20).contains(&b))) // benchmarking
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return public(IpAddr::V4(v4));
            }
            let seg = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || (seg[0] == 0x2001 && seg[1] == 0x0db8) // documentation
                || (seg[0] == 0x0064 && seg[1] == 0xff9b) // NAT64, an IPv4 address in disguise
                || seg[0] == 0) // other IPv4-compatible and reserved forms
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_world_goes_by_its_name_or_else_its_address() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        assert_eq!(
            WorldInfo::new("https://Home.Example.com/", &key, None).name,
            "home.example.com"
        );
        let named = WorldInfo::new("https://home.example.com", &key, Some("Willow Bank".into()));
        assert_eq!(
            (named.name.as_str(), named.url.as_str()),
            ("Willow Bank", "https://home.example.com")
        );
        assert_eq!(
            clean_world_name("  Willow   <b>Bank</b>! "),
            "Willow bBankb"
        );
    }

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

    #[test]
    fn only_public_addresses_count() {
        let p = |a: &str| public(a.parse().unwrap());
        for private in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "::1",
            "fdaa::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "64:ff9b::a00:1",
            "::",
        ] {
            assert!(!p(private), "{private} isn't public");
        }
        for open in ["93.184.216.34", "1.1.1.1", "2606:4700::1111"] {
            assert!(p(open), "{open} is public");
        }
    }

    #[tokio::test]
    async fn home_worlds_must_be_out_on_the_internet() {
        let e = home_info("http://127.0.0.1:9", false)
            .await
            .unwrap_err()
            .to_string();
        assert!(e.contains("isn't out on the internet"), "{e}");
        let e = home_info("https://[::1]", false)
            .await
            .unwrap_err()
            .to_string();
        assert!(e.contains("isn't out on the internet"), "{e}");
    }

    #[tokio::test]
    async fn a_home_world_that_says_too_much_is_cut_off() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut c, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = c.read(&mut buf).await;
            let _ = c
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\r\n")
                .await;
            let junk = vec![b' '; 64 * 1024];
            let _ = c.write_all(&junk).await;
        });
        let e = home_info(&format!("http://{addr}"), true)
            .await
            .unwrap_err()
            .to_string();
        assert!(e.contains("far too long"), "{e}");
    }
}
