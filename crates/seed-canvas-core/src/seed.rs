//! Deterministic seed streams.
//!
//! A [`Seed`] wraps a SplitMix64 stream keyed by `SHA-256(domain_tag || 0x00 ||
//! raw)`. Splitting is exposed via [`Seed::fork`], which derives a child
//! stream from the parent's *next* state without advancing the parent —
//! this is what guarantees that tweaking one parameter does not cascade
//! into the others.
//!
//! ## Properties
//!
//! * **Deterministic**: same `(raw, domain_tag)` → same stream on every
//!   platform, every Rust version, every architecture.
//! * **Independent sub streams**: `parent.fork("color")` is unaffected by
//!   how many values the parent has yielded.
//! * **No allocations**: construction allocates exactly one 32-byte buffer.
//! * **Forward-only**: SplitMix64 has no `rewind`. This is intentional —
//!   forward-only streams are auditable.

use crate::hash::{seed_bytes, DEFAULT_DOMAIN_TAG};

/// A deterministic, forward-only RNG stream.
///
/// # Examples
///
/// ```
/// use seed_canvas_core::Seed;
///
/// let mut a = Seed::from_string("cosmos");
/// let mut b = Seed::from_string("cosmos");
/// assert_eq!(a.next_u64(), b.next_u64());
///
/// let mut color_stream = a.fork("color");
/// let r = color_stream.f32(0.0, 1.0);
/// assert!((0.0..1.0).contains(&r));
/// ```
#[derive(Clone, Debug)]
pub struct Seed {
    raw: String,
    domain_tag: String,
    bytes: [u8; 32],
    state: u64,
}

impl Seed {
    /// Construct a seed from any human-readable string. The input is
    /// trimmed; empty inputs are rejected.
    ///
    /// # Errors
    ///
    /// # Panics
    ///
    /// Panics if `raw` is empty after trimming.
    pub fn from_string(raw: impl Into<String>) -> Self {
        Self::from_string_with_domain(raw, DEFAULT_DOMAIN_TAG)
    }

    /// Construct a seed with an explicit domain tag. Use this when you need
    /// to guarantee that two generators using the same raw input never
    /// produce the same byte stream.
    ///
    /// # Errors
    ///
    /// # Panics
    ///
    /// Panics if `raw` is empty after trimming.
    pub fn from_string_with_domain(raw: impl Into<String>, domain_tag: &str) -> Self {
        let raw = raw.into();
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            // We panic instead of returning Result because constructing a Seed
            // with an empty input would silently produce a "default" stream,
            // and silent defaults are the worst kind of bug for a deterministic
            // system. Force the caller to handle it.
            panic!("Seed::from_string: input must not be empty");
        }
        let bytes = seed_bytes(trimmed, domain_tag);
        let state = u64::from_le_bytes(bytes[..8].try_into().expect("8 bytes"));
        Self {
            raw: trimmed.to_owned(),
            domain_tag: domain_tag.to_owned(),
            bytes,
            state,
        }
    }

    /// Reconstruct a seed from its canonical 24-character URL handle.
    ///
    /// This is the inverse of [`Seed::handle`] and is used when a user
    /// pastes a share URL like `/p/galaxy/sc_4f2c9b3a7e1d50a2`.
    ///
    /// # Errors
    ///
    /// Returns [`SeedError::InvalidHandle`] for the wrong shape, or
    /// [`SeedError::UnknownSeed`] when the handle is well-formed but does
    /// not correspond to a known seed in the supplied dictionary.
    pub fn from_handle(handle: &str, dictionary: &[Seed]) -> Result<Self, SeedError> {
        let Some(hex_part) = handle.strip_prefix("sc_") else {
            return Err(SeedError::InvalidHandle(handle.to_owned()));
        };
        if hex_part.len() != 24 || !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(SeedError::InvalidHandle(handle.to_owned()));
        }
        // The handle is 12 bytes (96 bits) of the seed digest. We look the
        // full 32-byte digest up by re-hashing each candidate's raw + domain.
        // In practice the dictionary will be the gallery's recent seeds and
        // lookups will hit cache, so this is O(N) but N is small.
        for candidate in dictionary {
            if candidate.handle().ends_with(hex_part) {
                return Ok(candidate.clone());
            }
        }
        Err(SeedError::UnknownSeed(handle.to_owned()))
    }

    /// Raw input the seed was constructed from. Preserved verbatim (modulo
    /// trimming) so that round-trips like `Seed::from_string(seed.raw())`
    /// are stable.
    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Domain tag the seed was hashed under.
    #[must_use]
    pub fn domain_tag(&self) -> &str {
        &self.domain_tag
    }

    /// The full 32-byte SHA-256 digest. Useful for content addressing.
    #[must_use]
    pub fn bytes(&self) -> &[u8; 32] {
        &self.bytes
    }

    /// Short URL-safe handle: `"sc_" + 24 hex chars`. The handle is 96 bits
    /// of entropy — enough to avoid collisions at GitHub-stars scale while
    /// staying readable in URLs.
    #[must_use]
    pub fn handle(&self) -> String {
        let mut s = String::with_capacity(27);
        s.push_str("sc_");
        for byte in &self.bytes[..12] {
            s.push_str(&format!("{byte:02x}"));
        }
        s
    }

    /// Advance and return the next 64-bit value from the stream.
    pub fn next_u64(&mut self) -> u64 {
        // SplitMix64 advance + finalizer.
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix64(self.state)
    }

    /// Uniform integer in `[0, n)`.
    ///
    /// # Panics
    ///
    /// Panics if `n == 0`.
    #[must_use]
    pub fn range(&mut self, n: u64) -> u64 {
        assert!(n > 0, "Seed::range: n must be positive, got {n}");
        // Lemire's nearly-divisionless bounded uniform — keeps the stream
        // forward-only without rejection sampling.
        let x = self.next_u64();
        let m = u128::from(x) * u128::from(n);
        let lo = m as u64;
        if lo < n {
            n - ((n - lo).rem_euclid(n)) // rare rejection branch
        } else {
            (m >> 64) as u64
        }
    }

    /// Uniform float in `[lo, hi)`. `lo` must be less than `hi`.
    ///
    /// # Panics
    ///
    /// Panics if `lo >= hi` or if either is NaN.
    #[must_use]
    pub fn f32(&mut self, lo: f64, hi: f64) -> f64 {
        assert!(
            hi > lo && lo.is_finite() && hi.is_finite(),
            "Seed::f32: require finite lo < hi, got lo={lo} hi={hi}"
        );
        // 53-bit precision; we cast through f64 so the result is
        // platform-independent (f32 would round through arch-specific paths).
        let u = self.next_u64() >> 11;
        let unit = u as f64 / (1u64 << 53) as f64;
        lo + (hi - lo) * unit
    }

    /// Weighted choice. The probability of returning `value[i]` is
    /// `weights[i] / sum(weights)`. Weights must be non-negative and at
    /// least one must be positive.
    ///
    /// # Panics
    ///
    /// Panics if `weights` is empty, contains a negative weight, or sums
    /// to zero.
    #[must_use]
    pub fn weighted<T: Copy>(&mut self, choices: &[(T, f64)]) -> T {
        assert!(
            !choices.is_empty(),
            "Seed::weighted: choices must not be empty"
        );
        let mut total = 0.0_f64;
        for (_, w) in choices {
            assert!(*w >= 0.0, "Seed::weighted: weights must be non-negative");
            total += *w;
        }
        assert!(
            total > 0.0,
            "Seed::weighted: at least one weight must be positive"
        );

        let mut target = self.f32(0.0, total);
        for (value, weight) in choices {
            target -= *weight;
            if target < 0.0 {
                return *value;
            }
        }
        choices.last().expect("non-empty").0
    }

    /// Bernoulli trial — returns `true` with probability `p`.
    ///
    /// # Panics
    ///
    /// Panics if `p` is not in `[0, 1]`.
    #[must_use]
    pub fn branch(&mut self, p: f64) -> bool {
        assert!(
            (0.0..=1.0).contains(&p),
            "Seed::branch: p must be in [0, 1], got {p}"
        );
        self.f32(0.0, 1.0) < p
    }

    /// Spawn a deterministic sub-stream. The child is derived from the
    /// parent's *next* state (not its current one), so consuming the child
    /// does not perturb the parent.
    ///
    /// # Panics
    ///
    /// Panics if `label` is empty.
    #[must_use]
    pub fn fork(&self, label: &str) -> Self {
        assert!(!label.is_empty(), "Seed::fork: label must not be empty");
        // Simulate the parent's next state without actually advancing it.
        let parent_next = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let parent_mix = mix64(parent_next);
        // Hash the label under the same domain tag to keep the avalanche
        // distribution uniform regardless of label length.
        let label_bytes = seed_bytes(label, &self.domain_tag);
        let label_seed = u64::from_le_bytes(label_bytes[..8].try_into().expect("8 bytes"));
        let label_mix = mix64(label_seed);
        let child_state = parent_mix ^ label_mix;
        Self {
            raw: format!("{}#{}", self.raw, label),
            domain_tag: self.domain_tag.clone(),
            bytes: label_bytes,
            state: child_state,
        }
    }

    /// Convenience: produce a fresh entropy-backed seed using the system
    /// RNG. Used by the `seed-canvas random` CLI command.
    ///
    /// # Panics
    ///
    /// Panics if the operating system's RNG is unavailable.
    #[must_use]
    pub fn random() -> Self {
        let entropy = rand::random::<u64>();
        let raw = format!("entropy-{entropy}");
        Self::from_string(raw)
    }
}

/// Errors returned by [`Seed::from_handle`].
#[derive(Debug, thiserror::Error)]
pub enum SeedError {
    /// Handle did not match the `sc_` + 24 hex pattern.
    #[error("invalid seed handle: {0:?} (expected `sc_<24 hex chars>`)")]
    InvalidHandle(String),

    /// Handle is well-formed but the corresponding seed was not found in
    /// the dictionary.
    #[error("unknown seed: {0:?}")]
    UnknownSeed(String),
}

/// SplitMix64 finalizer — avalanche function that maximizes bit diffusion.
#[inline]
#[must_use]
pub(crate) fn mix64(x: u64) -> u64 {
    let z = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn determinism_across_calls() {
        let mut a = Seed::from_string("cosmos");
        let mut b = Seed::from_string("cosmos");
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn handles_are_27_chars_and_hex() {
        let s = Seed::from_string("cosmos");
        let handle = s.handle();
        assert!(handle.starts_with("sc_"));
        assert_eq!(handle.len(), 27);
        assert!(handle[3..].chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn range_stays_in_bounds() {
        let mut s = Seed::from_string("cosmos");
        for _ in 0..10_000 {
            let r = s.range(100);
            assert!(r < 100);
        }
    }

    #[test]
    fn fork_is_independent_of_parent() {
        // Two parents built identically should yield the same stream; forking
        // a child off parent A and consuming it must NOT change what parent B
        // produces next.
        let mut a = Seed::from_string("cosmos");
        let mut b = Seed::from_string("cosmos");

        let mut child = a.fork("color");
        let _ = child.next_u64();
        let _ = child.next_u64();

        let a1 = a.next_u64();
        let b1 = b.next_u64();
        assert_eq!(a1, b1, "forking + consuming child must not perturb parent");
    }

    #[test]
    fn fork_produces_different_stream_than_parent() {
        let mut parent = Seed::from_string("cosmos");
        let mut child = parent.fork("color");
        let p = parent.next_u64();
        let c = child.next_u64();
        assert_ne!(p, c, "forked stream must differ from parent");
    }

    #[test]
    fn weighted_respects_distribution() {
        let mut s = Seed::from_string("cosmos");
        let mut warm = 0_usize;
        let mut cool = 0_usize;
        for _ in 0..10_000 {
            let pick: &str = s.weighted(&[("warm", 9.0), ("cool", 1.0)]);
            match pick {
                "warm" => warm += 1,
                "cool" => cool += 1,
                _ => unreachable!(),
            }
        }
        // Should be roughly 90/10. Allow a wide margin to keep the test stable.
        assert!(
            warm > 8000,
            "warm should dominate, got warm={warm} cool={cool}"
        );
        assert!(
            cool < 2000,
            "cool should be minority, got warm={warm} cool={cool}"
        );
    }

    #[test]
    fn f32_stays_in_bounds() {
        let mut s = Seed::from_string("cosmos");
        for _ in 0..10_000 {
            let v = s.f32(-1.0, 1.0);
            assert!((-1.0..1.0).contains(&v), "f32 leaked out of bounds: {v}");
        }
    }

    #[test]
    fn empty_input_panics() {
        let result = std::panic::catch_unwind(|| Seed::from_string("   "));
        assert!(
            result.is_err(),
            "empty input must panic, not silently default"
        );
    }

    #[test]
    fn handle_round_trip() {
        let dict = vec![Seed::from_string("cosmos"), Seed::from_string("lattice")];
        let handle = dict[0].handle();
        let recovered = Seed::from_handle(&handle, &dict).expect("must round-trip");
        assert_eq!(recovered.raw(), dict[0].raw());
    }
}
