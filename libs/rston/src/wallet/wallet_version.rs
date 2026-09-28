use crate::cell::{Cell, CellBuilder, HashBytes};
use crate::error::WalletError;
use crate::wallet::WalletVersion::*;
use crate::wallet::*;

/// A TON wallet contract version.
///
/// Serde represents versions using their Rust variant names, such as `"V4R2"` and `"HLV2R2"`.
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum WalletVersion {
    V1R1,
    V1R2,
    V1R3,
    V2R1,
    V2R2,
    V3R1,
    V3R2,
    V4R1,
    V4R2,
    V5R1,
    HLV1R1,
    HLV1R2,
    HLV2,
    HLV2R1,
    HLV2R2,
}

impl WalletVersion {
    /// Builds the initial data cell for a wallet version.
    pub fn get_default_data(
        version: WalletVersion,
        key_pair: &KeyPair,
        wallet_id: i32,
    ) -> Result<Cell, WalletError> {
        let public_key = HashBytes(key_pair.public_key);
        let data = match version {
            V1R1 | V1R2 | V1R3 | V2R1 | V2R2 => {
                CellBuilder::build_from(WalletV1V2Data::new(public_key))
            }
            V3R1 | V3R2 => CellBuilder::build_from(WalletV3Data::new(wallet_id, public_key)),
            V4R1 | V4R2 => CellBuilder::build_from(WalletV4Data::new(wallet_id, public_key)),
            V5R1 => CellBuilder::build_from(WalletV5Data::new(wallet_id, public_key)),
            HLV2R2 => CellBuilder::build_from(WalletHLV2R2Data::new(wallet_id, public_key)),
            HLV1R1 | HLV1R2 | HLV2 | HLV2R1 => {
                return Err(WalletError::Custom(format!(
                    "initial_data for {version:?} is unsupported"
                )));
            }
        };
        Ok(data?)
    }

    /// Returns the code cell for a wallet version.
    pub fn get_code(version: WalletVersion) -> Result<&'static Cell, WalletError> {
        TON_WALLET_CODE_BY_VERSION
            .get(&version)
            .ok_or_else(|| WalletError::Custom(format!("No code found for {version:?}")))
    }

    /// Detects a wallet version from its code hash.
    pub fn get_version_by_code(code_hash: HashBytes) -> Result<WalletVersion, WalletError> {
        TON_WALLET_VERSION_BY_CODE
            .get(&code_hash)
            .copied()
            .ok_or_else(|| {
                WalletError::Custom(format!("No version found for code_hash: {code_hash}"))
            })
    }

    /// Builds an unsigned external-message body.
    pub fn build_ext_in_body(
        version: WalletVersion,
        valid_until: u32,
        msg_seqno: u32,
        wallet_id: i32,
        msgs: Vec<Cell>,
    ) -> Result<Cell, WalletError> {
        let res = match version {
            V2R1 | V2R2 => CellBuilder::build_from(WalletV2ExtMsgBody {
                msg_seqno,
                valid_until,
                msgs_modes: vec![3u8; msgs.len()],
                msgs,
            }),
            V3R1 | V3R2 => CellBuilder::build_from(WalletV3ExtMsgBody {
                subwallet_id: wallet_id,
                msg_seqno,
                valid_until,
                msgs_modes: vec![3u8; msgs.len()],
                msgs,
            }),
            V4R1 | V4R2 => CellBuilder::build_from(WalletV4ExtMsgBody {
                subwallet_id: wallet_id,
                valid_until,
                msg_seqno,
                opcode: 0,
                msgs_modes: vec![3u8; msgs.len()],
                msgs,
            }),
            V5R1 => CellBuilder::build_from(WalletV5ExtMsgBody {
                wallet_id,
                valid_until,
                msg_seqno,
                msgs_modes: vec![3u8; msgs.len()],
                msgs,
            }),
            _ => {
                return Err(WalletError::Custom(format!(
                    "build_ext_in_body for {version:?} is unsupported"
                )));
            }
        };
        res.map_err(WalletError::from)
    }

    pub(super) fn sign_msg(
        version: WalletVersion,
        msg_cell: &Cell,
        sign: &[u8],
    ) -> Result<Cell, WalletError> {
        match version {
            // different order
            V5R1 => {
                let mut builder = CellBuilder::new();
                builder.store_slice(msg_cell.as_slice()?)?;
                builder.store_raw(sign, (sign.len() * 8) as u16)?;
                Ok(builder.build()?)
            }
            _ => {
                let mut builder = CellBuilder::new();
                builder.store_raw(sign, (sign.len() * 8) as u16)?;
                builder.store_slice(msg_cell.as_slice()?)?;
                Ok(builder.build()?)
            }
        }
    }
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
