//! Usernames, and hashing the secrets in magic links and device tokens.

use sha2::{Digest, Sha256};

/// A cute, never-changing username, e.g. "sunny-otter-42".
pub fn username(roll: u64) -> String {
    const ADJ: &[&str] = &[
        "sunny", "misty", "mossy", "sleepy", "bouncy", "gentle", "fuzzy", "breezy", "starry",
        "rosy", "cosy", "twinkly", "dappled", "golden", "silver", "velvet", "merry", "quiet",
        "brave", "curious", "sparkly", "dreamy", "plucky", "cheery",
    ];
    const ANIMAL: &[&str] = &[
        "otter", "fox", "wren", "hedgehog", "badger", "rabbit", "owl", "duckling", "heron", "moth",
        "newt", "vole", "robin", "finch", "seal", "puffin", "turtle", "squirrel", "lamb", "kitten",
        "panda", "koala", "frog", "bee",
    ];
    let a = ADJ[(roll % ADJ.len() as u64) as usize];
    let b = ANIMAL[((roll / 31) % ANIMAL.len() as u64) as usize];
    format!("{a}-{b}-{}", 10 + (roll / 977) % 90)
}

/// "Poppy (sunny-otter-42)"
pub fn full(nickname: &str, user: &str) -> String {
    if nickname.eq_ignore_ascii_case(user) {
        user.to_string()
    } else {
        format!("{nickname} ({user})")
    }
}

pub fn hash(key: &str) -> String {
    Sha256::digest(key.trim().as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_are_cute_and_varied() {
        let names: std::collections::HashSet<String> = (0..500u64)
            .map(|i| username(crate::server::rng::hash(i)))
            .collect();
        assert!(names.len() > 450, "too many repeats: {}", names.len());
        let u = username(12345);
        let parts: Vec<&str> = u.split('-').collect();
        assert_eq!(parts.len(), 3, "{u}");
        assert!(
            parts[2]
                .parse::<u32>()
                .is_ok_and(|n| (10..100).contains(&n)),
            "{u}"
        );
        assert_eq!(full("Poppy", "sunny-otter-42"), "Poppy (sunny-otter-42)");
    }
}
