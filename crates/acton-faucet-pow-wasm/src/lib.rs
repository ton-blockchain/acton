use sha2::{Digest, Sha256};
use wasm_bindgen::prelude::*;

const JS_MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
const NONCE_NOT_FOUND: f64 = -1.0;

/// Searches one bounded nonce range and returns `-1` when the range has no solution.
///
/// The browser worker calls this in chunks so it can publish progress and honour
/// the challenge deadline between CPU-bound scans.
#[wasm_bindgen]
pub fn find_nonce(
    challenge: &str,
    difficulty: u32,
    start_nonce: f64,
    max_attempts: u32,
) -> Result<f64, JsValue> {
    if difficulty > 256 {
        return Err(JsValue::from_str(
            "PoW difficulty must be between 0 and 256 bits",
        ));
    }
    if !start_nonce.is_finite()
        || !(0.0..=JS_MAX_SAFE_INTEGER).contains(&start_nonce)
        || start_nonce.fract() != 0.0
    {
        return Err(JsValue::from_str(
            "PoW start nonce must be a non-negative safe integer",
        ));
    }

    let start_nonce = start_nonce as u64;
    let end_nonce = start_nonce
        .checked_add(u64::from(max_attempts))
        .filter(|end| *end <= JS_MAX_SAFE_INTEGER as u64 + 1)
        .ok_or_else(|| JsValue::from_str("PoW nonce range exceeds JavaScript safe integers"))?;

    Ok(
        find_nonce_in_range(challenge.as_bytes(), difficulty, start_nonce, end_nonce)
            .map_or(NONCE_NOT_FOUND, |nonce| nonce as f64),
    )
}

fn find_nonce_in_range(
    challenge: &[u8],
    difficulty: u32,
    start_nonce: u64,
    end_nonce: u64,
) -> Option<u64> {
    let mut challenge_hasher = Sha256::new();
    challenge_hasher.update(challenge);

    for nonce in start_nonce..end_nonce {
        let mut hasher = challenge_hasher.clone();
        hasher.update(nonce.to_be_bytes());
        let digest = hasher.finalize();

        if leading_zero_bits(&digest) >= difficulty {
            return Some(nonce);
        }
    }
    None
}

fn leading_zero_bits(bytes: &[u8]) -> u32 {
    let mut zero_bits = 0;
    for &byte in bytes {
        let leading_zeros = byte.leading_zeros();
        zero_bits += leading_zeros;
        if leading_zeros < 8 {
            break;
        }
    }
    zero_bits
}

#[cfg(test)]
mod tests {
    use super::{find_nonce_in_range, leading_zero_bits};

    #[test]
    fn matches_shared_faucet_vector() {
        assert_eq!(
            find_nonce_in_range(b"actonscan-test-vector", 12, 0, 100_000),
            Some(3869)
        );
    }

    #[test]
    fn respects_range_boundaries() {
        assert_eq!(
            find_nonce_in_range(b"actonscan-test-vector", 12, 0, 3869),
            None
        );
        assert_eq!(
            find_nonce_in_range(b"actonscan-test-vector", 12, 3869, 3870),
            Some(3869)
        );
    }

    #[test]
    fn counts_leading_zero_bits_across_bytes() {
        assert_eq!(leading_zero_bits(&[0, 0, 0b0001_1111]), 19);
        assert_eq!(leading_zero_bits(&[0xff]), 0);
        assert_eq!(leading_zero_bits(&[0, 0]), 16);
    }
}
