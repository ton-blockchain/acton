use crate::cell::*;
use crate::dict::AugDictExtra;
use crate::error::Error;
use crate::models::{
    CurrencyCollection, ExtInMsgInfo, IntMsgInfo, Message, MsgEnvelope, MsgInfo, OwnedMessage,
    Transaction,
};
use crate::num::Tokens;

/// Inbound message import fees.
#[derive(Default, PartialEq, Eq, Clone, Debug, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ImportFees {
    /// Fees collected from the message.
    pub fees_collected: Tokens,
    /// Value imported from the message.
    pub value_imported: CurrencyCollection,
}

impl AugDictExtra for ImportFees {
    fn comp_add(
        left: &mut CellSlice,
        right: &mut CellSlice,
        b: &mut CellBuilder,
        cx: &dyn CellContext,
    ) -> Result<(), Error> {
        let left = ok!(Self::load_from(left));
        let right = ok!(Self::load_from(right));

        Self {
            fees_collected: ok!(left
                .fees_collected
                .checked_add(right.fees_collected)
                .ok_or(Error::IntOverflow)),
            value_imported: ok!(left.value_imported.checked_add(&right.value_imported)),
        }
        .store_into(b, cx)
    }
}

/// Inbound message.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "ty"))]
pub enum InMsg {
    /// Inbound external message.
    External(InMsgExternal),
    /// Immediately routed internal message.
    Immediate(InMsgFinal),
    /// Internal message with a destination in this block.
    Final(InMsgFinal),
    /// Transit internal message.
    Transit(InMsgTransit),
}

impl InMsg {
    const MSG_IMPORT_EXT: u8 = 0b000;
    const MSG_IMPORT_IMM: u8 = 0b011;
    const MSG_IMPORT_FIN: u8 = 0b100;
    const MSG_IMPORT_TR: u8 = 0b101;

    /// Loads a transaction for the inbound message.
    ///
    /// Transaction exists only in [`External`], [`Immediate`], and [`Final`] inbound messages.
    ///
    /// [`External`]: InMsg::External
    /// [`Immediate`]: InMsg::Immediate
    /// [`Final`]: InMsg::Final
    pub fn load_transaction(&self) -> Result<Option<Transaction>, Error> {
        match self {
            Self::External(msg) => msg.load_transaction().map(Some),
            Self::Immediate(msg) | Self::Final(msg) => msg.load_transaction().map(Some),
            Self::Transit(_) => Ok(None),
        }
    }

    /// Returns a transaction cell for the inbound message.
    ///
    /// Transaction exists only in [`External`], [`Immediate`], and [`Final`] inbound messages.
    ///
    /// [`External`]: InMsg::External
    /// [`Immediate`]: InMsg::Immediate
    /// [`Final`]: InMsg::Final
    pub fn transaction_cell(&self) -> Option<Cell> {
        match self {
            Self::External(msg) => Some(msg.transaction.inner().clone()),
            Self::Immediate(msg) | Self::Final(msg) => Some(msg.transaction.inner().clone()),
            Self::Transit(_) => None,
        }
    }

    /// Loads a non-owned inbound message.
    pub fn load_msg(&self) -> Result<Message<'_>, Error> {
        match self {
            Self::External(msg) => msg.load_in_msg(),
            Self::Immediate(msg) => msg.load_in_msg(),
            Self::Final(msg) => msg.load_in_msg(),
            Self::Transit(msg) => msg.load_in_msg(),
        }
    }

    /// Loads an owned inbound message.
    pub fn load_msg_owned(&self) -> Result<OwnedMessage, Error> {
        match self {
            Self::External(x) => x.load_in_msg_owned(),
            Self::Immediate(x) => x.load_in_msg_owned(),
            Self::Final(x) => x.load_in_msg_owned(),
            Self::Transit(x) => x.load_in_msg_owned(),
        }
    }

    /// Loads an inbound message cell.
    pub fn load_msg_cell(&self) -> Result<Cell, Error> {
        match self {
            Self::External(msg) => Ok(msg.in_msg.inner().clone()),
            Self::Immediate(msg) => msg.load_in_msg_cell(),
            Self::Final(msg) => msg.load_in_msg_cell(),
            Self::Transit(msg) => msg.load_in_msg_cell(),
        }
    }

    /// Returns an envelope cell of the inbound message.
    pub fn in_msg_envelope_cell(&self) -> Option<Cell> {
        match self {
            Self::External(_) => None,
            Self::Immediate(msg) => Some(msg.in_msg_envelope.inner().clone()),
            Self::Final(msg) => Some(msg.in_msg_envelope.inner().clone()),
            Self::Transit(msg) => Some(msg.in_msg_envelope.inner().clone()),
        }
    }

    /// Loads an inbound message envelope.
    pub fn load_in_msg_envelope(&self) -> Result<Option<MsgEnvelope>, Error> {
        match self {
            Self::External(_) => Ok(None),
            Self::Immediate(msg) => msg.load_in_msg_envelope().map(Some),
            Self::Final(msg) => msg.load_in_msg_envelope().map(Some),
            Self::Transit(msg) => msg.load_in_msg_envelope().map(Some),
        }
    }

    /// Returns an outbound envelope cell.
    pub fn out_msg_envelope_cell(&self) -> Option<Cell> {
        match self {
            Self::External(_) => None,
            Self::Immediate(_) => None,
            Self::Final(_) => None,
            Self::Transit(msg) => Some(msg.out_msg_envelope.inner().clone()),
        }
    }

    /// Loads an outbound message envelope.
    pub fn load_out_msg_envelope(&self) -> Result<Option<MsgEnvelope>, Error> {
        match self {
            Self::External(_) => Ok(None),
            Self::Immediate(_) => Ok(None),
            Self::Final(_) => Ok(None),
            Self::Transit(x) => x.load_out_msg_envelope().map(Some),
        }
    }

    /// Computes import fees.
    pub fn compute_fees(&self) -> Result<ImportFees, Error> {
        Ok(match self {
            Self::External(_) => ImportFees::default(),
            Self::Immediate(msg) => {
                let info = ok!(msg.load_in_msg_info());
                ImportFees {
                    fees_collected: info.fwd_fee,
                    value_imported: CurrencyCollection::ZERO,
                }
            }
            Self::Final(msg) => {
                let env = ok!(msg.load_in_msg_envelope());
                if msg.fwd_fee != env.fwd_fee_remaining {
                    return Err(Error::InvalidData);
                }

                let info = ok!(env.load_message_info());

                let mut value_imported = info.value;
                ok!(value_imported.try_add_assign_tokens(env.fwd_fee_remaining));

                ImportFees {
                    fees_collected: env.fwd_fee_remaining,
                    value_imported,
                }
            }
            Self::Transit(msg) => {
                let env = ok!(msg.load_in_msg_envelope());
                if env.fwd_fee_remaining < msg.transit_fee {
                    return Err(Error::InvalidData);
                }

                let info = ok!(env.load_message_info());

                let mut value_imported = info.value;
                ok!(value_imported.try_add_assign_tokens(env.fwd_fee_remaining));

                ImportFees {
                    fees_collected: msg.transit_fee,
                    value_imported,
                }
            }
        })
    }
}

impl Store for InMsg {
    fn store_into(&self, builder: &mut CellBuilder, cx: &dyn CellContext) -> Result<(), Error> {
        match self {
            Self::External(msg) => {
                ok!(builder.store_small_uint(Self::MSG_IMPORT_EXT, 3));
                msg.store_into(builder, cx)
            }
            Self::Immediate(msg) => {
                ok!(builder.store_small_uint(Self::MSG_IMPORT_IMM, 3));
                msg.store_into(builder, cx)
            }
            Self::Final(msg) => {
                ok!(builder.store_small_uint(Self::MSG_IMPORT_FIN, 3));
                msg.store_into(builder, cx)
            }
            Self::Transit(msg) => {
                ok!(builder.store_small_uint(Self::MSG_IMPORT_TR, 3));
                msg.store_into(builder, cx)
            }
        }
    }
}

impl<'a> Load<'a> for InMsg {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match ok!(slice.load_small_uint(3)) {
            Self::MSG_IMPORT_EXT => InMsgExternal::load_from(slice).map(Self::External),
            Self::MSG_IMPORT_IMM => InMsgFinal::load_from(slice).map(Self::Immediate),
            Self::MSG_IMPORT_FIN => InMsgFinal::load_from(slice).map(Self::Final),
            Self::MSG_IMPORT_TR => InMsgTransit::load_from(slice).map(Self::Transit),
            _ => Err(Error::InvalidTag),
        }
    }
}

/// Inbound external message.
#[derive(Clone, Debug, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "ty"))]
pub struct InMsgExternal {
    /// External message itself.
    #[cfg_attr(feature = "serde", serde(serialize_with = "Lazy::serialize_repr_hash"))]
    pub in_msg: Lazy<OwnedMessage>,
    /// Executed transaction for this external message.
    #[cfg_attr(feature = "serde", serde(serialize_with = "Lazy::serialize_repr_hash"))]
    pub transaction: Lazy<Transaction>,
}

impl InMsgExternal {
    /// Loads only message info.
    pub fn load_in_msg_info(&self) -> Result<ExtInMsgInfo, Error> {
        if let MsgInfo::ExtIn(info) = ok!(<_>::load_from(&mut ok!(self.in_msg.as_slice()))) {
            Ok(info)
        } else {
            Err(Error::InvalidData)
        }
    }

    /// Loads a non-owned message.
    pub fn load_in_msg(&self) -> Result<Message<'_>, Error> {
        self.in_msg.cast_ref::<Message<'_>>().load()
    }

    /// Loads an owned message.
    pub fn load_in_msg_owned(&self) -> Result<OwnedMessage, Error> {
        self.in_msg.load()
    }

    /// Loads transaction.
    pub fn load_transaction(&self) -> Result<Transaction, Error> {
        self.transaction.load()
    }
}

/// Executed inbound internal message.
#[derive(Clone, Debug, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct InMsgFinal {
    /// Old envelope.
    pub in_msg_envelope: Lazy<MsgEnvelope>,
    /// Transaction
    #[cfg_attr(feature = "serde", serde(serialize_with = "Lazy::serialize_repr_hash"))]
    pub transaction: Lazy<Transaction>,
    /// Forward fee.
    pub fwd_fee: Tokens,
}

impl InMsgFinal {
    /// Load an inbound message envelope.
    pub fn load_in_msg_envelope(&self) -> Result<MsgEnvelope, Error> {
        self.in_msg_envelope.load()
    }

    /// Load a non-owned inbound message.
    pub fn load_in_msg(&self) -> Result<Message<'_>, Error> {
        let mut envelope = ok!(self.in_msg_envelope.as_slice());
        Message::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Load an inbound message envelope.
    pub fn load_in_msg_info(&self) -> Result<IntMsgInfo, Error> {
        let mut envelope = ok!(self.in_msg_envelope.as_slice());
        let mut message = ok!(envelope.load_reference_as_slice());
        if let MsgInfo::Int(info) = ok!(<_>::load_from(&mut message)) {
            Ok(info)
        } else {
            Err(Error::InvalidData)
        }
    }

    /// Load an owned inbound message.
    pub fn load_in_msg_owned(&self) -> Result<OwnedMessage, Error> {
        let mut envelope = ok!(self.in_msg_envelope.as_slice());
        OwnedMessage::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Loads an inbound message cell.
    pub fn load_in_msg_cell(&self) -> Result<Cell, Error> {
        ok!(self.in_msg_envelope.as_slice()).load_reference_cloned()
    }

    /// Returns a hash of the envelope with the inbound message.
    pub fn in_msg_envelope_hash(&self) -> &HashBytes {
        self.in_msg_envelope.repr_hash()
    }

    /// Loads transaction.
    pub fn load_transaction(&self) -> Result<Transaction, Error> {
        self.transaction.load()
    }
}

/// Internal message that was not processed in this block.
#[derive(Clone, Debug, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct InMsgTransit {
    /// Old envelope.
    pub in_msg_envelope: Lazy<MsgEnvelope>,
    /// New envelope.
    pub out_msg_envelope: Lazy<MsgEnvelope>,
    /// Transit fee.
    pub transit_fee: Tokens,
}

impl InMsgTransit {
    /// Load a non-owned inbound message.
    pub fn load_in_msg(&self) -> Result<Message<'_>, Error> {
        let mut envelope = ok!(self.in_msg_envelope.as_slice());
        Message::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Load an owned inbound message.
    pub fn load_in_msg_owned(&self) -> Result<OwnedMessage, Error> {
        let mut envelope = ok!(self.in_msg_envelope.as_slice());
        OwnedMessage::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Load a non-owned outbound message.
    pub fn load_out_msg(&self) -> Result<Message<'_>, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        Message::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Load an owned outbound message.
    pub fn load_out_msg_owned(&self) -> Result<OwnedMessage, Error> {
        let mut envelope = ok!(self.out_msg_envelope.as_slice());
        OwnedMessage::load_from(&mut ok!(envelope.load_reference_as_slice()))
    }

    /// Load inbound message cell.
    pub fn load_in_msg_cell(&self) -> Result<Cell, Error> {
        ok!(self.in_msg_envelope.as_slice()).load_reference_cloned()
    }

    /// Load outbound message cell.
    pub fn load_out_msg_cell(&self) -> Result<Cell, Error> {
        ok!(self.out_msg_envelope.as_slice()).load_reference_cloned()
    }

    /// Load inbound message envelope.
    pub fn load_in_msg_envelope(&self) -> Result<MsgEnvelope, Error> {
        self.in_msg_envelope.load()
    }

    /// Load outbound message envelope.
    pub fn load_out_msg_envelope(&self) -> Result<MsgEnvelope, Error> {
        self.out_msg_envelope.load()
    }

    /// Returns a hash of the envelope with the inbound message.
    pub fn in_msg_envelope_hash(&self) -> &HashBytes {
        self.in_msg_envelope.repr_hash()
    }

    /// Returns a hash of the envelope with the outbound message.
    pub fn out_msg_envelope_hash(&self) -> &HashBytes {
        self.out_msg_envelope.repr_hash()
    }
}
