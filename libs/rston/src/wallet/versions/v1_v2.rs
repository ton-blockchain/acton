//! Wallet V1 and V2: sequence-number wallets with a shared storage layout.
//!
//! V1 uses an Ed25519 signature and a sequence number for authorization and replay protection.
//! V2 adds request expiration. Revisions retain the storage layout but have different code hashes.
//! This module supports V1/V2 storage and V2 transfer bodies. V1 requests have no body type here.
//!
//! Protocol references:
//!
//! - [V1/V2 layouts and revision hashes in TON Docs](https://docs.ton.org/contracts/standard/wallets/history).
//! - [V1 Fift source](https://github.com/ton-blockchain/ton/blob/master/crypto/smartcont/new-wallet.fif).
//! - [V2 FunC source](https://github.com/ton-blockchain/ton/blob/master/crypto/smartcont/wallet-code.fc).
//! - [V2 Fift source](https://github.com/ton-blockchain/ton/blob/master/crypto/smartcont/new-wallet-v2.fif).
//! - Tolk ABI definitions: [V1R3](https://github.com/ton-blockchain/abis/blob/master/data/wallets/w1r3/types/wallet_v1r3.types.tolk)
//!   and [V2R2](https://github.com/ton-blockchain/abis/blob/master/data/wallets/w2r2/types/wallet_v2r2.types.tolk).
//!
//! The contract sources define the bit and reference layout for these versions.
//! The [ABI catalog](https://github.com/ton-blockchain/abis/tree/master/data/wallets)
//! contains the other revisions, their code hashes, getters, and message definitions.

use crate::cell::{Cell, CellBuilder, CellContext, CellSlice, HashBytes, Load, Store};
use crate::error::Error;
use crate::wallet::WalletMessage;
use crate::wallet::versions::message_utils::{read_up_to_4_msgs, write_up_to_4_msgs};

/// Persistent data shared by V1 and V2 wallet revisions.
///
/// The serialized fields are `seqno:uint32` and `public_key:bits256`, in that order.
/// The contract checks each signed request against the stored sequence number for replay protection.
#[derive(Debug, PartialEq, Clone, Load, Store)]
pub struct WalletV1V2Data {
    /// Stored sequence number for replay protection. Initial data uses zero.
    pub seqno: u32,
    /// Ed25519 public key used to verify owner signatures.
    pub public_key: HashBytes,
}

impl WalletV1V2Data {
    /// Creates deployment data for a V1 or V2 wallet with sequence number zero.
    ///
    /// The supplied public key is stored unchanged. Its Ed25519 validity is not checked.
    /// Pair this data with the chosen revision's code to derive its address.
    pub fn new(public_key: HashBytes) -> Self {
        Self {
            seqno: 0,
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

/// Unsigned transfer request for V2R1 and V2R2 wallets.
///
/// The serialized fields are `msg_seqno:uint32`, `valid_until:uint32`, then up to four
/// entries with an 8-bit send mode and a complete outgoing message reference.
/// The reference count determines the message count. There is no count field.
/// The caller supplies the current sequence number and expiration in Unix seconds.
/// V1 has no expiration field and cannot use this body type.
///
/// Layout: <https://docs.ton.org/contracts/standard/wallets/history#wallet-v2>
#[derive(Debug, PartialEq, Clone)]
pub struct WalletV2ExtMsgBody {
    /// Sequence number that must match the current wallet state.
    pub msg_seqno: u32,
    /// Expiration as Unix time in seconds. The contract requires a future timestamp.
    pub valid_until: u32,
    /// Outgoing messages in execution order. Serialization accepts zero to 4 entries.
    pub msgs: Vec<WalletMessage>,
}

impl<'a> Load<'a> for WalletV2ExtMsgBody {
    fn load_from(parser: &mut CellSlice<'a>) -> Result<Self, Error> {
        let msg_seqno = Load::load_from(parser)?;
        let valid_until = Load::load_from(parser)?;
        let msgs = read_up_to_4_msgs(parser)?;
        Ok(Self {
            msg_seqno,
            valid_until,
            msgs,
        })
    }
}

impl Store for WalletV2ExtMsgBody {
    fn store_into(&self, dst: &mut CellBuilder, context: &dyn CellContext) -> Result<(), Error> {
        self.msg_seqno.store_into(dst, context)?;
        self.valid_until.store_into(dst, context)?;
        write_up_to_4_msgs(dst, &self.msgs)?;
        Ok(())
    }
}

impl WalletV2ExtMsgBody {
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
