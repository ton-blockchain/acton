use crate::adnl::crypto::{KeyPair, PublicKey};

use crate::adnl::{AdnlAddress, AdnlAesParams, AdnlHandshake};

use crate::adnl::helper_types::CryptoRandom;

/// Builder of [`AdnlHandshake`] structure, which then can be transformed into [`crate::AdnlClient`]
pub struct AdnlBuilder {
    aes_params: AdnlAesParams,
}

impl AdnlBuilder {
    /// Use specified session parameters. It is recommended to use random parameters.
    pub fn with_static_aes_params(aes_params: AdnlAesParams) -> Self {
        Self { aes_params }
    }

    /// Use random session parameters (recommended).
    pub fn with_random_aes_params<R: CryptoRandom>(rng: &mut R) -> Self {
        Self {
            aes_params: {
                let mut buffer = [0u8; 160];
                rng.fill_bytes(&mut buffer);
                AdnlAesParams::from(buffer)
            },
        }
    }

    /// Specify sender, receiver, and secret on which they already agreed.
    pub fn use_static_ecdh(
        self,
        sender_public: PublicKey,
        receiver_address: AdnlAddress,
        ecdh_secret: [u8; 32],
    ) -> AdnlHandshake {
        AdnlHandshake::new(
            receiver_address,
            sender_public,
            ecdh_secret,
            self.aes_params,
        )
    }

    /// Perform key agreement using sender private key and receiver public
    pub fn perform_ecdh(
        self,
        sender_keypair: &KeyPair,
        receiver_public: &PublicKey,
    ) -> AdnlHandshake {
        AdnlHandshake::new(
            AdnlAddress::from(receiver_public),
            sender_keypair.public_key,
            sender_keypair.compute_shared_secret(receiver_public),
            self.aes_params,
        )
    }
}
