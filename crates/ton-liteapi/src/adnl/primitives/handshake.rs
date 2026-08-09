use crate::adnl::crypto::{KeyPair, PublicKey};
use crate::adnl::primitives::AdnlAes;
use crate::adnl::{AdnlAddress, AdnlAesParams, AdnlError, AdnlPeer};
use aes::cipher::KeyIvInit;
use ctr::cipher::StreamCipher;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::codec::AdnlCodec;

/// Handshake packet, must be sent from client to server prior to any datagrams
#[derive(Debug)]
pub struct AdnlHandshake {
    receiver: AdnlAddress,
    sender: PublicKey,
    aes_params: AdnlAesParams,
    secret: [u8; 32],
}

impl AdnlHandshake {
    /// Create handshake with given sender and receiver, who already agreed on given secret, also
    /// use given session parameters
    pub fn new(
        receiver: AdnlAddress,
        sender: PublicKey,
        secret: [u8; 32],
        aes_params: AdnlAesParams,
    ) -> Self {
        Self {
            receiver,
            sender,
            aes_params,
            secret,
        }
    }

    /// Get session AES parameters
    pub fn aes_params(&self) -> &AdnlAesParams {
        &self.aes_params
    }

    /// Get initiator public key of this handshake
    pub fn sender(&self) -> &PublicKey {
        &self.sender
    }

    /// Get destination ADNL address of this handshake
    pub fn receiver(&self) -> &AdnlAddress {
        &self.receiver
    }

    /// Serialize handshake to send it over the transport
    pub fn to_bytes(&self) -> [u8; 256] {
        let mut raw_params = self.aes_params.to_bytes();
        let hash = Self::sha256(raw_params);
        let mut aes = Self::initialize_aes(&self.secret, &hash);
        aes.apply_keystream(&mut raw_params);

        let mut packet = [0u8; 256];
        packet[..32].copy_from_slice(self.receiver.as_bytes());
        packet[32..64].copy_from_slice(self.sender.as_bytes());
        packet[64..96].copy_from_slice(&hash);
        packet[96..256].copy_from_slice(&raw_params);
        packet
    }

    pub fn make_client_codec(&self) -> AdnlCodec {
        AdnlCodec::client(&self.aes_params)
    }

    pub fn make_server_codec(&self) -> AdnlCodec {
        AdnlCodec::server(&self.aes_params)
    }

    /// Send handshake over the given transport, build [`AdnlClient`] on top of it
    pub async fn perform_handshake<T: AsyncReadExt + AsyncWriteExt + Unpin>(
        &self,
        transport: T,
    ) -> Result<AdnlPeer<T>, AdnlError> {
        AdnlPeer::perform_custom_handshake(transport, self).await
    }

    fn initialize_aes(secret: &[u8; 32], hash: &[u8]) -> AdnlAes {
        let mut key = [0u8; 32];
        key[..16].copy_from_slice(&secret[..16]);
        key[16..32].copy_from_slice(&hash[16..32]);

        let mut nonce = [0u8; 16];
        nonce[..4].copy_from_slice(&hash[..4]);
        nonce[4..16].copy_from_slice(&secret[20..32]);

        AdnlAes::new(key.as_slice().into(), nonce.as_slice().into())
    }

    fn sha256(data: impl AsRef<[u8]>) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    /// Deserialize and decrypt handshake using keypair from `keypair_selector` function
    pub fn decrypt_from_raw<F: Fn(&AdnlAddress) -> Option<KeyPair>>(
        packet: &[u8; 256],
        keypair_selector: F,
    ) -> Result<Self, AdnlError> {
        let receiver = packet[..32].try_into().unwrap();
        let sender = PublicKey::from_bytes(packet[32..64].try_into().unwrap())
            .ok_or(AdnlError::InvalidPublicKey)?;
        let hash: [u8; 32] = packet[64..96].try_into().unwrap();
        let mut raw_params: [u8; 160] = packet[96..256].try_into().unwrap();

        let keypair =
            keypair_selector(&receiver).ok_or_else(|| AdnlError::UnknownAddr(receiver.clone()))?;

        let our_address = AdnlAddress::from(&keypair.public_key);
        if our_address != receiver {
            log::error!(
                "private key selector returned wrong key, expected address: {:?}, got: {:?}",
                &receiver,
                our_address
            );
            return Err(AdnlError::UnknownAddr(receiver));
        }

        let secret = keypair.compute_shared_secret(&sender);
        let mut aes = Self::initialize_aes(&secret, &hash);
        aes.apply_keystream(&mut raw_params);

        if hash != Self::sha256(raw_params) {
            return Err(AdnlError::IntegrityError);
        }

        Ok(Self {
            receiver,
            sender,
            aes_params: AdnlAesParams::from(raw_params),
            secret,
        })
    }
}
