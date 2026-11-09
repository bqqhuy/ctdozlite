//! Safety number (fingerprint) generation for E2EE identity verification.
//!
//! Evidence: `re-notes/36-zalo-e2ee-signal-protocol-analysis.md` §3.1
//! - `FingerprintGenerator.createFor(a, pubKey_a, b, pubKey_b)`
//! - `FINGERPRINT_ITERATIONS = 5200`
//! - 60 digits, grouped 5 per group, symmetric (`a↔b` yields the same number)
//!
//! The algorithm follows the publicly documented Signal fingerprint scheme:
//! iterate `SHA-512` over `hash || pubKey_a || pubKey_b` 5200 times, then read
//! the resulting digest in 5-byte chunks, each chunk yielding five decimal
//! digits.  No code from libsignal is reproduced.

use anyhow::Result;
use sha2::{Digest, Sha512};

/// Number of SHA-512 iterations used by the fingerprint generator.
pub const FINGERPRINT_ITERATIONS: usize = 5200;

/// Number of decimal digits in a safety number.
pub const SAFETY_NUMBER_DIGITS: usize = 60;

/// Digits per display group.
pub const GROUP_DIGITS: usize = 5;

/// Compute the safety number for a pair of identity public keys.
///
/// The result is symmetric: `safety_number(a, b) == safety_number(b, a)`.
pub fn safety_number(pub_a: &[u8; 32], pub_b: &[u8; 32]) -> Result<String> {
    let digits = safety_number_digits(pub_a, pub_b)?;
    Ok(group_digits(&digits))
}

/// Compute the raw 60-digit safety number.
///
/// The two public keys are ordered lexicographically before hashing so that
/// both peers derive the same digits regardless of who initiated the check.
pub fn safety_number_digits(
    pub_a: &[u8; 32],
    pub_b: &[u8; 32],
) -> Result<[u8; SAFETY_NUMBER_DIGITS]> {
    let (first, second) = if pub_a <= pub_b {
        (pub_a, pub_b)
    } else {
        (pub_b, pub_a)
    };
    let mut hash = [0u8; 64];
    for _ in 0..FINGERPRINT_ITERATIONS {
        let mut hasher = Sha512::new();
        hasher.update(hash);
        hasher.update(first);
        hasher.update(second);
        hash = hasher.finalize().into();
    }
    // 60 digits = 12 groups of 5 bytes.
    let mut digits = [0u8; SAFETY_NUMBER_DIGITS];
    for chunk in 0..(SAFETY_NUMBER_DIGITS / GROUP_DIGITS) {
        let mut value: u64 = 0;
        for byte in &hash[chunk * 5..chunk * 5 + 5] {
            value = (value << 8) | u64::from(*byte);
        }
        let group = value % 100_000u64;
        for i in 0..GROUP_DIGITS {
            // Right-align the five digits inside the group.
            let shift = 10u64.pow((GROUP_DIGITS - 1 - i) as u32);
            digits[chunk * GROUP_DIGITS + i] = ((group / shift) % 10) as u8;
        }
    }
    Ok(digits)
}

/// Group the digits for display: 12 groups of 5 separated by spaces.
pub fn group_digits(digits: &[u8; SAFETY_NUMBER_DIGITS]) -> String {
    let mut out = String::with_capacity(SAFETY_NUMBER_DIGITS + 12);
    for (i, d) in digits.iter().enumerate() {
        if i > 0 && i % GROUP_DIGITS == 0 {
            out.push(' ');
        }
        out.push((b'0' + d) as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safety_number_is_symmetric() {
        let a = [1u8; 32];
        let b = [2u8; 32];
        let ab = safety_number(&a, &b).unwrap();
        let ba = safety_number(&b, &a).unwrap();
        assert_eq!(ab, ba);
    }

    #[test]
    fn safety_number_has_sixty_digits_in_twelve_groups() {
        let a = [3u8; 32];
        let b = [4u8; 32];
        let text = safety_number(&a, &b).unwrap();
        let groups: Vec<&str> = text.split(' ').collect();
        assert_eq!(groups.len(), 12);
        for g in groups {
            assert_eq!(g.len(), GROUP_DIGITS);
            assert!(g.chars().all(|c| c.is_ascii_digit()));
        }
        let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
        assert_eq!(digits.len(), SAFETY_NUMBER_DIGITS);
    }

    #[test]
    fn different_identities_give_different_numbers() {
        let a = [5u8; 32];
        let b = [6u8; 32];
        let c = [7u8; 32];
        assert_ne!(
            safety_number(&a, &b).unwrap(),
            safety_number(&a, &c).unwrap()
        );
    }

    #[test]
    fn safety_number_is_deterministic() {
        let a = [9u8; 32];
        let b = [10u8; 32];
        assert_eq!(
            safety_number(&a, &b).unwrap(),
            safety_number(&a, &b).unwrap()
        );
    }
}
