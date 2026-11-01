//! Names belong to the device that first used them. Each client keeps a
//! secret key and sends it when joining; the world keeps only a hash of it.

use sha2::{Digest, Sha256};

#[derive(Debug, PartialEq, Eq)]
pub enum NameCheck {
    /// The right key, or a name nobody has claimed and no key to claim it with.
    Welcome,
    /// Nobody has this name yet: claim it with this hash.
    Claim(String),
    /// Someone else's name. Holds the hash of the key that was tried, so a
    /// keeper can welcome that device in.
    Taken(Option<String>),
}

/// A cute, never-changing username, e.g. "sunny-otter-42".
pub fn username(roll: u64) -> String {
    const ADJ: &[&str] = &[
        "sunny", "misty", "mossy", "sleepy", "bouncy", "gentle", "fuzzy", "breezy", "starry", "rosy", "cosy", "twinkly",
        "dappled", "golden", "silver", "velvet", "merry", "quiet", "brave", "curious", "sparkly", "dreamy", "plucky", "cheery",
    ];
    const ANIMAL: &[&str] = &[
        "otter", "fox", "wren", "hedgehog", "badger", "rabbit", "owl", "duckling", "heron", "moth", "newt", "vole",
        "robin", "finch", "seal", "puffin", "turtle", "squirrel", "lamb", "kitten", "panda", "koala", "frog", "bee",
    ];
    let a = ADJ[(roll % ADJ.len() as u64) as usize];
    let b = ANIMAL[((roll / 31) % ANIMAL.len() as u64) as usize];
    format!("{a}-{b}-{}", 10 + (roll / 977) % 90)
}

/// "Poppy (sunny-otter-42)"
pub fn full(nickname: &str, user: &str) -> String {
    if nickname.eq_ignore_ascii_case(user) { user.to_string() } else { format!("{nickname} ({user})") }
}

/// A short code for a knocking device to read out to a keeper, e.g. "FROG-42".
pub fn knock_code(roll: u64) -> String {
    const WORDS: &[&str] = &[
        "FROG", "BEE", "MOSS", "FERN", "MOTH", "WREN", "OWL", "SHELL", "DAISY", "POPPY", "PEARL", "CLOUD",
        "LEAF", "STAR", "SNAIL", "OTTER", "MAPLE", "ACORN", "CORAL", "BERRY",
    ];
    let word = WORDS[(roll % WORDS.len() as u64) as usize];
    format!("{word}-{}", 10 + (roll / 97) % 90)
}

pub fn hash(key: &str) -> String {
    Sha256::digest(key.trim().as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn check(stored: Option<&str>, given: Option<&str>) -> NameCheck {
    let given = given.map(str::trim).filter(|k| !k.is_empty());
    match (stored, given) {
        (Some(h), Some(k)) if hash(k) == h => NameCheck::Welcome,
        (Some(_), k) => NameCheck::Taken(k.map(hash)),
        (None, Some(k)) => NameCheck::Claim(hash(k)),
        (None, None) => NameCheck::Welcome,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_are_cute_and_varied() {
        let names: std::collections::HashSet<String> = (0..500u64).map(|i| username(crate::server::rng::hash(i))).collect();
        assert!(names.len() > 450, "too many repeats: {}", names.len());
        let u = username(12345);
        let parts: Vec<&str> = u.split('-').collect();
        assert_eq!(parts.len(), 3, "{u}");
        assert!(parts[2].parse::<u32>().is_ok_and(|n| (10..100).contains(&n)), "{u}");
        assert_eq!(full("Poppy", "sunny-otter-42"), "Poppy (sunny-otter-42)");
    }

    #[test]
    fn first_device_claims_then_only_it_gets_in() {
        let claim = check(None, Some("pink-key"));
        let NameCheck::Claim(h) = claim else { panic!("should claim") };
        assert_eq!(check(Some(&h), Some("pink-key")), NameCheck::Welcome);
        assert_eq!(check(Some(&h), Some("other-key")), NameCheck::Taken(Some(hash("other-key"))));
        assert_eq!(check(Some(&h), None), NameCheck::Taken(None));
        assert_eq!(check(None, None), NameCheck::Welcome);
        assert_eq!(check(None, Some("  ")), NameCheck::Welcome);
    }
}
