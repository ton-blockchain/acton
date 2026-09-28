//! Embedded wallet contract code and code-hash lookup.
//!
//! The code cells are decoded once on first access. Their representation hashes
//! identify exact contract revisions, even when two revisions share a data layout.
//! [`get_code`] and [`get_version_by_code`] expose these lookups without manual map access.
//!
//! The [TON ABI catalog](https://github.com/ton-blockchain/abis/tree/master/data/wallets)
//! lists known code hashes and source links for each revision.

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::boc::Boc;
use crate::cell::{Cell, HashBytes};
use crate::error::WalletError;
use crate::wallet::WalletVersion;

macro_rules! load_code {
    ($filename:literal) => {
        Boc::decode_base64(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/ton_wallet_code/",
            $filename
        )))
        .unwrap()
    };
}

/// Contract code indexed by exact revision, decoded once on first access.
///
/// Every [`WalletVersion`] has an entry, including revisions without request constructors.
/// Entries contain executable code cells, not deployment states or encoded BoC bytes.
pub static TON_WALLET_CODE_BY_VERSION: LazyLock<HashMap<WalletVersion, Cell>> =
    LazyLock::new(|| {
        use WalletVersion::*;

        HashMap::from([
            (V1R1, load_code!("wallet_v1r1.code")),
            (V1R2, load_code!("wallet_v1r2.code")),
            (V1R3, load_code!("wallet_v1r3.code")),
            (V2R1, load_code!("wallet_v2r1.code")),
            (V2R2, load_code!("wallet_v2r2.code")),
            (V3R1, load_code!("wallet_v3r1.code")),
            (V3R2, load_code!("wallet_v3r2.code")),
            (V4R1, load_code!("wallet_v4r1.code")),
            (V4R2, load_code!("wallet_v4r2.code")),
            (V5R1, load_code!("wallet_v5.code")),
            (HLV1R1, load_code!("highload_v1r1.code")),
            (HLV1R2, load_code!("highload_v1r2.code")),
            (HLV2, load_code!("highload_v2.code")),
            (HLV2R1, load_code!("highload_v2r1.code")),
            (HLV2R2, load_code!("highload_v2r2.code")),
        ])
    });

/// Known wallet revisions indexed by the representation hash of their code cell.
///
/// Derived from [`TON_WALLET_CODE_BY_VERSION`]. Only exact embedded code hashes match.
pub static TON_WALLET_VERSION_BY_CODE: LazyLock<HashMap<HashBytes, WalletVersion>> =
    LazyLock::new(|| {
        TON_WALLET_CODE_BY_VERSION
            .iter()
            .map(|(k, v)| (*v.repr_hash(), *k))
            .collect()
    });

/// Returns embedded contract code for the requested revision.
///
/// The shared cell remains available for the lifetime of the process.
/// Code availability does not imply support for constructing requests for that revision.
///
/// # Errors
///
/// Returns [`WalletError::CodeNotFound`] if the embedded catalog has no entry for `version`.
pub fn get_code(version: WalletVersion) -> Result<&'static Cell, WalletError> {
    TON_WALLET_CODE_BY_VERSION
        .get(&version)
        .ok_or(WalletError::CodeNotFound(version))
}

/// Identifies a wallet revision by its code cell's representation hash.
///
/// This expects the code hash, not the account address or a hash of encoded BoC bytes.
/// The lookup is local and matches only revisions in [`TON_WALLET_VERSION_BY_CODE`].
///
/// # Errors
///
/// Returns [`WalletError::UnknownCodeHash`] if no embedded code cell has the supplied hash.
pub fn get_version_by_code(code_hash: HashBytes) -> Result<WalletVersion, WalletError> {
    TON_WALLET_VERSION_BY_CODE
        .get(&code_hash)
        .copied()
        .ok_or(WalletError::UnknownCodeHash(code_hash))
}
