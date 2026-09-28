//! Global version and capabilities.

use crate::cell::{Load, Store};
use crate::error::ParseGlobalCapabilityError;

macro_rules! decl_global_capability {
    ($(#[doc = $doc:expr])* $vis:vis enum $ident:ident {$(
        $(#[doc = $var_doc:expr])*
        $field:ident = $descr:literal
    ),*$(,)?}) => {
        $(#[doc = $doc])*
        #[derive(Debug, Copy, Clone, Eq, PartialEq)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[repr(u64)]
        $vis enum $ident {$(
            $(#[doc = $var_doc])*
            $field = 1u64 << $descr
        ),*,}

        impl GlobalCapability {
            const fn from_bit_offset(bit_offset: u32) -> Option<Self> {
                Some(match bit_offset {
                    $($descr => Self::$field),*,
                    _ => return None,
                })
            }
        }

        impl std::fmt::Display for GlobalCapability {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(match self {
                    $(Self::$field => stringify!($field),)*
                })
            }
        }

        impl std::str::FromStr for GlobalCapability {
            type Err = ParseGlobalCapabilityError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(match s {
                    $(stringify!($field) => Self::$field,)*
                    _ => return Err(ParseGlobalCapabilityError::UnknownCapability),
                })
            }
        }
    };
}

decl_global_capability! {
    /// Node software capabilities.
    pub enum GlobalCapability {
        /// Instant Hypercube Routing.
        ///
        /// Mask: `0x0000001`.
        CapIhrEnabled = 0,

        /// Tracking of block collation stats.
        ///
        /// Mask: `0x0000002`.
        CapCreateStatsEnabled = 1,

        /// Body (at most 256 bits) in bounced messages.
        ///
        /// Mask: `0x0000004`.
        CapBounceMsgBody = 2,

        /// Supported software version and capabilities as field in [`BlockInfo`].
        ///
        /// Mask: `0x0000008`.
        ///
        /// [`BlockInfo`]: crate::models::block::BlockInfo
        CapReportVersion = 3,

        /// Special transactions on split or merge.
        ///
        /// Mask: `0x0000010`.
        CapSplitMergeTransactions = 4,

        /// Short output messages queue.
        ///
        /// Mask: `0x0000020`.
        CapShortDequeue = 5,

        /// _unknown_ (possibly just a stub).
        ///
        /// Mask: `0x0000040`.
        CapMbppEnabled = 6,

        /// Precompute storage stats for cells and use this info for storage phase.
        /// NOTE: changes behavior for storage phase, computing stats for non-unique cells.
        ///
        /// Mask: `0x0000080`
        CapFastStorageStat = 7,

        /// Store init code hash in account state.
        ///
        /// Mask: `0x0000100`.
        CapInitCodeHash = 8,

        /// Disable hypercube for message routing.
        ///
        /// Mask: `0x0000200`.
        CapOffHypercube = 9,

        /// `MYCODE` TVM opcode.
        ///
        /// Mask: `0x0000400`.
        CapMyCode = 10,

        /// `CHANGELIB` and `SETLIBCODE` TVM opcodes.
        ///
        /// Mask: `0x0000800`.
        CapSetLibCode = 11,

        /// Fix in `SETINDEX*` TVM opcodes.
        ///
        /// Mask: `0x0001000`.
        CapFixTupleIndexBug = 12,

        /// Reliable External Messaging Protocol.
        ///
        /// Mask: `0x0002000`.
        CapRemp = 13,

        /// Support for decentralized elections.
        ///
        /// Mask: `0x0004000`.
        CapDelections = 14,

        // Capability 15 is reserved (?)
        /// Full message body in bounced messages (in the first child cell).
        ///
        /// Mask: `0x0010000`.
        CapFullBodyInBounced = 16,

        /// `STORAGEFEE` TVM opcode.
        ///
        /// Mask: `0x0020000`.
        CapStorageFeeToTvm = 17,

        /// Support for copyleft messages.
        ///
        /// Mask: `0x0040000`.
        CapCopyleft = 18,

        /// `FIND_BY_*` TVM opcodes.
        ///
        /// Mask: `0x0080000`.
        CapIndexAccounts = 19,

        /// `DIFF*`, `ZIP`, `UNZIP` TVM opcodes.
        ///
        /// Mask: `0x0100000`.
        CapDiff = 20,

        /// Cumulative patches to TVM and cells (popsave, exception handler, loops).
        ///
        /// Mask: `0x0200000`.
        CapsTvmBugfixes2022 = 21,

        /// Support for message queues between workchains.
        ///
        /// Mask: `0x0400000`.
        CapWorkchains = 22,

        /// New continuation serialization format.
        ///
        /// Mask: `0x0800000`.
        CapStcontNewFormat = 23,

        /// Use fast stats for `*DATASIZE*` TVM opcodes.
        ///
        /// Mask: `0x1000000`.
        CapFastStorageStatBugfix = 24,

        /// Add support for transparent loading of merkle cells.
        ///
        /// Mask: `0x2000000`.
        CapResolveMerkleCell = 25,

        /// Prepend signature with `global_id` for TVM.
        ///
        /// Mask: `0x4000000`.
        CapSignatureWithId = 26,

        /// Execute bounce phase even after failed action phase.
        ///
        /// Mask: `0x8000000`.
        CapBounceAfterFailedAction = 27,

        /// Groth16 support in TVM.
        ///
        /// Mask: `0x10000000`
        CapGroth16 = 28,

        /// Makes all fees in config in gas units.
        ///
        /// Mask: `0x20000000`
        CapFeeInGasUnits = 29,

        /// Big cells support.
        ///
        /// Mask: `0x40000000`
        CapBigCells = 30,

        /// Suspend addresses using a config param.
        ///
        /// Mask: `0x80000000`
        CapSuspendedList = 31,

        /// Adds intershard communication between master blocks.
        ///
        /// Mask: `0x100000000`
        CapFastFinality = 32,

        /// Allows to suspend accounts by special extra currencies.
        ///
        /// Mask: `0x200000000`
        CapSuspendByMarks = 33,

        /// Omits master block history to save storage space.
        ///
        /// Mask: `0x400000000`
        CapOmitMasterBlockHistory = 34,

        /// Apply signature domain to verified data.
        ///
        /// It is a newer version of [`GlobalCapability::CapSignatureWithId`]
        /// and has a higher priority if both are enabled at the same time.
        ///
        /// Mask: `0x800000000`
        CapSignatureDomain = 35,
    }
}

impl std::ops::BitOr<GlobalCapability> for GlobalCapability {
    type Output = GlobalCapabilities;

    #[inline]
    fn bitor(self, rhs: GlobalCapability) -> Self::Output {
        GlobalCapabilities(self as u64 | rhs as u64)
    }
}

impl std::ops::BitOr<GlobalCapability> for u64 {
    type Output = GlobalCapabilities;

    #[inline]
    fn bitor(self, rhs: GlobalCapability) -> Self::Output {
        GlobalCapabilities(self | rhs as u64)
    }
}

impl std::ops::BitOrAssign<GlobalCapability> for u64 {
    #[inline]
    fn bitor_assign(&mut self, rhs: GlobalCapability) {
        *self = (*self | rhs).0;
    }
}

impl std::ops::BitOr<u64> for GlobalCapability {
    type Output = GlobalCapabilities;

    #[inline]
    fn bitor(self, rhs: u64) -> Self::Output {
        GlobalCapabilities(self as u64 | rhs)
    }
}

impl std::ops::BitOr<GlobalCapability> for GlobalCapabilities {
    type Output = GlobalCapabilities;

    #[inline]
    fn bitor(self, rhs: GlobalCapability) -> Self::Output {
        GlobalCapabilities(self.0 | rhs as u64)
    }
}

impl std::ops::BitOr<GlobalCapabilities> for GlobalCapability {
    type Output = GlobalCapabilities;

    #[inline]
    fn bitor(self, rhs: GlobalCapabilities) -> Self::Output {
        GlobalCapabilities(self as u64 | rhs.0)
    }
}

impl std::ops::BitOrAssign<u64> for GlobalCapabilities {
    #[inline]
    fn bitor_assign(&mut self, rhs: u64) {
        *self = GlobalCapabilities(self.0 | rhs);
    }
}

impl std::ops::BitOrAssign<GlobalCapability> for GlobalCapabilities {
    #[inline]
    fn bitor_assign(&mut self, rhs: GlobalCapability) {
        *self = *self | rhs;
    }
}

/// Software info.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Store, Load)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[tlb(tag = "#c4")]
pub struct GlobalVersion {
    /// Software version.
    pub version: u32,
    /// Software capability flags.
    pub capabilities: GlobalCapabilities,
}

/// A set of enabled capabilities.
///
/// See [`GlobalCapability`].
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Store, Load)]
#[repr(transparent)]
pub struct GlobalCapabilities(u64);

impl GlobalCapabilities {
    /// Creates a new capabilities set from the inner value.
    #[inline]
    pub const fn new(inner: u64) -> Self {
        Self(inner)
    }

    /// Returns `true` if the set contains no enabled capabilities.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Returns the number of enabled capabilities.
    pub const fn len(&self) -> usize {
        self.0.count_ones() as usize
    }

    /// Returns `true` if the specified capability is enabled.
    #[inline]
    pub const fn contains(&self, capability: GlobalCapability) -> bool {
        (self.0 & (capability as u64)) != 0
    }

    /// Converts this wrapper into an underlying type.
    #[inline]
    pub const fn into_inner(self) -> u64 {
        self.0
    }

    /// Gets an iterator over the enabled capabilities.
    #[inline]
    pub fn iter(&self) -> GlobalCapabilitiesIter {
        GlobalCapabilitiesIter(self.0)
    }
}

impl From<u64> for GlobalCapabilities {
    #[inline]
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<GlobalCapabilities> for u64 {
    #[inline]
    fn from(value: GlobalCapabilities) -> Self {
        value.0
    }
}

impl PartialEq<u64> for GlobalCapabilities {
    #[inline]
    fn eq(&self, other: &u64) -> bool {
        self.0 == *other
    }
}

impl IntoIterator for GlobalCapabilities {
    type Item = GlobalCapability;
    type IntoIter = GlobalCapabilitiesIter;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        GlobalCapabilitiesIter(self.0)
    }
}

impl FromIterator<GlobalCapability> for GlobalCapabilities {
    fn from_iter<T: IntoIterator<Item = GlobalCapability>>(iter: T) -> Self {
        let mut res = GlobalCapabilities::default();
        for item in iter {
            res |= item;
        }
        res
    }
}

impl<const N: usize> From<[GlobalCapability; N]> for GlobalCapabilities {
    fn from(value: [GlobalCapability; N]) -> Self {
        let mut res = GlobalCapabilities::default();
        for item in value.iter() {
            res |= *item;
        }
        res
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for GlobalCapabilities {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeSeq;

        if serializer.is_human_readable() {
            let mut seq = serializer.serialize_seq(Some(self.len()))?;
            for capability in self.iter() {
                seq.serialize_element(&capability)?;
            }
            seq.end()
        } else {
            serializer.serialize_u64(self.0)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for GlobalCapabilities {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Visitor;

        struct GlobalCapabilitiesVisitor;

        impl<'de> Visitor<'de> for GlobalCapabilitiesVisitor {
            type Value = GlobalCapabilities;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a list of global capabilities")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut res = GlobalCapabilities::default();
                while let Some(capacility) = ok!(seq.next_element::<GlobalCapability>()) {
                    res |= capacility;
                }
                Ok(res)
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_seq(GlobalCapabilitiesVisitor)
        } else {
            u64::deserialize(deserializer).map(Self)
        }
    }
}

/// An iterator over the enabled capabilities of [`GlobalCapabilities`].
///
/// This struct is created by the [`iter`] method on [`GlobalCapabilities`].
/// See its documentation for more.
///
/// [`iter`]: GlobalCapabilities::iter
#[derive(Clone)]
pub struct GlobalCapabilitiesIter(u64);

impl Iterator for GlobalCapabilitiesIter {
    type Item = GlobalCapability;

    fn next(&mut self) -> Option<Self::Item> {
        while self.0 != 0 {
            //  10100 - 1     = 10011
            // !10011         = 01100
            //  10100 & 01100 = 00100
            let mask = self.0 & !(self.0 - 1);

            // 10100 & !00100 -> 10000
            self.0 &= !mask;

            if let Some(item) = GlobalCapability::from_bit_offset(mask.trailing_zeros()) {
                return Some(item);
            }
        }

        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.0.count_ones() as usize;
        (len, Some(len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capabilities_iter() {
        let capabilities = GlobalCapability::CapCreateStatsEnabled
            | GlobalCapability::CapBounceMsgBody
            | GlobalCapability::CapReportVersion
            | GlobalCapability::CapShortDequeue
            | GlobalCapability::CapFastStorageStat
            | GlobalCapability::CapOffHypercube
            | GlobalCapability::CapMyCode
            | GlobalCapability::CapFixTupleIndexBug;

        let capabilities = capabilities.into_iter().collect::<Vec<_>>();
        assert_eq!(capabilities, [
            GlobalCapability::CapCreateStatsEnabled,
            GlobalCapability::CapBounceMsgBody,
            GlobalCapability::CapReportVersion,
            GlobalCapability::CapShortDequeue,
            GlobalCapability::CapFastStorageStat,
            GlobalCapability::CapOffHypercube,
            GlobalCapability::CapMyCode,
            GlobalCapability::CapFixTupleIndexBug
        ]);
    }
}
