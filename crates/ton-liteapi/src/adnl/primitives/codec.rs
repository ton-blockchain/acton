use aes::cipher::{KeyIvInit, StreamCipher};
use sha2::{Digest, Sha256};
use tokio_util::{
    bytes::{Buf, Bytes, BytesMut},
    codec::{Decoder, Encoder},
};

use crate::adnl::{AdnlAesParams, AdnlError};

use super::AdnlAes;

/// Implementation of ADNL protocol. Connection must be first initialized with [`AdnlHandshake`] to exchange keys.
pub struct AdnlCodec {
    aes_rx: AdnlAes,
    aes_tx: AdnlAes,
    last_readed_length: Option<usize>,
}

impl AdnlCodec {
    pub fn client(aes_params: &AdnlAesParams) -> Self {
        Self {
            aes_rx: AdnlAes::new(aes_params.rx_key().into(), aes_params.rx_nonce().into()),
            aes_tx: AdnlAes::new(aes_params.tx_key().into(), aes_params.tx_nonce().into()),
            last_readed_length: None,
        }
    }

    pub fn server(aes_params: &AdnlAesParams) -> Self {
        Self {
            aes_rx: AdnlAes::new(aes_params.tx_key().into(), aes_params.tx_nonce().into()),
            aes_tx: AdnlAes::new(aes_params.rx_key().into(), aes_params.rx_nonce().into()),
            last_readed_length: None,
        }
    }
}

impl Decoder for AdnlCodec {
    type Item = Bytes;

    type Error = AdnlError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let length = if let Some(length) = self.last_readed_length {
            length
        } else {
            if src.len() < 4 {
                return Ok(None);
            }
            self.aes_rx.apply_keystream(&mut src[..4]);
            let mut length_bytes = [0u8; 4];
            length_bytes.copy_from_slice(&src[..4]);
            let length = u32::from_le_bytes(length_bytes) as usize;
            if length < 64 {
                return Err(AdnlError::TooShortPacket);
            }
            if length > (1 << 24) {
                return Err(AdnlError::TooLongPacket);
            }
            src.advance(4);
            self.last_readed_length = Some(length);
            length
        };

        // not enough bytes, need to wait for more data
        if src.len() < length {
            if src.capacity() < length {
                src.reserve(length - src.capacity());
            }
            return Ok(None);
        }

        self.last_readed_length = None;

        // decode packet
        self.aes_rx.apply_keystream(&mut src[..length]);
        let given_hash = &src[length - 32..length];

        // integrity check
        let mut hasher = Sha256::new();
        hasher.update(&src[..length - 32]);
        if given_hash != hasher.finalize().as_slice() {
            return Err(AdnlError::IntegrityError);
        }

        // copy and return buffer
        let result = Bytes::copy_from_slice(&src[32..length - 32]);
        src.advance(length);
        Ok(Some(result))
    }
}

impl Encoder<Bytes> for AdnlCodec {
    type Error = AdnlError;

    fn encode(&mut self, buffer: Bytes, dst: &mut BytesMut) -> Result<(), Self::Error> {
        if buffer.len() > ((1 << 24) - 64) {
            return Err(AdnlError::TooLongPacket);
        }
        let length = ((buffer.len() + 64) as u32).to_le_bytes();
        let nonce = rand::random::<[u8; 32]>();
        let mut hash = Sha256::new();
        hash.update(nonce);
        hash.update(&buffer);
        let hash = hash.finalize();
        dst.reserve(buffer.len() + 68);
        dst.extend_from_slice(&length);
        dst.extend_from_slice(&nonce);
        dst.extend_from_slice(&buffer);
        dst.extend_from_slice(&hash);
        let start_offset = dst.len() - buffer.len() - 68;
        self.aes_tx.apply_keystream(&mut dst[start_offset..]);
        Ok(())
    }
}
