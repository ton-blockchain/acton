use super::IntMsgInfo;
use crate::cell::*;
use crate::error::Error;
use crate::models::{Message, MsgInfo, OwnedMessage};
use crate::num::Tokens;
use crate::util::unlikely;

/// Next-hop address for a message.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "ty"))]
pub enum IntermediateAddr {
    /// Destination prefix length within the same workchain.
    Regular(IntermediateAddrRegular),
    /// Address prefix with a basic workchain id.
    Simple(IntermediateAddrSimple),
    /// Address prefix with an extended workchain id.
    Ext(IntermediateAddrExt),
}

impl IntermediateAddr {
    /// Full destination address within the same workchain.
    pub const FULL_DEST_SAME_WORKCHAIN: Self = Self::Regular(IntermediateAddrRegular {
        use_dest_bits: IntermediateAddrRegular::FULL_BITS,
    });

    /// Full source address within the same workchain.
    pub const FULL_SRC_SAME_WORKCHAIN: Self =
        Self::Regular(IntermediateAddrRegular { use_dest_bits: 0 });

    /// Full destination address within masterchain.
    pub const FULL_MASTERCHAIN: Self = Self::Simple(IntermediateAddrSimple {
        workchain: -1,
        address_prefix: 0b1 << 63,
    });

    /// Returns target workchain id if specified.
    /// Returns `None` if the same workchain is used.
    pub fn workchain(&self) -> Option<i32> {
        match self {
            IntermediateAddr::Regular(_) => None,
            IntermediateAddr::Simple(simple) => Some(simple.workchain as i32),
            IntermediateAddr::Ext(ext) => Some(ext.workchain),
        }
    }

    /// Returns the address prefix if specified.
    /// Returns `None` if bits length is used.
    pub fn address_prefix(&self) -> Option<u64> {
        match self {
            IntermediateAddr::Regular(_) => None,
            IntermediateAddr::Simple(simple) => Some(simple.address_prefix),
            IntermediateAddr::Ext(ext) => Some(ext.address_prefix),
        }
    }
}

impl From<IntermediateAddrRegular> for IntermediateAddr {
    #[inline]
    fn from(addr: IntermediateAddrRegular) -> Self {
        IntermediateAddr::Regular(addr)
    }
}

impl From<IntermediateAddrSimple> for IntermediateAddr {
    #[inline]
    fn from(addr: IntermediateAddrSimple) -> Self {
        IntermediateAddr::Simple(addr)
    }
}

impl From<IntermediateAddrExt> for IntermediateAddr {
    #[inline]
    fn from(addr: IntermediateAddrExt) -> Self {
        IntermediateAddr::Ext(addr)
    }
}

impl Store for IntermediateAddr {
    fn store_into(&self, builder: &mut CellBuilder, cx: &dyn CellContext) -> Result<(), Error> {
        match self {
            IntermediateAddr::Regular(addr) => {
                ok!(builder.store_bit_zero()); // tag = $0
                addr.store_into(builder, cx)
            }
            IntermediateAddr::Simple(addr) => {
                ok!(builder.store_small_uint(0b10, 2)); // tag = $10
                addr.store_into(builder, cx)
            }
            IntermediateAddr::Ext(addr) => {
                ok!(builder.store_small_uint(0b11, 2)); // tag = $11
                addr.store_into(builder, cx)
            }
        }
    }
}

impl<'a> Load<'a> for IntermediateAddr {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        if unlikely(slice.load_bit()?) {
            if unlikely(slice.load_bit()?) {
                // tag = $11
                IntermediateAddrExt::load_from(slice).map(IntermediateAddr::Ext)
            } else {
                // tag = $10
                IntermediateAddrSimple::load_from(slice).map(IntermediateAddr::Simple)
            }
        } else {
            // tag = $0
            IntermediateAddrRegular::load_from(slice).map(IntermediateAddr::Regular)
        }
    }
}

/// Destination prefix length within the same workchain.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[tlb(validate_with = "Self::is_valid")]
pub struct IntermediateAddrRegular {
    /// Destination address prefix length in bits.
    use_dest_bits: u8,
}

impl IntermediateAddrRegular {
    /// Full address prefix length in bits.
    pub const FULL_BITS: u8 = 96;

    /// Returns whether the address prefix length is valid.
    pub const fn is_valid(&self) -> bool {
        self.use_dest_bits <= Self::FULL_BITS
    }

    /// Create for the destination address.
    pub fn with_dest_bits(use_dest_bits: u8) -> Option<Self> {
        (use_dest_bits <= Self::FULL_BITS).then_some(IntermediateAddrRegular { use_dest_bits })
    }

    /// Create for the source address.
    pub fn with_src_bits(use_src_bits: u8) -> Option<Self> {
        (use_src_bits <= Self::FULL_BITS).then(|| IntermediateAddrRegular {
            use_dest_bits: Self::FULL_BITS - use_src_bits,
        })
    }

    /// Returns the destination address prefix length in bits.
    pub fn use_dest_bits(&self) -> u8 {
        self.use_dest_bits
    }

    /// Returns the source address prefix length in bits.
    pub fn use_src_bits(&self) -> u8 {
        Self::FULL_BITS - self.use_dest_bits
    }
}

/// Address prefix with a basic workchain id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Load, Store)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct IntermediateAddrSimple {
    /// Basic workchain id.
    ///
    /// See [`WorkchainFormatBasic`].
    ///
    /// [`WorkchainFormatBasic`]: crate::models::WorkchainFormatBasic
    pub workchain: i8,

    /// High 64 bits of the address.
    #[cfg_attr(feature = "serde", serde(serialize_with = "serialize_account_prefix"))]
    pub address_prefix: u64,
}

/// Address prefix with an extended workchain id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct IntermediateAddrExt {
    /// Workchain ID
    pub workchain: i32,

    /// High 64 bits of the address.
    #[cfg_attr(feature = "serde", serde(serialize_with = "serialize_account_prefix"))]
    pub address_prefix: u64,
}

/// Message with routing information.
#[derive(Clone, Debug, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[tlb(tag = "#4")]
pub struct MsgEnvelope {
    /// Current address.
    pub cur_addr: IntermediateAddr,
    /// Next-hop address.
    pub next_addr: IntermediateAddr,
    /// Remaining transit fee.
    pub fwd_fee_remaining: Tokens,
    /// The message itself.
    #[cfg_attr(feature = "serde", serde(serialize_with = "Lazy::serialize_repr_hash"))]
    pub message: Lazy<OwnedMessage>,
}

impl MsgEnvelope {
    /// Load only message info.
    pub fn load_message_info(&self) -> Result<IntMsgInfo, Error> {
        if let MsgInfo::Int(info) = ok!(<_>::load_from(&mut ok!(self.message.as_slice()))) {
            Ok(info)
        } else {
            Err(Error::InvalidData)
        }
    }

    /// Load a non-owned message.
    pub fn load_message(&self) -> Result<Message<'_>, Error> {
        self.message.cast_ref::<Message<'_>>().load()
    }

    /// Load an owned message.
    pub fn load_message_owned(&self) -> Result<OwnedMessage, Error> {
        self.message.load()
    }

    /// Returns a hash of the message.
    pub fn message_hash(&self) -> &HashBytes {
        self.message.repr_hash()
    }

    /// Tries to subtract transfer fee from envelope.
    pub fn collect_fee(&mut self, fee: Tokens) -> bool {
        match self.fwd_fee_remaining.checked_sub(fee) {
            Some(remaining) => {
                self.fwd_fee_remaining = remaining;
                true
            }
            None => false,
        }
    }
}

#[cfg(feature = "serde")]
fn serialize_account_prefix<S>(prefix: &u64, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&format!("{prefix:08x}"))
}
