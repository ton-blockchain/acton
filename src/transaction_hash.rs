use std::str::FromStr;

use anyhow::Context as _;
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use tycho_types::cell::HashBytes;

pub(crate) fn toncenter_transaction_hash_hex(hash: &str) -> anyhow::Result<String> {
    if let Ok(hash) = HashBytes::from_str(hash) {
        return Ok(hex::encode(hash.as_slice()));
    }

    let decoded = [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD]
        .into_iter()
        .find_map(|engine| engine.decode(hash).ok())
        .with_context(|| format!("Invalid Toncenter transaction hash: {hash}"))?;
    if decoded.len() != 32 {
        anyhow::bail!(
            "Toncenter transaction hash must be 32 bytes, got {}",
            decoded.len()
        );
    }
    Ok(hex::encode(decoded))
}

#[cfg(test)]
mod tests {
    use super::toncenter_transaction_hash_hex;

    const HASH_HEX: &str = "a07d951a702b910d5f65b710ca8ce9667bd0f3d803cf848e01f75744a08d394b";
    const HASH_BASE64: &str = "oH2VGnArkQ1fZbcQyozpZnvQ89gDz4SOAfdXRKCNOUs=";

    #[test]
    fn normalizes_hex_and_base64_toncenter_transaction_hashes() {
        assert_eq!(
            toncenter_transaction_hash_hex(HASH_HEX).expect("hex hash should normalize"),
            HASH_HEX
        );
        assert_eq!(
            toncenter_transaction_hash_hex(HASH_BASE64).expect("base64 hash should normalize"),
            HASH_HEX
        );
    }
}
