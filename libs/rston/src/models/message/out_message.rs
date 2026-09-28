use crate::cell::*;
use crate::error::Error;
use crate::models::{
    CurrencyCollection, ExtOutMsgInfo, InMsg, IntMsgInfo, Message, MsgEnvelope, MsgInfo,
    OwnedMessage, Transaction,
};

/// Outbound message queue entry.
#[derive(Debug, Clone, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct EnqueuedMsg {
    /// Enqueued message lt.
    pub enqueued_lt: u64,
    /// Outbound message envelope.
    pub out_msg_envelope: Lazy<MsgEnvelope>,
}

impl EnqueuedMsg {
    /// Loads a logical time when the message was created.
    pub fn load_created_lt(&self) -> Result<u64, Error> {
        match ok!(self.load_out_msg_info()) {
            MsgInfo::Int(info) => Ok(info.created_lt),
            MsgInfo::ExtOut(info) => Ok(info.created_lt),
            _ => Err(Error::InvalidData),
        }
    }

    /// Loads a message info.
    pub fn load_out_msg_info(&self) -> Result<MsgInfo, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        MsgInfo::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Loads a non-owned message.
    pub fn load_out_msg(&self) -> Result<Message<'_>, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        Message::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Loads an owned message.
    pub fn load_out_msg_owned(&self) -> Result<OwnedMessage, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        OwnedMessage::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }
}

/// Outbound message.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "ty"))]
pub enum OutMsg {
    /// External outbound message.
    External(OutMsgExternal),
    /// Ordinary (internal) outbound message, generated in this block and
    /// included into the outbound queue.
    New(OutMsgNew),
    /// Immediately processed internal outbound message.
    Immediate(OutMsgImmediate),
    /// A message that was dequeued from the outbound queue
    /// and immediately queued in the same block.
    DequeueImmediate(OutMsgDequeueImmediate),
    /// A message that was dequeued from the outbound queue.
    DequeueShort(OutMsgDequeueShort),
}

impl OutMsg {
    const OUT_MSG_EXT: u8 = 0b000;
    const OUT_MSG_NEW: u8 = 0b001;
    const OUT_MSG_IMM: u8 = 0b010;
    const OUT_MSG_DEQ_IMM: u8 = 0b100;

    // NOTE: Make sure to update `Store` and `Load` impl if the tag is changed.
    const OUT_MSG_DEQ_SHORT: u8 = 0b1101;

    /// Loads an envelope of the outbound message.
    pub fn load_out_msg_envelope(&self) -> Result<Option<MsgEnvelope>, Error> {
        match self {
            Self::External(_) => Ok(None),
            Self::New(msg) => msg.load_out_msg_envelope().map(Some),
            Self::Immediate(msg) => msg.load_out_msg_envelope().map(Some),
            Self::DequeueShort(_) => Ok(None),
            Self::DequeueImmediate(msg) => msg.load_out_msg_envelope().map(Some),
        }
    }

    /// Loads an envelope of the outbound message.
    pub fn out_msg_envelope_cell(&self) -> Option<Cell> {
        match self {
            Self::External(_) => None,
            Self::New(msg) => Some(msg.out_msg_envelope.inner().clone()),
            Self::Immediate(msg) => Some(msg.out_msg_envelope.inner().clone()),
            Self::DequeueShort(_) => None,
            Self::DequeueImmediate(msg) => Some(msg.out_msg_envelope.inner().clone()),
        }
    }

    /// Loads a non-owned outbound message.
    pub fn load_message(&self) -> Result<Option<Message<'_>>, Error> {
        match self {
            Self::External(msg) => msg.load_out_msg().map(Some),
            Self::New(msg) => msg.load_out_msg().map(Some),
            Self::Immediate(msg) => msg.load_out_msg().map(Some),
            Self::DequeueShort(_) => Ok(None),
            Self::DequeueImmediate(msg) => msg.load_out_msg().map(Some),
        }
    }

    /// Loads an owned outbound message.
    pub fn load_message_owned(&self) -> Result<Option<OwnedMessage>, Error> {
        match self {
            Self::External(msg) => msg.load_out_msg_owned().map(Some),
            Self::New(msg) => msg.load_out_msg_owned().map(Some),
            Self::Immediate(msg) => msg.load_out_msg_owned().map(Some),
            Self::DequeueShort(_) => Ok(None),
            Self::DequeueImmediate(msg) => msg.load_out_msg_owned().map(Some),
        }
    }

    /// Returns a source transaction of the outbound message.
    pub fn transaction_cell(&self) -> Option<Cell> {
        match self {
            Self::External(msg) => Some(msg.transaction.inner().clone()),
            Self::New(msg) => Some(msg.transaction.inner().clone()),
            Self::Immediate(msg) => Some(msg.transaction.inner().clone()),
            Self::DequeueShort(_) => None,
            Self::DequeueImmediate(_) => None,
        }
    }

    /// Loads a source transaction of the outbound message.
    pub fn load_transaction(&self) -> Result<Option<Transaction>, Error> {
        match self {
            Self::External(msg) => msg.load_transaction().map(Some),
            Self::New(msg) => msg.load_transaction().map(Some),
            Self::Immediate(msg) => msg.load_transaction().map(Some),
            Self::DequeueShort(_) => Ok(None),
            Self::DequeueImmediate(_) => Ok(None),
        }
    }

    /// Loads a reimport message.
    pub fn load_reimport_msg(&self) -> Result<Option<InMsg>, Error> {
        match self {
            Self::Immediate(msg) => msg.load_reimport_msg().map(Some),
            Self::DequeueImmediate(msg) => msg.load_reimport_msg().map(Some),
            _ => Ok(None),
        }
    }

    /// Returns a reimport message cell.
    pub fn reimport_msg_cell(&self) -> Option<Cell> {
        match self {
            Self::Immediate(msg) => Some(msg.reimport.inner().clone()),
            Self::DequeueImmediate(msg) => Some(msg.reimport.inner().clone()),
            _ => None,
        }
    }

    /// Compute exported value.
    pub fn compute_exported_value(&self) -> Result<CurrencyCollection, Error> {
        Ok(if let Self::New(msg) = self {
            let envelope = ok!(msg.load_out_msg_envelope());
            let info = ok!(envelope.load_message_info());

            let mut result = info.value;
            ok!(result.try_add_assign_tokens(envelope.fwd_fee_remaining));
            result
        } else {
            CurrencyCollection::ZERO
        })
    }
}

impl Store for OutMsg {
    fn store_into(&self, builder: &mut CellBuilder, cx: &dyn CellContext) -> Result<(), Error> {
        match self {
            OutMsg::External(msg) => {
                ok!(builder.store_small_uint(Self::OUT_MSG_EXT, 3));
                msg.store_into(builder, cx)
            }
            OutMsg::Immediate(msg) => {
                ok!(builder.store_small_uint(Self::OUT_MSG_IMM, 3));
                msg.store_into(builder, cx)
            }
            OutMsg::New(msg) => {
                ok!(builder.store_small_uint(Self::OUT_MSG_NEW, 3));
                msg.store_into(builder, cx)
            }
            OutMsg::DequeueShort(msg) => {
                ok!(builder.store_small_uint(Self::OUT_MSG_DEQ_SHORT, 4));
                msg.store_into(builder, cx)
            }
            OutMsg::DequeueImmediate(msg) => {
                ok!(builder.store_small_uint(Self::OUT_MSG_DEQ_IMM, 3));
                msg.store_into(builder, cx)
            }
        }
    }
}

impl<'a> Load<'a> for OutMsg {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match ok!(slice.load_small_uint(3)) {
            Self::OUT_MSG_EXT => OutMsgExternal::load_from(slice).map(Self::External),
            Self::OUT_MSG_NEW => OutMsgNew::load_from(slice).map(Self::New),
            Self::OUT_MSG_IMM => OutMsgImmediate::load_from(slice).map(Self::Immediate),
            Self::OUT_MSG_DEQ_IMM => {
                OutMsgDequeueImmediate::load_from(slice).map(Self::DequeueImmediate)
            }
            0b110 if ok!(slice.load_bit()) => {
                OutMsgDequeueShort::load_from(slice).map(Self::DequeueShort)
            }
            _ => Err(Error::InvalidTag),
        }
    }
}

/// External outbound message.
#[derive(Clone, Debug, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct OutMsgExternal {
    /// External message itself.
    #[cfg_attr(feature = "serde", serde(serialize_with = "Lazy::serialize_repr_hash"))]
    pub out_msg: Lazy<OwnedMessage>,

    /// The source transaction of this external message.
    #[cfg_attr(feature = "serde", serde(serialize_with = "Lazy::serialize_repr_hash"))]
    pub transaction: Lazy<Transaction>,
}

impl OutMsgExternal {
    /// Loads only message info.
    pub fn load_out_msg_info(&self) -> Result<ExtOutMsgInfo, Error> {
        if let MsgInfo::ExtOut(info) = ok!(<_>::load_from(&mut ok!(self.out_msg.as_slice()))) {
            Ok(info)
        } else {
            Err(Error::InvalidData)
        }
    }

    /// Loads a non-owned message.
    pub fn load_out_msg(&self) -> Result<Message<'_>, Error> {
        self.out_msg.cast_ref::<Message<'_>>().load()
    }

    /// Loads an owned message.
    pub fn load_out_msg_owned(&self) -> Result<OwnedMessage, Error> {
        self.out_msg.load()
    }

    /// Loads transaction.
    pub fn load_transaction(&self) -> Result<Transaction, Error> {
        self.transaction.load()
    }
}

/// Immediately processed internal outbound message.
#[derive(Clone, Debug, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct OutMsgImmediate {
    /// Outbound message envelope.
    pub out_msg_envelope: Lazy<MsgEnvelope>,
    /// The source transaction of this message.
    #[cfg_attr(feature = "serde", serde(serialize_with = "Lazy::serialize_repr_hash"))]
    pub transaction: Lazy<Transaction>,
    /// The destination reimport message.
    pub reimport: Lazy<InMsg>,
}

impl OutMsgImmediate {
    /// Loads an envelope for the message.
    pub fn load_out_msg_envelope(&self) -> Result<MsgEnvelope, Error> {
        self.out_msg_envelope.load()
    }

    /// Loads only message info.
    pub fn load_out_msg_info(&self) -> Result<IntMsgInfo, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        let mut message = ok!(envelope.load_reference_as_slice());
        if let MsgInfo::Int(info) = ok!(<_>::load_from(&mut message)) {
            Ok(info)
        } else {
            Err(Error::InvalidData)
        }
    }

    /// Loads a non-owned message.
    pub fn load_out_msg(&self) -> Result<Message<'_>, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        Message::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Loads an owned message.
    pub fn load_out_msg_owned(&self) -> Result<OwnedMessage, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        OwnedMessage::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Loads transaction.
    pub fn load_transaction(&self) -> Result<Transaction, Error> {
        self.transaction.load()
    }

    /// Loads a reimport message.
    pub fn load_reimport_msg(&self) -> Result<InMsg, Error> {
        self.reimport.load()
    }
}

/// Ordinary (internal) outbound message, generated in this block and
/// included into the outbound queue.
#[derive(Clone, Debug, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct OutMsgNew {
    /// Outbound message envelope.
    pub out_msg_envelope: Lazy<MsgEnvelope>,
    /// The source transaction of this message.
    #[cfg_attr(feature = "serde", serde(serialize_with = "Lazy::serialize_repr_hash"))]
    pub transaction: Lazy<Transaction>,
}

impl OutMsgNew {
    /// Loads an envelope for the message.
    pub fn load_out_msg_envelope(&self) -> Result<MsgEnvelope, Error> {
        self.out_msg_envelope.load()
    }

    /// Loads only message info.
    pub fn load_out_msg_info(&self) -> Result<IntMsgInfo, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        let mut message = ok!(envelope.load_reference_as_slice());
        if let MsgInfo::Int(info) = ok!(<_>::load_from(&mut message)) {
            Ok(info)
        } else {
            Err(Error::InvalidData)
        }
    }

    /// Loads a non-owned message.
    pub fn load_out_msg(&self) -> Result<Message<'_>, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        Message::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Loads an owned message.
    pub fn load_out_msg_owned(&self) -> Result<OwnedMessage, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        OwnedMessage::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Loads transaction.
    pub fn load_transaction(&self) -> Result<Transaction, Error> {
        self.transaction.load()
    }
}

/// A message that was dequeued from the outbound queue
/// and immediately queued in the same block.
#[derive(Clone, Debug, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct OutMsgDequeueImmediate {
    /// Outbound message envelope.
    pub out_msg_envelope: Lazy<MsgEnvelope>,
    /// The destination reimport message.
    pub reimport: Lazy<InMsg>,
}

impl OutMsgDequeueImmediate {
    /// Loads an envelope for the message.
    pub fn load_out_msg_envelope(&self) -> Result<MsgEnvelope, Error> {
        self.out_msg_envelope.load()
    }

    /// Loads only message info.
    pub fn load_out_msg_info(&self) -> Result<IntMsgInfo, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        let mut message = ok!(envelope.load_reference_as_slice());
        if let MsgInfo::Int(info) = ok!(<_>::load_from(&mut message)) {
            Ok(info)
        } else {
            Err(Error::InvalidData)
        }
    }

    /// Loads a non-owned message.
    pub fn load_out_msg(&self) -> Result<Message<'_>, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        Message::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Loads an owned message.
    pub fn load_out_msg_owned(&self) -> Result<OwnedMessage, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        OwnedMessage::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Loads a reimport message.
    pub fn load_reimport_msg(&self) -> Result<InMsg, Error> {
        self.reimport.load()
    }
}

/// A message that was dequeued from the outbound queue.
#[derive(Clone, Debug, Default, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct OutMsgDequeueShort {
    /// Message envelope hash.
    pub msg_env_hash: HashBytes,
    /// Next workchain.
    pub next_workchain: i32,
    /// Next address prefix.
    pub next_addr_pfx: u64,
    /// Import block logical time.
    pub import_block_lt: u64,
}
