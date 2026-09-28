//! Highload V2R2 storage for wallets that track query IDs instead of a sequence number.
//!
//! Highload V2 keeps recent query IDs in a dictionary for replay protection.
//! Distinct requests can therefore arrive without a shared sequential counter.
//! This module only models V2R2 storage and its initial state.
//! It does not construct or sign Highload transfer requests.
//! Other Highload revisions have code lookup support but no initial-data builder.
//!
//! Protocol references:
//!
//! - [Highload V2 specification and TL-B layouts](https://docs.ton.org/contracts/standard/wallets/highload/v2/specification).
//! - [Highload V2 family source](https://github.com/ton-blockchain/ton/blob/master/crypto/smartcont/highload-wallet-v2-code.fc),
//!   including replay checks and dictionary cleanup.
//! - [V2R2 Tolk ABI](https://github.com/ton-blockchain/abis/blob/master/data/wallets/highload_v2r2/types/wallet_highload_v2r2.types.tolk)
//!   and [revision metadata](https://github.com/ton-blockchain/abis/blob/master/data/wallets/highload_v2r2/info.toml).
//!
//! The source link describes the V2 family. The catalog identifies V2R2 by its code hash.

use crate::cell::{Cell, CellBuilder, HashBytes, Load, Store};
use crate::error::Error;

/// Persistent Highload V2R2 data for replay protection through stored query IDs.
///
/// The serialized fields are `wallet_id:uint32`, `last_cleaned_time:uint64`,
/// `public_key:bits256`, and a dictionary of 64-bit query IDs with empty values.
/// The dictionary uses a presence bit and an optional root reference.
///
/// Despite its name, `last_cleaned_time` stores the last removed query ID, not a plain timestamp.
/// Query IDs combine an expiration timestamp in the upper 32 bits with a lower 32-bit identifier.
/// This type preserves the raw dictionary root without decoding entries.
#[derive(Clone, Debug, Load, Store)]
pub struct WalletHLV2R2Data {
    /// Wallet identifier encoded as 32 bits. It must match the request and stored state.
    pub wallet_id: i32,
    /// Last removed query ID, including its timestamp and lower identifier bits.
    pub last_cleaned_time: u64,
    /// Ed25519 public key used to verify owner signatures.
    pub public_key: HashBytes,
    /// Raw dictionary of recent query IDs. `None` represents an empty dictionary.
    pub queries: Option<Cell>,
}

impl WalletHLV2R2Data {
    /// Creates Highload V2R2 deployment data with no recorded queries.
    ///
    /// The cleanup watermark starts at query ID zero. It is not initialized from the current time.
    /// The supplied public key and wallet ID are stored unchanged.
    pub fn new(wallet_id: i32, public_key: HashBytes) -> Self {
        Self {
            wallet_id,
            last_cleaned_time: 0,
            public_key,
            queries: None,
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
