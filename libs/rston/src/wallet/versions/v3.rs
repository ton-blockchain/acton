//! Wallet V3R1 and V3R2: sequence-number wallets with a subwallet identifier.
//!
//! V3 lets one public key control several wallets through different subwallet identifiers.
//! The identifier forms part of the initial data, so changing it changes the address.
//! R1 and R2 share data and transfer layouts, but have different code hashes.
//! This module supports storage and ordinary signed transfer requests for both revisions.
//!
//! Protocol references:
//!
//! - [V3 layout, revisions, and getters in TON Docs](https://docs.ton.org/contracts/standard/wallets/history#wallet-v3).
//! - [FunC parser and storage serialization](https://github.com/ton-blockchain/ton/blob/master/crypto/smartcont/wallet3-code.fc).
//! - [Fift contract source](https://github.com/ton-blockchain/ton/blob/master/crypto/smartcont/wallet-v3-code.fif).
//! - Tolk ABI definitions: [V3R1](https://github.com/ton-blockchain/abis/blob/master/data/wallets/w3r1/types/wallet_v3r1.types.tolk)
//!   and [V3R2](https://github.com/ton-blockchain/abis/blob/master/data/wallets/w3r2/types/wallet_v3r2.types.tolk).
//!
//! The [ABI catalog](https://github.com/ton-blockchain/abis/tree/master/data/wallets/w3r2)
//! also includes the code hash, getter signatures, and sample contract data.

use crate::cell::{Cell, CellBuilder, CellContext, CellSlice, HashBytes, Load, Store};
use crate::error::Error;
use crate::wallet::WalletMessage;
use crate::wallet::versions::message_utils::{read_up_to_4_msgs, write_up_to_4_msgs};

/// Persistent data for V3R1 and V3R2 wallets.
///
/// The serialized fields are `seqno:uint32`, `wallet_id:uint32`, and `public_key:bits256`.
/// The Rust `i32` wallet ID preserves all 32 wire bits.
/// The ID is part of the initial state, so it affects the wallet address.
#[derive(Debug, PartialEq, Clone, Load, Store)]
pub struct WalletV3Data {
    /// Stored sequence number for replay protection. Initial data uses zero.
    pub seqno: u32,
    /// Wallet identifier encoded as 32 bits. It must match the request and stored state.
    pub wallet_id: i32,
    /// Ed25519 public key used to verify owner signatures.
    pub public_key: HashBytes,
}

impl WalletV3Data {
    /// Creates deployment data for a V3 wallet with sequence number zero.
    ///
    /// `wallet_id` and the public key are stored unchanged. The ID selects the subwallet address.
    /// The constructor does not derive a network or workchain-specific ID.
    pub fn new(wallet_id: i32, public_key: HashBytes) -> Self {
        Self {
            seqno: 0,
            wallet_id,
            public_key,
        }
    }

    /// Serializes this state as the wallet's persistent data cell.
    ///
    /// Preserves the current field values. For deployment defaults, use [`Self::new`].
    pub fn to_cell(&self) -> Result<Cell, Error> {
        CellBuilder::build_from(self)
    }
}

/// Unsigned transfer request for V3R1 and V3R2 wallets.
///
/// The serialized fields are `subwallet_id:uint32`, `valid_until:uint32`, `msg_seqno:uint32`,
/// then up to four entries with an 8-bit mode and a complete outgoing message reference.
/// The reference count determines the message count. There is no count field.
/// The Rust `i32` identifier preserves all 32 wire bits.
/// The caller supplies the stored subwallet ID, current sequence number, and expiration in Unix seconds.
///
/// The [contract parser](https://github.com/ton-blockchain/ton/blob/master/crypto/smartcont/wallet3-code.fc)
/// defines the field order: expiration precedes the sequence number.
#[derive(Debug, PartialEq, Clone)]
pub struct WalletV3ExtMsgBody {
    /// Subwallet identifier from the target wallet state.
    pub subwallet_id: i32,
    /// Expiration as Unix time in seconds. The contract requires a future timestamp.
    pub valid_until: u32,
    /// Sequence number that must match the current wallet state.
    pub msg_seqno: u32,
    /// Outgoing messages in execution order. Serialization accepts zero to 4 entries.
    pub msgs: Vec<WalletMessage>,
}

impl<'a> Load<'a> for WalletV3ExtMsgBody {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        let subwallet_id = Load::load_from(parser)?;
        let valid_until = Load::load_from(parser)?;
        let msg_seqno = Load::load_from(parser)?;
        let msgs = read_up_to_4_msgs(parser)?;
        Ok(Self {
            subwallet_id,
            msg_seqno,
            valid_until,
            msgs,
        })
    }
}

impl Store for WalletV3ExtMsgBody {
    fn store_into(&self, dst: &mut CellBuilder, context: &dyn CellContext) -> Result<(), Error> {
        self.subwallet_id.store_into(dst, context)?;
        self.valid_until.store_into(dst, context)?;
        self.msg_seqno.store_into(dst, context)?;
        write_up_to_4_msgs(dst, &self.msgs)?;
        Ok(())
    }
}

impl WalletV3ExtMsgBody {
    /// Builds an unsigned body for Ed25519 signing over its representation hash.
    ///
    /// The caller prepends the 512-bit signature to the body, then adds the external-message envelope.
    /// Values and outgoing message cells are stored as supplied, without checks against account state.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TooManyMessages`] if there are more than 4 messages.
    /// Other cell construction failures are propagated as [`Error`].
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
    use crate::wallet::{WALLET_ID_DEFAULT, WalletV3Data, WalletV3ExtMsgBody};
    use std::str::FromStr;

    #[test]
    fn test_wallet_v3_data() -> anyhow::Result<()> {
        // https://tonviewer.com/UQAMY2B4xfQO6m3YpmzfX5Za-Ning4kWKFjPdubbPPV3Ffel
        let src_boc_hex = "b5ee9c7241010101002a0000500000000129a9a317cbf377c9b73604c70bf73488ddceba14f763baef2ac70f68d1d6032a120149f4b6de3f10";
        let wallet_data = BocRepr::decode_hex::<WalletV3Data, _>(src_boc_hex)?;
        assert_eq!(wallet_data.seqno, 1);
        assert_eq!(wallet_data.wallet_id, WALLET_ID_DEFAULT);
        assert_eq!(
            wallet_data.public_key,
            HashBytes::from_str(
                "cbf377c9b73604c70bf73488ddceba14f763baef2ac70f68d1d6032a120149f4"
            )?
        );
        let serial_boc_hex = BocRepr::encode_hex(&wallet_data)?;
        let restored = BocRepr::decode_hex::<WalletV3Data, _>(&serial_boc_hex)?;
        assert_eq!(wallet_data, restored);
        Ok(())
    }

    #[test]
    fn test_wallet_v3_ext_msg_body() -> anyhow::Result<()> {
        // https://tonviewer.com/transaction/b4bd316c74b4c99586e07c167979ce4a6e18db31704abd7e85b1cacb065ce66c
        let body_signed_cell = Boc::decode_hex(
            "b5ee9c7201010201008500019a86be376ea96e2f1252377976716a3d252906151feabc8e4b51506405035e45a7b4ff81f783cfe3f86483c822bcbb4f9481804990868bac69caf7af56e30fe70b29a9a317ffffffff000000000301006642007847b4630eb08d9f486fe846d5496878556dfd5a084f82a9a3fb01224e67c84c187a120000000000000000000000000000",
        )?;
        let mut parser = body_signed_cell.as_slice()?;
        parser.skip_first(512, 0)?;
        let body_no_sign = CellBuilder::build_from(parser.load_remaining())?;

        let body = WalletV3ExtMsgBody::read_signed(&mut body_signed_cell.as_slice()?)?.0;
        assert_eq!(body.subwallet_id, WALLET_ID_DEFAULT);
        assert_eq!(body.msg_seqno, 0);
        assert_eq!(body.valid_until, 4294967295);
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
