//! Safety number (fingerprint) generation for E2EE identity verification.
//!
//! Signal display fingerprints with identifier-bound inputs (ADR-0038).
//!
//! Independently implemented from the algorithm and public known-answer vector;
//! no libsignal implementation is copied. Zalo identifier semantics and native
//! interoperability remain unverified (re-notes/43-audit21-remediation.md).

use anyhow::Result;
use sha2::{Digest, Sha512};

/// Number of SHA-512 iterations used by the fingerprint generator.
pub const FINGERPRINT_ITERATIONS: usize = 5200;

/// Number of decimal digits in a safety number.
pub const SAFETY_NUMBER_DIGITS: usize = 60;

/// Digits per display group.
pub const GROUP_DIGITS: usize = 5;

/// Compute a local display fingerprint with empty stable identifiers.
///
/// This compatibility helper is not suitable for peer verification. Production
/// verification must use `safety_number_with_identifiers` with account and peer IDs.
pub fn safety_number(pub_a: &[u8; 32], pub_b: &[u8; 32]) -> Result<String> {
    let digits = safety_number_digits(pub_a, pub_b)?;
    Ok(group_digits(&digits))
}

/// Compute a local 60-digit display value with empty stable identifiers.
pub fn safety_number_digits(
    pub_a: &[u8; 32],
    pub_b: &[u8; 32],
) -> Result<[u8; SAFETY_NUMBER_DIGITS]> {
    Ok(combine_parties("", pub_a, "", pub_b))
}

/// Compute the Signal display fingerprint for two identified peers.
/// # Errors
/// Rejects missing or oversized stable identifiers instead of silently hashing
/// only the keys. This does not establish the official Zalo identifier contract.
pub fn safety_number_with_identifiers(
    local_id: &str,
    local_key: &[u8; 32],
    remote_id: &str,
    remote_key: &[u8; 32],
) -> Result<String> {
    anyhow::ensure!(
        !local_id.is_empty()
            && !remote_id.is_empty()
            && local_id.len() <= 4096
            && remote_id.len() <= 4096,
        "Thiếu hoặc sai định danh để tính mã an toàn"
    );
    Ok(group_digits(&combine_parties(
        local_id, local_key, remote_id, remote_key,
    )))
}

fn party_digits(identifier: &str, public_key: &[u8; 32]) -> [u8; 30] {
    let serialized_key = super::keystore::to_djb_bytes(public_key);
    // The display scheme hashes version 0, serialized key and identifier,
    // followed by the key on every round, including the first round.
    let mut seed = Sha512::new();
    seed.update([0u8; 2]);
    seed.update(serialized_key);
    seed.update(identifier.as_bytes());
    seed.update(serialized_key);
    let mut digest = seed.finalize();
    for _ in 1..FINGERPRINT_ITERATIONS {
        let mut round = Sha512::new();
        round.update(digest);
        round.update(serialized_key);
        digest = round.finalize();
    }
    let mut digits = [0u8; 30];
    for (chunk, encoded) in digest[..30]
        .as_chunks::<5>()
        .0
        .iter()
        .zip(digits.as_chunks_mut::<5>().0.iter_mut())
    {
        let value = chunk
            .iter()
            .fold(0u64, |acc, byte| (acc << 8) | u64::from(*byte))
            % 100_000;
        for (index, target) in encoded.iter_mut().enumerate() {
            *target = ((value / 10u64.pow(4 - index as u32)) % 10) as u8;
        }
    }
    digits
}

fn combine_parties(
    first_id: &str,
    first_key: &[u8; 32],
    second_id: &str,
    second_key: &[u8; 32],
) -> [u8; SAFETY_NUMBER_DIGITS] {
    let first = party_digits(first_id, first_key);
    let second = party_digits(second_id, second_key);
    let (left, right) = if first <= second {
        (first, second)
    } else {
        (second, first)
    };
    let mut digits = [0u8; SAFETY_NUMBER_DIGITS];
    digits[..30].copy_from_slice(&left);
    digits[30..].copy_from_slice(&right);
    digits
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
    fn identified_fingerprint_matches_independent_signal_vector() {
        // Public known-answer data, not real account credentials (re-notes/43).
        let alice: [u8; 32] =
            hex::decode("06863bc66d02b40d27b8d49ca7c09e9239236f9d7d25d6fcca5ce13c7064d868")
                .unwrap()
                .try_into()
                .unwrap();
        let bob: [u8; 32] =
            hex::decode("f781b6fb32fed9ba1cf2de978d4d5da28dc34046ae814402b5c0dbd96fda907b")
                .unwrap()
                .try_into()
                .unwrap();
        let actual =
            safety_number_with_identifiers("+14152222222", &alice, "+14153333333", &bob).unwrap();
        assert_eq!(
            actual.replace(' ', ""),
            "300354477692869396892869876765458257569162576843440918079131"
        );
        assert_eq!(
            actual,
            safety_number_with_identifiers("+14153333333", &bob, "+14152222222", &alice).unwrap()
        );
        assert_ne!(
            actual,
            safety_number_with_identifiers("different-owner", &alice, "+14153333333", &bob)
                .unwrap()
        );
    }

    #[test]
    fn verification_fingerprint_refuses_missing_identifiers() {
        assert!(safety_number_with_identifiers("", &[1; 32], "peer", &[2; 32]).is_err());
        assert!(safety_number_with_identifiers("owner", &[1; 32], "", &[2; 32]).is_err());
        assert!(
            safety_number_with_identifiers(&"x".repeat(4097), &[1; 32], "peer", &[2; 32]).is_err()
        );
    }

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
