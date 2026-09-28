//! Transaction models.

pub use self::phases::*;
use crate::cell::*;
use crate::dict::{self, Dict};
use crate::error::*;
use crate::models::account::AccountStatus;
use crate::models::currency::CurrencyCollection;
use crate::models::message::Message;
use crate::num::*;

mod phases;

#[cfg(test)]
mod tests;

/// Blockchain transaction.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Transaction {
    /// Account on which this transaction was produced.
    pub account: HashBytes,
    /// Logical time when the transaction was created.
    pub lt: u64,
    /// The hash of the previous transaction on the same account.
    pub prev_trans_hash: HashBytes,
    /// The logical time of the previous transaction on the same account.
    pub prev_trans_lt: u64,
    /// Unix timestamp when the transaction was created.
    pub now: u32,
    /// The number of outgoing messages.
    pub out_msg_count: Uint15,
    /// Account status before this transaction.
    pub orig_status: AccountStatus,
    /// Account status after this transaction.
    pub end_status: AccountStatus,
    /// Optional incoming message.
    #[cfg_attr(feature = "serde", serde(with = "serde_in_msg"))]
    pub in_msg: Option<Cell>,
    /// Outgoing messages.
    #[cfg_attr(feature = "serde", serde(with = "serde_out_msgs"))]
    pub out_msgs: Dict<Uint15, Cell>,
    /// Total transaction fees (including extra fwd fees).
    pub total_fees: CurrencyCollection,
    /// Account state hashes.
    pub state_update: Lazy<HashUpdate>,
    /// Detailed transaction info.
    pub info: Lazy<TxInfo>,
}

impl Transaction {
    /// Tries to load the incoming message, if present.
    pub fn load_in_msg(&self) -> Result<Option<Message<'_>>, Error> {
        match &self.in_msg {
            Some(in_msg) => match in_msg.parse::<Message>() {
                Ok(message) => Ok(Some(message)),
                Err(e) => Err(e),
            },
            None => Ok(None),
        }
    }

    /// Tries to load the detailed transaction info from the lazy cell.
    pub fn load_info(&self) -> Result<TxInfo, Error> {
        self.info.load()
    }
}

impl Transaction {
    /// Gets an iterator over the output messages of this transaction, in order by lt.
    /// The iterator element type is `Result<Message<'a>>`.
    ///
    /// If the dictionary or message is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn iter_out_msgs(&'_ self) -> TxOutMsgIter<'_> {
        TxOutMsgIter {
            inner: self.out_msgs.raw_values(),
        }
    }
}

#[cfg(feature = "serde")]
mod serde_in_msg {
    use super::*;

    pub fn serialize<S>(in_msg: &Option<Cell>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            match in_msg {
                Some(in_msg) => {
                    let message = ok!(in_msg.parse::<Message>().map_err(serde::ser::Error::custom));
                    serializer.serialize_some(&message)
                }
                None => serializer.serialize_none(),
            }
        } else {
            crate::boc::Boc::serialize(in_msg, serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Cell>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Deserialize, Error};

        if deserializer.is_human_readable() {
            match ok!(Option::<crate::models::OwnedMessage>::deserialize(
                deserializer
            )) {
                Some(message) => CellBuilder::build_from(&message)
                    .map_err(Error::custom)
                    .map(Some),
                None => Ok(None),
            }
        } else {
            crate::boc::Boc::deserialize(deserializer)
        }
    }
}

#[cfg(feature = "serde")]
mod serde_out_msgs {
    use super::*;

    pub fn serialize<S>(out_msgs: &Dict<Uint15, Cell>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::{Error, SerializeMap};

        if serializer.is_human_readable() {
            let mut map = ok!(serializer.serialize_map(None));
            for entry in out_msgs.iter() {
                match entry {
                    Ok((key, value)) => {
                        let message = ok!(value.parse::<Message>().map_err(Error::custom));
                        ok!(map.serialize_entry(&key, &message));
                    }
                    Err(e) => return Err(Error::custom(e)),
                }
            }
            map.end()
        } else {
            crate::boc::BocRepr::serialize(out_msgs, serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Dict<Uint15, Cell>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Deserialize, Error};

        if deserializer.is_human_readable() {
            let messages = ok!(
                ahash::HashMap::<Uint15, crate::models::OwnedMessage>::deserialize(deserializer)
            );

            let cx = Cell::empty_context();
            let mut dict = Dict::new();
            for (key, value) in &messages {
                let cell = ok!(CellBuilder::build_from(value).map_err(Error::custom));
                ok!(dict.set_ext(*key, cell, cx).map_err(Error::custom));
            }
            Ok(dict)
        } else {
            crate::boc::BocRepr::deserialize(deserializer)
        }
    }
}

/// An iterator over the transaction output messages.
///
/// This struct is created by the [`iter_out_msgs`] method on [`Transaction`].
/// See its documentation for more.
///
/// [`iter_out_msgs`]: Transaction::iter_out_msgs
#[derive(Clone)]
pub struct TxOutMsgIter<'a> {
    inner: dict::RawValues<'a>,
}

impl<'a> Iterator for TxOutMsgIter<'a> {
    type Item = Result<Message<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Ok(mut value) => {
                let e = match value.load_reference_as_slice() {
                    Ok(mut value) => match Message::<'a>::load_from(&mut value) {
                        Ok(message) => return Some(Ok(message)),
                        Err(e) => e,
                    },
                    Err(e) => e,
                };

                Some(Err(self.inner.finish(e)))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

impl Transaction {
    const TAG: u8 = 0b0111;
}

impl Store for Transaction {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let messages = {
            let mut builder = CellBuilder::new();
            ok!(self.in_msg.store_into(&mut builder, context));
            ok!(self.out_msgs.store_into(&mut builder, context));
            ok!(builder.build_ext(context))
        };

        ok!(builder.store_small_uint(Self::TAG, 4));
        ok!(builder.store_u256(&self.account));
        ok!(builder.store_u64(self.lt));
        ok!(builder.store_u256(&self.prev_trans_hash));
        ok!(builder.store_u64(self.prev_trans_lt));
        ok!(builder.store_u32(self.now));
        ok!(self.out_msg_count.store_into(builder, context));
        ok!(self.orig_status.store_into(builder, context));
        ok!(self.end_status.store_into(builder, context));
        ok!(builder.store_reference(messages));
        ok!(self.total_fees.store_into(builder, context));
        ok!(self.state_update.store_into(builder, context));
        self.info.store_into(builder, context)
    }
}

impl<'a> Load<'a> for Transaction {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_small_uint(4) {
            Ok(Self::TAG) => {}
            Ok(_) => return Err(Error::InvalidTag),
            Err(e) => return Err(e),
        }

        let (in_msg, out_msgs) = {
            let slice = &mut ok!(slice.load_reference_as_slice());
            let in_msg = ok!(Option::<Cell>::load_from(slice));
            let out_msgs = ok!(Dict::load_from(slice));
            (in_msg, out_msgs)
        };

        Ok(Self {
            account: ok!(slice.load_u256()),
            lt: ok!(slice.load_u64()),
            prev_trans_hash: ok!(slice.load_u256()),
            prev_trans_lt: ok!(slice.load_u64()),
            now: ok!(slice.load_u32()),
            out_msg_count: ok!(Uint15::load_from(slice)),
            orig_status: ok!(AccountStatus::load_from(slice)),
            end_status: ok!(AccountStatus::load_from(slice)),
            in_msg,
            out_msgs,
            total_fees: ok!(CurrencyCollection::load_from(slice)),
            state_update: ok!(Lazy::<HashUpdate>::load_from(slice)),
            info: ok!(Lazy::<TxInfo>::load_from(slice)),
        })
    }
}

/// Detailed transaction info.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "ty"))]
pub enum TxInfo {
    /// Ordinary transaction info.
    Ordinary(OrdinaryTxInfo),
    /// Tick-tock transaction info.
    TickTock(TickTockTxInfo),
}

impl Store for TxInfo {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        match self {
            Self::Ordinary(info) => {
                ok!(builder.store_small_uint(0b0000, 4));
                info.store_into(builder, context)
            }
            Self::TickTock(info) => {
                ok!(builder.store_small_uint(0b001, 3));
                info.store_into(builder, context)
            }
        }
    }
}

impl<'a> Load<'a> for TxInfo {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let tag_part = ok!(slice.load_small_uint(3));
        Ok(if tag_part == 0b001 {
            match TickTockTxInfo::load_from(slice) {
                Ok(info) => Self::TickTock(info),
                Err(e) => return Err(e),
            }
        } else if tag_part == 0b000 && !ok!(slice.load_bit()) {
            match OrdinaryTxInfo::load_from(slice) {
                Ok(info) => Self::Ordinary(info),
                Err(e) => return Err(e),
            }
        } else {
            return Err(Error::InvalidTag);
        })
    }
}

impl From<OrdinaryTxInfo> for TxInfo {
    #[inline]
    fn from(value: OrdinaryTxInfo) -> Self {
        Self::Ordinary(value)
    }
}

impl From<TickTockTxInfo> for TxInfo {
    #[inline]
    fn from(value: TickTockTxInfo) -> Self {
        Self::TickTock(value)
    }
}

/// Ordinary transaction info.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OrdinaryTxInfo {
    /// Whether the credit phase was executed first
    /// (usually set when incoming message has `bounce: false`).
    pub credit_first: bool,
    /// Storage phase info.
    ///
    /// Skipped if the account did not exist prior to execution.
    pub storage_phase: Option<StoragePhase>,
    /// Credit phase info.
    ///
    /// Skipped if the incoming message is external.
    pub credit_phase: Option<CreditPhase>,
    /// Compute phase info.
    pub compute_phase: ComputePhase,
    /// Action phase info.
    ///
    /// Skipped if the transaction was aborted at the compute phase.
    pub action_phase: Option<ActionPhase>,
    /// Whether the transaction was reverted.
    pub aborted: bool,
    /// Bounce phase info.
    ///
    /// Only present if the incoming message had `bounce: true` and
    /// the compute phase failed.
    pub bounce_phase: Option<BouncePhase>,
    /// Whether the account was destroyed during this transaction.
    pub destroyed: bool,
}

impl Store for OrdinaryTxInfo {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let action_phase = match &self.action_phase {
            Some(action_phase) => {
                let mut builder = CellBuilder::new();
                ok!(action_phase.store_into(&mut builder, context));
                Some(ok!(builder.build_ext(context)))
            }
            None => None,
        };

        ok!(builder.store_bit(self.credit_first));
        ok!(self.storage_phase.store_into(builder, context));
        ok!(self.credit_phase.store_into(builder, context));
        ok!(self.compute_phase.store_into(builder, context));
        ok!(action_phase.store_into(builder, context));
        ok!(builder.store_bit(self.aborted));
        ok!(self.bounce_phase.store_into(builder, context));
        builder.store_bit(self.destroyed)
    }
}

impl<'a> Load<'a> for OrdinaryTxInfo {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        Ok(Self {
            credit_first: ok!(slice.load_bit()),
            storage_phase: ok!(Option::<StoragePhase>::load_from(slice)),
            credit_phase: ok!(Option::<CreditPhase>::load_from(slice)),
            compute_phase: ok!(ComputePhase::load_from(slice)),
            action_phase: match ok!(Option::<Cell>::load_from(slice)) {
                Some(cell) => Some(ok!(cell.as_ref().parse::<ActionPhase>())),
                None => None,
            },
            aborted: ok!(slice.load_bit()),
            bounce_phase: ok!(Option::<BouncePhase>::load_from(slice)),
            destroyed: ok!(slice.load_bit()),
        })
    }
}

/// Tick-tock transaction info.
#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TickTockTxInfo {
    /// Tick-tock transaction execution edge.
    pub kind: TickTock,
    /// Storage phase info.
    pub storage_phase: StoragePhase,
    /// Compute phase info.
    pub compute_phase: ComputePhase,
    /// Action phase info.
    ///
    /// Skipped if the transaction was aborted at the compute phase.
    pub action_phase: Option<ActionPhase>,
    /// Whether the transaction was reverted.
    pub aborted: bool,
    /// Whether the account was destroyed during this transaction.
    pub destroyed: bool,
}

impl Store for TickTockTxInfo {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        let action_phase = match &self.action_phase {
            Some(action_phase) => {
                let mut builder = CellBuilder::new();
                ok!(action_phase.store_into(&mut builder, context));
                Some(ok!(builder.build_ext(context)))
            }
            None => None,
        };

        let flags = ((self.aborted as u8) << 1) | (self.destroyed as u8);

        ok!(self.kind.store_into(builder, context));
        ok!(self.storage_phase.store_into(builder, context));
        ok!(self.compute_phase.store_into(builder, context));
        ok!(action_phase.store_into(builder, context));
        builder.store_small_uint(flags, 2)
    }
}

impl<'a> Load<'a> for TickTockTxInfo {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let kind = ok!(TickTock::load_from(slice));
        let storage_phase = ok!(StoragePhase::load_from(slice));
        let compute_phase = ok!(ComputePhase::load_from(slice));
        let action_phase = match ok!(Option::<Cell>::load_from(slice)) {
            Some(cell) => Some(ok!(cell.as_ref().parse::<ActionPhase>())),
            None => None,
        };
        let flags = ok!(slice.load_small_uint(2));

        Ok(Self {
            kind,
            storage_phase,
            compute_phase,
            action_phase,
            aborted: flags & 0b10 != 0,
            destroyed: flags & 0b01 != 0,
        })
    }
}

/// Tick-tock transaction execution edge.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TickTock {
    /// Start of the block.
    Tick = 0,
    /// End of the block.
    Tock = 1,
}

impl Store for TickTock {
    #[inline]
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        builder.store_bit(*self == Self::Tock)
    }
}

impl<'a> Load<'a> for TickTock {
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_bit() {
            Ok(false) => Ok(Self::Tick),
            Ok(true) => Ok(Self::Tock),
            Err(e) => Err(e),
        }
    }
}

/// Account state hash update.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#72")]
pub struct HashUpdate {
    /// Old account state hash.
    pub old: HashBytes,
    /// New account state hash.
    pub new: HashBytes,
}
