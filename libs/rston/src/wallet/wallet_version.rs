//! Identifiers for wallet contract versions and revisions.
//!
//! A revision selects an exact code cell and therefore affects the contract address.
//! Several revisions can share one storage or message layout.
//! [`crate::wallet::get_version_by_code`] identifies a revision by its code hash.
//! [`crate::wallet::Wallet`] handles address derivation and message construction.
//!
//! The enum includes code-only revisions without request constructors.
//! See the [TON ABI catalog](https://github.com/ton-blockchain/abis/tree/master/data/wallets)
//! for per-revision hashes, interfaces, and contract sources.

/// A TON wallet contract version.
///
/// A version selects embedded code, but does not guarantee support for every wallet operation.
/// [`crate::wallet::Wallet`] supports these operations:
///
/// | Versions | Initial data and address derivation | External transfer construction |
/// | --- | --- | --- |
/// | V1R1–V1R3 | Yes | No |
/// | V2R1–V2R2, V3R1–V3R2, V4R1–V4R2, V5R1 | Yes | Yes |
/// | HLV2R2 | Yes | No |
/// | HLV1R1, HLV1R2, HLV2, HLV2R1 | No | No |
///
/// Code and hash lookup are available for every variant.
/// Serde represents versions using their Rust variant names, such as `"V4R2"` and `"HLV2R2"`.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum WalletVersion {
    /// Original V1 code, with [`crate::wallet::WalletV1V2Data`] storage and no getters.
    V1R1,
    /// V1 code with the `seqno` getter and the same [`crate::wallet::WalletV1V2Data`] storage.
    V1R2,
    /// V1 code with `seqno` and `get_public_key` getters. Request construction is unsupported.
    V1R3,
    /// V2 code with expiring requests represented by [`crate::wallet::WalletV2ExtMsgBody`].
    V2R1,
    /// V2 code with the `get_public_key` getter and the same [`crate::wallet::WalletV2ExtMsgBody`] format.
    V2R2,
    /// V3 code with subwallet identifiers and [`crate::wallet::WalletV3Data`] storage.
    V3R1,
    /// V3 code with the `get_public_key` getter and the same [`crate::wallet::WalletV3ExtMsgBody`] format.
    V3R2,
    /// V4 revision 1, with plugin storage. This library constructs transfer requests only.
    V4R1,
    /// V4 revision 2, sharing [`crate::wallet::WalletV4Data`] and [`crate::wallet::WalletV4ExtMsgBody`] with revision 1.
    V4R2,
    /// V5R1 code with extension storage and [`crate::wallet::WalletV5ExtMsgBody`] external transfers.
    V5R1,
    /// Highload V1 revision 1, available for code and hash lookup only.
    HLV1R1,
    /// Highload V1 revision 2, available for code and hash lookup only.
    HLV1R2,
    /// Original Highload V2 code, available for code and hash lookup only.
    HLV2,
    /// Highload V2 revision 1, available for code and hash lookup only.
    HLV2R1,
    /// Highload V2 revision 2, with [`crate::wallet::WalletHLV2R2Data`] initial storage support.
    HLV2R2,
}

#[cfg(all(test, feature = "serde"))]
mod tests {
    use super::WalletVersion;

    #[test]
    fn test_wallet_version_serde_contract() -> anyhow::Result<()> {
        let version = WalletVersion::V4R2;
        let serialized = "\"V4R2\"";

        assert_eq!(serde_json::to_string(&version)?, serialized);
        assert_eq!(serde_json::from_str::<WalletVersion>(serialized)?, version);

        Ok(())
    }
}
