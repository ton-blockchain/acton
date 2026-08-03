use rand::RngCore;
use sha2::{Digest, Sha256};

pub const CHALLENGE_VERSION: [u32; 1] = [1];

#[derive(Clone, Copy, Debug)]
pub struct Pow {
    difficulty: u32,
}

impl Pow {
    pub fn new(difficulty: u32) -> Self {
        Self { difficulty }
    }

    pub fn difficulty(&self) -> u32 {
        self.difficulty
    }

    pub fn version(&self) -> u32 {
        CHALLENGE_VERSION[CHALLENGE_VERSION.len() - 1]
    }

    pub fn can_process_version(&self, version: u32) -> bool {
        CHALLENGE_VERSION.contains(&version)
    }

    pub fn create(&self) -> String {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        hex::encode(bytes)
    }

    pub fn verify(&self, challenge: &str, nonce: u64) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(challenge.as_bytes());
        hasher.update(nonce.to_be_bytes());
        let result = hasher.finalize();

        let mut zero_bits = 0;
        for &byte in result.iter() {
            let leading_zeros = byte.leading_zeros();
            zero_bits += leading_zeros;
            if leading_zeros < 8 {
                break;
            }
        }

        zero_bits >= self.difficulty
    }
}

#[cfg(test)]
mod tests {
    use super::{CHALLENGE_VERSION, Pow};
    use std::collections::HashSet;

    #[test]
    fn stores_difficulty() {
        for difficulty in [0, 1, 8, 21, 256, u32::MAX] {
            assert_eq!(Pow::new(difficulty).difficulty(), difficulty);
        }
    }

    #[test]
    fn stores_challenge_versions() {
        assert_eq!(CHALLENGE_VERSION, [1]);
    }

    #[test]
    fn returns_current_challenge_version() {
        assert_eq!(Pow::new(21).version(), 1);
    }

    #[test]
    fn accepts_current_challenge_version() {
        assert!(Pow::new(21).can_process_version(1));
    }

    #[test]
    fn rejects_unknown_challenge_versions() {
        let pow = Pow::new(21);

        assert!(!pow.can_process_version(0));
        assert!(!pow.can_process_version(pow.version() + 1));
    }

    #[test]
    fn creates_32_byte_hex_challenge() {
        for _ in 0..64 {
            let challenge = Pow::new(0).create();
            let decoded = hex::decode(&challenge).unwrap();

            assert_eq!(challenge.len(), 64);
            assert_eq!(decoded.len(), 32);
            assert!(challenge.chars().all(|char| char.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn creates_distinct_challenges() {
        let challenges = (0..128)
            .map(|_| Pow::new(0).create())
            .collect::<HashSet<_>>();

        assert!(challenges.len() > 1);
    }

    #[test]
    fn zero_difficulty_accepts_any_input() {
        let pow = Pow::new(0);

        for (challenge, nonce) in [
            ("", 0),
            ("challenge", 0),
            ("challenge", u64::MAX),
            ("тон", 42),
            ("not-hex-but-still-valid-input", 1),
        ] {
            assert!(pow.verify(challenge, nonce));
        }
    }

    #[test]
    fn impossible_difficulty_rejects_everything() {
        for difficulty in [257, u32::MAX] {
            let pow = Pow::new(difficulty);

            assert!(!pow.verify("", 0));
            assert!(!pow.verify("challenge", 0));
            assert!(!pow.verify("challenge", u64::MAX));
        }
    }

    #[test]
    fn verifies_known_nonce_vectors_at_exact_boundaries() {
        let vectors = [
            ("challenge", 0, 3),
            ("challenge", 3, 4),
            ("challenge", 197, 8),
            ("challenge", 2933, 12),
            ("challenge", 10974, 16),
            ("challenge", 1_145_266, 23),
        ];

        for (challenge, nonce, zero_bits) in vectors {
            assert!(Pow::new(zero_bits).verify(challenge, nonce));
            assert!(!Pow::new(zero_bits + 1).verify(challenge, nonce));
        }
    }

    #[test]
    fn verify_is_deterministic() {
        let pow = Pow::new(12);

        for (challenge, nonce) in [
            ("", 0),
            ("challenge", 197),
            ("challenge", u64::MAX),
            ("тон", 42),
        ] {
            let expected = pow.verify(challenge, nonce);

            for _ in 0..16 {
                assert_eq!(pow.verify(challenge, nonce), expected);
            }
        }
    }

    #[test]
    fn matches_actonscan_sha256_test_vector() {
        let pow = Pow::new(12);

        assert!(pow.verify("actonscan-test-vector", 3_869));
    }

    #[test]
    fn generated_challenge_is_accepted_at_zero_difficulty() {
        let pow = Pow::new(0);

        for nonce in [0, 1, 42, u64::MAX] {
            assert!(pow.verify(&pow.create(), nonce));
        }
    }
}
