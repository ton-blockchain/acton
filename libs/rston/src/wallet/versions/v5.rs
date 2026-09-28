//! Wallet V5R1: extensible wallets with linked action lists and signature controls.
//!
//! The contract accepts signed external requests, signed internal requests, and requests from registered extensions.
//! This module models storage and signed external transfers with up to 255 messages.
//! Internal authentication and extended actions are outside its API.
//! The wallet ID scheme separates networks and supports multiple wallets for one public key.
//!
//! Protocol references:
//!
//! - [Complete TL-B schema](https://github.com/ton-blockchain/wallet-contract-v5/blob/main/types.tlb),
//!   including storage, `OutList`, signed requests, and extended actions.
//! - [FunC contract source](https://github.com/ton-blockchain/wallet-contract-v5/blob/main/contracts/wallet_v5.fc).
//! - [V5 behavior and wallet ID scheme](https://docs.ton.org/contracts/standard/wallets/v5).
//! - [Message and getter reference](https://docs.ton.org/contracts/standard/wallets/v5-api).
//! - [Tolk ABI definitions](https://github.com/ton-blockchain/abis/blob/master/data/wallets/w5r1/types/wallet_v5r1.types.tolk)
//!   and [code hashes and fixtures](https://github.com/ton-blockchain/abis/tree/master/data/wallets/w5r1).

use crate::cell::{Cell, CellBuilder, CellContext, CellFamily, CellSlice, HashBytes, Load, Store};
use crate::error::Error;
use crate::models::OutAction;
use crate::wallet::WalletMessage;

/// Persistent V5R1 data, including signature controls and registered extensions.
///
/// The serialized fields are `sign_allowed:bit`, `seqno:uint32`, `wallet_id:uint32`,
/// `public_key:bits256`, and `extensions:(HashmapE 256 int1)`.
/// The Rust `i32` wallet ID preserves all 32 wire bits.
/// The extensions dictionary is stored as a presence bit and an optional root reference.
/// This type preserves its raw root without decoding entries.
///
/// Schema: <https://github.com/ton-blockchain/wallet-contract-v5/blob/main/types.tlb>
#[derive(Debug, PartialEq, Clone, Load, Store)]
pub struct WalletV5Data {
    /// Controls signature authentication while registered extensions exist.
    ///
    /// If the extensions dictionary is empty, the contract also accepts valid owner signatures with this flag cleared.
    /// Registered extensions can change the flag through extended actions.
    pub sign_allowed: bool,
    /// Stored sequence number for replay protection. Initial data uses zero.
    pub seqno: u32,
    /// Wallet identifier encoded as 32 bits. It must match the request and stored state.
    pub wallet_id: i32,
    /// Ed25519 public key used to verify owner signatures.
    pub public_key: HashBytes,
    /// Raw extension dictionary root. `None` represents an empty dictionary.
    pub extensions: Option<Cell>,
}

impl WalletV5Data {
    /// Creates V5R1 deployment data with signature authentication enabled and sequence number zero.
    ///
    /// The extensions dictionary starts empty. The public key and `wallet_id` are stored unchanged.
    /// The caller supplies an ID for the intended network, workchain, and subwallet.
    pub fn new(wallet_id: i32, public_key: HashBytes) -> Self {
        Self {
            sign_allowed: true,
            seqno: 0,
            wallet_id,
            public_key,
            extensions: None,
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

/// Unsigned V5R1 external transfer request with the `sign` operation prefix.
///
/// The serialized fields are `0x7369676e:uint32`, `wallet_id:uint32`, `valid_until:uint32`,
/// `msg_seqno:uint32`, and the inner request. Expiration is a Unix timestamp in seconds.
/// The inner request contains an optional `OutList` reference and no extended actions.
/// Each action contains a previous-list reference, `action_send_msg#0ec3c86d`,
/// an 8-bit mode, and a complete outgoing message reference.
/// Serialization accepts up to 255 messages and preserves their execution order.
/// Loading does not enforce the action-count limit or validate outgoing message contents.
///
/// The contract requires [`crate::wallet::SendMsgFlags::IGNORE_ERROR`] on every external transfer.
/// The serializer preserves the caller's modes without adding this flag.
/// Successful serialization does not imply that the contract will accept the request.
///
/// Schema: <https://github.com/ton-blockchain/wallet-contract-v5/blob/main/types.tlb>
#[derive(Debug, PartialEq, Clone)]
pub struct WalletV5ExtMsgBody {
    /// Wallet identifier encoded as 32 bits. It must match the request and stored state.
    pub wallet_id: i32,
    /// Expiration as Unix time in seconds. The contract requires a future timestamp.
    pub valid_until: u32,
    /// Sequence number that must match the current wallet state.
    pub msg_seqno: u32,
    /// Outgoing messages in execution order. Serialization accepts zero to 255 entries.
    pub msgs: Vec<WalletMessage>,
}

impl<'a> Load<'a> for WalletV5ExtMsgBody {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        if parser.load_u32()? != 0x7369676e {
            return Err(Error::InvalidTag);
        }
        let wallet_id = Load::load_from(parser)?;
        let valid_until = Load::load_from(parser)?;
        let msg_seqno = Load::load_from(parser)?;
        let inner_request = WalletV5InnerRequest::load_from(parser)?;
        let msgs = parse_v5_inner_request(inner_request)?;
        Ok(Self {
            wallet_id,
            valid_until,
            msg_seqno,
            msgs,
        })
    }
}

impl Store for WalletV5ExtMsgBody {
    fn store_into(&self, dst: &mut CellBuilder, context: &dyn CellContext) -> Result<(), Error> {
        dst.store_u32(0x7369676e)?;
        self.wallet_id.store_into(dst, context)?;
        self.valid_until.store_into(dst, context)?;
        self.msg_seqno.store_into(dst, context)?;
        let inner_req = build_v5_inner_request(&self.msgs)?;
        inner_req.store_into(dst, context)?;
        Ok(())
    }
}

impl WalletV5ExtMsgBody {
    /// Builds an unsigned body for Ed25519 signing over its representation hash.
    ///
    /// The caller appends the 512-bit signature after the body, then adds the external-message envelope.
    /// Values and outgoing message cells are stored as supplied, without checks against account state.
    ///
    /// # Errors
    ///
    /// Returns [`Error::TooManyMessages`] if there are more than 255 messages.
    /// Other cell construction failures are propagated as [`Error`].
    pub fn to_cell(&self) -> Result<Cell, Error> {
        CellBuilder::build_from(self)
    }

    /// Reads an external transfer body followed by its 512-bit Ed25519 signature.
    ///
    /// Returns the unsigned fields and 64 signature bytes without verifying the signature.
    /// Empty transfer lists are supported. Extended actions and other request prefixes are unsupported.
    /// The parser advances through the body and signature, without checking that the slice is exhausted.
    /// On failure, the parser can remain partially consumed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidTag`] for a prefix other than `sign` (`0x7369676e`).
    /// Returns [`Error::InvalidData`] for extended actions or an action other than `action_send_msg`.
    /// Truncated fields, references, or signatures return a cell loading error.
    pub fn read_signed(parser: &mut CellSlice<'_>) -> Result<(Self, Vec<u8>), Error> {
        let body = Self::load_from(parser)?;
        let mut signature = vec![0; 64];
        parser.load_raw(&mut signature, 512)?;
        Ok((body, signature))
    }
}

/// Transfer actions from `W5InnerRequest`, without extended actions.
#[derive(Debug, PartialEq, Clone)]
struct WalletV5InnerRequest {
    /// Raw `OutList` root. `None` represents an empty transfer list.
    out_actions: Option<Cell>,
}

impl<'a> Load<'a> for WalletV5InnerRequest {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        let out_actions = Option::<Cell>::load_from(parser)?;
        if parser.load_bit()? {
            return Err(Error::InvalidData);
        }
        Ok(Self { out_actions })
    }
}

impl Store for WalletV5InnerRequest {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        builder.store_bit(self.out_actions.is_some())?;
        if let Some(actions) = &self.out_actions {
            builder.store_reference(actions.clone())?;
        }
        builder.store_bit(false)?; // other_actions are not supported
        Ok(())
    }
}

/// Reads send actions from the reverse-linked `OutList` and restores execution order.
/// The caller is responsible for validating list size and outgoing message contents.
fn parse_v5_inner_request(request: WalletV5InnerRequest) -> Result<Vec<WalletMessage>, Error> {
    let mut out_list = match request.out_actions {
        Some(out_list) => out_list,
        None => return Ok(vec![]),
    };
    let mut msgs = vec![];
    while out_list.bit_len() != 0 {
        let mut parser = out_list.as_slice()?;
        let next = parser.load_reference_cloned()?;
        if parser.load_u32()? == OutAction::TAG_SEND_MSG {
            msgs.push(WalletMessage::load_from(&mut parser)?);
        } else {
            return Err(Error::InvalidData);
        }
        out_list = next;
    }

    msgs.reverse();
    Ok(msgs)
}

/// Encodes up to 255 send actions in a reverse-linked list, without extended actions.
/// An empty slice encodes an absent list rather than a reference to an empty cell.
fn build_v5_inner_request(msgs: &[WalletMessage]) -> Result<WalletV5InnerRequest, Error> {
    if msgs.is_empty() {
        return Ok(WalletV5InnerRequest { out_actions: None });
    }

    if msgs.len() > 255 {
        return Err(Error::TooManyMessages {
            actual: msgs.len(),
            max: 255,
        });
    }

    let mut actions = Cell::empty_cell();
    for msg in msgs {
        let mut builder = CellBuilder::new();
        builder.store_reference(actions)?;
        builder.store_u32(OutAction::TAG_SEND_MSG)?;
        msg.store_into(&mut builder, Cell::empty_context())?;
        actions = builder.build()?;
    }

    Ok(WalletV5InnerRequest {
        out_actions: Some(actions),
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::boc::{Boc, BocRepr};
    use crate::models::{OwnedRelaxedMessage, RelaxedMsgInfo};
    use crate::wallet::SendMsgFlags;
    use crate::wallet::{WALLET_V5R1_ID_DEFAULT, WALLET_V5R1_ID_DEFAULT_TESTNET};
    use std::str::FromStr;

    #[test]
    fn test_empty_signed_request_roundtrip() -> anyhow::Result<()> {
        let body = WalletV5ExtMsgBody {
            wallet_id: WALLET_V5R1_ID_DEFAULT,
            valid_until: u32::MAX,
            msg_seqno: 0,
            msgs: vec![],
        };
        let signature = [0xa5; 64];
        let mut builder = CellBuilder::new();
        builder.store_slice(body.to_cell()?.as_slice()?)?;
        builder.store_raw(&signature, 512)?;
        let cell = builder.build()?;

        let mut parser = cell.as_slice()?;
        let (parsed_body, parsed_signature) = WalletV5ExtMsgBody::read_signed(&mut parser)?;

        assert_eq!(parsed_body, body);
        assert_eq!(parsed_signature, signature);
        assert_eq!(parser.size_bits(), 0);
        assert_eq!(parser.size_refs(), 0);
        Ok(())
    }

    #[test]
    fn test_wallet_data_v5() -> anyhow::Result<()> {
        // https://tonviewer.com/UQDwj2jGHWEbPpDf0I2qktDwqtv6tBCfBVNH9gJEnM-QmHDa
        let src_boc_hex = "b5ee9c7241010101002b00005180000000bfffff88e5f9bbe4db9b026385fb9a446ee75d0a7bb1dd77956387b468eb01950900a4fa20cbe13a2a";
        let wallet_data = BocRepr::decode_hex::<WalletV5Data, _>(src_boc_hex)?;
        assert_eq!(wallet_data.seqno, 1);
        assert_eq!(wallet_data.wallet_id, WALLET_V5R1_ID_DEFAULT);
        assert_eq!(
            wallet_data.public_key,
            HashBytes::from_str(
                "cbf377c9b73604c70bf73488ddceba14f763baef2ac70f68d1d6032a120149f4"
            )?
        );
        assert_eq!(wallet_data.extensions, None);

        let cell = wallet_data.to_cell()?;
        let mut bytes = Vec::new();
        crate::boc::ser::BocHeader::<ahash::RandomState>::with_root(cell.as_ref())
            .with_crc(true)
            .encode(&mut bytes);
        let serial_boc_hex = hex::encode(bytes);
        assert_eq!(src_boc_hex, serial_boc_hex);
        let restored = BocRepr::decode_hex::<WalletV5Data, _>(&serial_boc_hex)?;
        assert_eq!(wallet_data, restored);
        Ok(())
    }

    #[test]
    fn test_wallet_data_v5_testnet() -> anyhow::Result<()> {
        let src_boc_hex = "b5ee9c7201010101002b000051800000013ffffffed2b31b23dbe5144a626b9d5d1d4208e36d97e4adb472d42c073bfff85b3107e4a0";
        let wallet_data = BocRepr::decode_hex::<WalletV5Data, _>(src_boc_hex)?;
        assert_eq!(wallet_data.seqno, 2);
        assert_eq!(wallet_data.wallet_id, WALLET_V5R1_ID_DEFAULT_TESTNET);
        Ok(())
    }

    #[test]
    fn test_wallet_ext_msg_body_v5() -> anyhow::Result<()> {
        // https://tonviewer.com/transaction/b4c5eddc52d0e23dafb2da6d022a5b6ae7eba52876fa75d32b2a95fa30c7e2f0
        let body_signed_cell = Boc::decode_hex(
            "b5ee9c720101040100940001a17369676e7fffff11ffffffff00000000bc04889cb28b36a3a00810e363a413763ec34860bf0fce552c5d36e37289fafd442f1983d740f92378919d969dd530aec92d258a0779fb371d4659f10ca1b3826001020a0ec3c86d030302006642007847b4630eb08d9f486fe846d5496878556dfd5a084f82a9a3fb01224e67c84c187a1200000000000000000000000000000000",
        )?;
        let mut parser = body_signed_cell.as_slice()?;
        parser.skip_first(body_signed_cell.bit_len() - 512, 0)?;
        let mut sign = vec![0; 64];
        parser.load_raw(&mut sign, 512)?;

        let body = WalletV5ExtMsgBody::read_signed(&mut body_signed_cell.as_slice()?)?.0;

        assert_eq!(body.wallet_id, WALLET_V5R1_ID_DEFAULT);
        assert_eq!(body.valid_until, 4294967295);
        assert_eq!(body.msg_seqno, 0);
        assert_eq!(body.msgs.len(), 1);
        assert_eq!(
            body.msgs[0].mode,
            SendMsgFlags::PAY_FEE_SEPARATELY | SendMsgFlags::IGNORE_ERROR
        );

        let mut signed_builder = CellBuilder::new();
        signed_builder.store_slice(CellBuilder::build_from(&body)?.as_slice()?)?;
        signed_builder.store_raw(&sign, 512)?;
        let signed_serial = signed_builder.build()?;
        assert_eq!(body_signed_cell, signed_serial);
        let parsed_back = signed_serial.parse::<WalletV5ExtMsgBody>()?;
        assert_eq!(body, parsed_back);
        Ok(())
    }

    #[test]
    fn test_wallet_ext_msg_body_v5_order() -> anyhow::Result<()> {
        let body_signed = Boc::decode_hex(
            "b5ee9c7201020a0100014a0001a17369676e7fffff1168f168d80000010db3172ba8e30d093aa36ec6e9e1f1f874cf38fb4a7914c66a0afcbd562ec6b54537629c135ce86700ae64b87dfc85545b34d561a3f7b429512c8cc02edbedc0c2a001020a0ec3c86d030203020a0ec3c86d03040500644200658250f64e6b787bcaa22ed0e90f9a348a637e5f1e9f2c997923bd441a3dd24911389800000000000000000000000000020a0ec3c86d03060700644200658250f64e6b787bcaa22ed0e90f9a348a637e5f1e9f2c997923bd441a3dd24911389000000000000000000000000000020a0ec3c86d03080900644200658250f64e6b787bcaa22ed0e90f9a348a637e5f1e9f2c997923bd441a3dd24911388800000000000000000000000000000000644200658250f64e6b787bcaa22ed0e90f9a348a637e5f1e9f2c997923bd441a3dd24911388000000000000000000000000000",
        )?;
        let mut parser = body_signed.as_slice()?;
        parser.skip_first(body_signed.bit_len() - 512, 0)?;
        let mut sign = vec![0; 64];
        parser.load_raw(&mut sign, 512)?;

        let body = WalletV5ExtMsgBody::read_signed(&mut body_signed.as_slice()?)?.0;
        assert_eq!(body.msgs.len(), 4);

        let RelaxedMsgInfo::Int(info) = body.msgs[0].msg.parse::<OwnedRelaxedMessage>()?.info
        else {
            panic!("Expected internal message");
        };
        assert_eq!(info.value.tokens.into_inner(), 10000);
        let RelaxedMsgInfo::Int(info) = body.msgs[1].msg.parse::<OwnedRelaxedMessage>()?.info
        else {
            panic!("Expected internal message");
        };
        assert_eq!(info.value.tokens.into_inner(), 10001);
        let RelaxedMsgInfo::Int(info) = body.msgs[2].msg.parse::<OwnedRelaxedMessage>()?.info
        else {
            panic!("Expected internal message");
        };
        assert_eq!(info.value.tokens.into_inner(), 10002);
        let RelaxedMsgInfo::Int(info) = body.msgs[3].msg.parse::<OwnedRelaxedMessage>()?.info
        else {
            panic!("Expected internal message");
        };
        assert_eq!(info.value.tokens.into_inner(), 10003);

        let mut signed_builder = CellBuilder::new();
        signed_builder.store_slice(CellBuilder::build_from(&body)?.as_slice()?)?;
        signed_builder.store_raw(&sign, 512)?;
        let signed_serial = signed_builder.build()?;
        assert_eq!(body_signed, signed_serial);
        let parsed_back = signed_serial.parse::<WalletV5ExtMsgBody>()?;
        assert_eq!(body, parsed_back);
        Ok(())
    }
}
