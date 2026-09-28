//! Wallet V4R1 and V4R2: subwallets with an on-chain plugin dictionary.
//!
//! V4 adds plugins that can request transfers through internal messages.
//! This module models wallet storage and ordinary signed transfers with opcode zero.
//! It does not model plugin installation, removal, or plugin request bodies.
//!
//! Protocol references:
//!
//! - [V4 storage, message layouts, and plugin operations](https://docs.ton.org/contracts/standard/wallets/v4).
//! - [FunC parser and dictionary operations](https://github.com/ton-blockchain/wallet-contract/blob/main/func/wallet-v4-code.fc).
//! - Tolk ABI definitions: [V4R1](https://github.com/ton-blockchain/abis/blob/master/data/wallets/w4r1/types/wallet_v4r1.types.tolk)
//!   and [V4R2](https://github.com/ton-blockchain/abis/blob/master/data/wallets/w4r2/types/wallet_v4r2.types.tolk).
//!
//! The contract source defines the exact dictionary and message encoding.
//! The ABI definitions also cover plugin operations outside this module's transfer API.

use crate::cell::{Cell, CellBuilder, CellContext, CellSlice, HashBytes, Load, Store};
use crate::error::Error;
use crate::wallet::WalletMessage;
use crate::wallet::versions::message_utils::{read_up_to_4_msgs, write_up_to_4_msgs};

/// Persistent data for V4R1 and V4R2 wallets, including installed plugins.
///
/// The serialized fields are `seqno:uint32`, `wallet_id:uint32`, `public_key:bits256`,
/// and the plugins dictionary. The Rust `i32` wallet ID preserves all 32 wire bits.
/// The [contract source](https://github.com/ton-blockchain/wallet-contract/blob/main/func/wallet-v4-code.fc)
/// defines plugin keys as an 8-bit workchain followed by a 256-bit account hash.
/// Dictionary values are empty. This type preserves the raw root without decoding entries.
#[derive(Debug, PartialEq, Clone, Load, Store)]
pub struct WalletV4Data {
    /// Stored sequence number for replay protection. Initial data uses zero.
    pub seqno: u32,
    /// Wallet identifier encoded as 32 bits. It must match the request and stored state.
    pub wallet_id: i32,
    /// Ed25519 public key used to verify owner signatures.
    pub public_key: HashBytes,
    /// Raw plugin dictionary root. `None` represents an empty dictionary.
    pub plugins: Option<Cell>,
}

impl WalletV4Data {
    /// Creates V4 deployment data with sequence number zero and an empty plugin dictionary.
    ///
    /// `wallet_id` and the public key are stored unchanged.
    /// Existing plugin state must be loaded from account data rather than recreated with this constructor.
    pub fn new(wallet_id: i32, public_key: HashBytes) -> Self {
        Self {
            seqno: 0,
            wallet_id,
            public_key,
            plugins: None,
        }
    }

    /// Serializes this state as the wallet's persistent data cell.
    ///
    /// Preserves field values and raw dictionary cells without validating dictionary entries.
    /// For deployment defaults, use [`Self::new`].
    pub fn to_cell(&self) -> Result<Cell, Error> {
        CellBuilder::build_from(self)
    }
}

/// Unsigned V4R1/V4R2 transfer request with opcode zero.
///
/// The serialized fields are `subwallet_id:uint32`, `valid_until:uint32`, `msg_seqno:uint32`,
/// `opcode:uint8`, then up to four mode/reference pairs.
/// Each mode occupies 8 bits. Each reference contains a complete outgoing message.
/// The reference count determines the message count. There is no count field.
/// The caller supplies the stored subwallet ID, current sequence number, and expiration in Unix seconds.
/// Loading and storing require `opcode == 0`. Plugin-management requests need different body types.
///
/// Layout: <https://docs.ton.org/contracts/standard/wallets/v4>
#[derive(Debug, PartialEq, Clone)]
pub struct WalletV4ExtMsgBody {
    /// Subwallet identifier from the target wallet state.
    pub subwallet_id: i32,
    /// Expiration as Unix time in seconds. The contract requires a future timestamp.
    pub valid_until: u32,
    /// Sequence number that must match the current wallet state.
    pub msg_seqno: u32,
    /// Transfer opcode. Only zero is supported by this body type.
    pub opcode: u8,
    /// Outgoing messages in execution order. Serialization accepts zero to 4 entries.
    pub msgs: Vec<WalletMessage>,
}

impl<'a> Load<'a> for WalletV4ExtMsgBody {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        let subwallet_id = Load::load_from(parser)?;
        let valid_until = Load::load_from(parser)?;
        let msg_seqno = Load::load_from(parser)?;
        let opcode = Load::load_from(parser)?;
        if opcode != 0 {
            return Err(Error::InvalidTag);
        }
        let msgs = read_up_to_4_msgs(parser)?;
        Ok(Self {
            subwallet_id,
            valid_until,
            msg_seqno,
            opcode,
            msgs,
        })
    }
}

impl Store for WalletV4ExtMsgBody {
    fn store_into(&self, dst: &mut CellBuilder, context: &dyn CellContext) -> Result<(), Error> {
        if self.opcode != 0 {
            return Err(Error::InvalidTag);
        }
        self.subwallet_id.store_into(dst, context)?;
        self.valid_until.store_into(dst, context)?;
        self.msg_seqno.store_into(dst, context)?;
        self.opcode.store_into(dst, context)?;
        write_up_to_4_msgs(dst, &self.msgs)?;
        Ok(())
    }
}

impl WalletV4ExtMsgBody {
    /// Builds an unsigned body for Ed25519 signing over its representation hash.
    ///
    /// The caller prepends the 512-bit signature to the body, then adds the external-message envelope.
    /// Values and outgoing message cells are stored as supplied, without checks against account state.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TooManyMessages`] if there are more than 4 messages.
    /// Other cell construction failures are propagated as [`Error`].
    /// Returns [`Error::InvalidTag`] if `opcode` is not zero.
    pub fn to_cell(&self) -> Result<Cell, Error> {
        CellBuilder::build_from(self)
    }

    /// Reads a 512-bit Ed25519 signature followed by an unsigned transfer body.
    ///
    /// The slice must start at the signature, not at the external-message envelope.
    /// Returns the unsigned fields and 64 signature bytes without verifying the signature.
    /// Consumes all remaining message references and their modes. Trailing bits are left in the parser.
    /// On failure, the parser can remain partially consumed.
    ///
    /// # Errors
    ///
    /// Returns a cell loading error for truncated fields, references, or signatures.
    /// Returns [`Error::InvalidTag`] for a nonzero opcode, including plugin-management requests.
    pub fn read_signed(parser: &mut CellSlice<'_>) -> Result<(Self, Vec<u8>), Error> {
        let mut signature = vec![0; 64];
        parser.load_raw(&mut signature, 512)?;
        Ok((Self::load_from(parser)?, signature))
    }
}

#[cfg(test)]
mod tests {
    use crate::boc::{Boc, BocRepr};
    use crate::cell::{CellBuilder, HashBytes};
    use crate::wallet::SendMsgFlags;
    use crate::wallet::{WALLET_ID_DEFAULT, WalletV4Data, WalletV4ExtMsgBody};
    use std::str::FromStr;

    #[test]
    fn test_wallet_data_v4() -> anyhow::Result<()> {
        // https://tonviewer.com/UQCS65EGyiApUTLOYXDs4jOLoQNCE0o8oNnkmfIcm0iX5FRT
        let src_boc_hex = "b5ee9c7241010101002b0000510000001429a9a317cbf377c9b73604c70bf73488ddceba14f763baef2ac70f68d1d6032a120149f440a6c9f37d";
        let wallet_data = BocRepr::decode_hex::<WalletV4Data, _>(src_boc_hex)?;
        assert_eq!(wallet_data.seqno, 20);
        assert_eq!(wallet_data.wallet_id, WALLET_ID_DEFAULT);
        assert_eq!(
            wallet_data.public_key,
            HashBytes::from_str(
                "cbf377c9b73604c70bf73488ddceba14f763baef2ac70f68d1d6032a120149f4"
            )?
        );
        assert_eq!(wallet_data.plugins, None);

        let serial_boc_hex = BocRepr::encode_hex(&wallet_data)?;
        let restored = BocRepr::decode_hex::<WalletV4Data, _>(&serial_boc_hex)?;
        assert_eq!(wallet_data, restored);
        Ok(())
    }

    #[test]
    fn test_wallet_ext_msg_body_v4() -> anyhow::Result<()> {
        // https://tonviewer.com/transaction/891dbceffb986251768d4c33bb8dcf11d522408ff78b8e683d135304ca377b8b
        let body_signed_cell = Boc::decode_hex(
            "b5ee9c7201010201008700019c9dcd3a68926ad6fb9d094c5b72901bfc359ada50f22b648c6c2223c767135d397c7489c121071e45a5316a94a533d80c41450049ebeed406c419fea99117f40629a9a31767ad328900000013000301006842007847b4630eb08d9f486fe846d5496878556dfd5a084f82a9a3fb01224e67c84c200989680000000000000000000000000000",
        )?;
        let mut parser = body_signed_cell.as_slice()?;
        parser.skip_first(512, 0)?;
        let body_no_sign = CellBuilder::build_from(parser.load_remaining())?;

        let body = WalletV4ExtMsgBody::read_signed(&mut body_signed_cell.as_slice()?)?.0;
        assert_eq!(body.subwallet_id, WALLET_ID_DEFAULT);
        assert_eq!(body.valid_until, 1739403913);
        assert_eq!(body.msg_seqno, 19);
        assert_eq!(body.opcode, 0);
        assert_eq!(body.msgs.len(), 1);
        assert_eq!(
            body.msgs[0].mode,
            SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR
        );

        let serial_cell = body.to_cell()?;
        assert_eq!(body_no_sign, serial_cell);
        Ok(())
    }
}
