//! Hashing primitives used to derive seed bytes.
//!
//! We use SHA-256 rather than a faster non-cryptographic hash because:
//!
//! 1. SHA-256 is universally available and audited.
//! 2. We never need to hash more than one short string per render — performance
//!    is irrelevant; determinism is everything.
//! 3. Domain separation (a string unique to seed-canvas) protects against
//!    collisions with future or third-party generators.

use sha2::{Digest, Sha256};

/// Canonical domain tag for seed derivation.
///
/// Bump the suffix whenever the derivation scheme changes so old seeds do not
/// silently collide with new ones.
pub const DEFAULT_DOMAIN_TAG: &str = "seed-canvas/v1";

/// Compute the hex-encoded SHA-256 of the concatenation of `parts`.
#[must_use]
pub fn sha256_hex(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    let digest = hasher.finalize();
    hex::encode(digest)
}

/// Derive the 32 seed bytes for a `(raw, domain_tag)` pair.
///
/// The hash input is `domain_tag || 0x00 || raw`. The `0x00` separator
/// protects against `"ab" || "cd"` and `"a" || "bcd"` producing the same
/// digest.
#[must_use]
pub fn seed_bytes(raw: &str, domain_tag: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain_tag.as_bytes());
    hasher.update([0u8]);
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_bytes_are_deterministic() {
        let a = seed_bytes("cosmos", DEFAULT_DOMAIN_TAG);
        let b = seed_bytes("cosmos", DEFAULT_DOMAIN_TAG);
        assert_eq!(a, b);
    }

    #[test]
    fn seed_bytes_differ_when_raw_differs() {
        assert_ne!(
            seed_bytes("cosmos", DEFAULT_DOMAIN_TAG),
            seed_bytes("cosmo", DEFAULT_DOMAIN_TAG)
        );
    }

    #[test]
    fn seed_bytes_differ_when_domain_differs() {
        let bytes_v1 = seed_bytes("cosmos", "seed-canvas/v1");
        let bytes_v2 = seed_bytes("cosmos", "seed-canvas/v2");
        assert_ne!(bytes_v1, bytes_v2);
    }

    #[test]
    fn domain_separator_blocks_concat_collision() {
        // Without the 0x00 separator these two could collide in the wild.
        let a = seed_bytes("ab", "cd");
        let b = seed_bytes("a", "bcd");
        assert_ne!(a, b);
    }

    #[test]
    fn sha256_hex_known_vector() {
        // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let h = sha256_hex(&[b""]);
        assert_eq!(
            h,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
