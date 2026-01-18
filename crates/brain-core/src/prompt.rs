//! Prompt helpers for hashing and tracking prompt versions.

use sha2::{Digest, Sha256};

/// Compute a stable SHA-256 fingerprint for a prompt string.
pub fn hash_prompt(prompt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prompt.as_bytes());
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{:02x}", byte));
    }
    hex
}

#[cfg(test)]
mod tests {
    use super::hash_prompt;

    #[test]
    fn test_hash_prompt_stable() {
        let first = hash_prompt("test prompt");
        let second = hash_prompt("test prompt");
        let different = hash_prompt("another prompt");

        assert_eq!(first, second);
        assert_ne!(first, different);
    }
}
