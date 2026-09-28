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

        /// Store the outgoing message queue size in the shard state.
        ///
        /// Mask: `0x0000040`.
        CapStoreOutMsgQueueSize = 6,

        /// Include message metadata in message envelopes.
        ///
        /// Mask: `0x0000080`.
        CapMsgMetadata = 7,

        /// Support deferred message dispatch.
        ///
        /// Mask: `0x0000100`.
        CapDeferMessages = 8,

        /// Include full collated data for block validation.
        ///
        /// Mask: `0x0000200`.
        CapFullCollatedData = 9,
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
/// Binary serialization preserves the full mask. Human-readable serialization
/// contains only the names of known [`GlobalCapability`] flags.
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
                while let Some(capability) = ok!(seq.next_element::<GlobalCapability>()) {
                    res |= capability;
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
        let capabilities = GlobalCapabilities::new(0x3ff);

        let capabilities = capabilities.into_iter().collect::<Vec<_>>();
        assert_eq!(
            capabilities,
            [
                GlobalCapability::CapIhrEnabled,
                GlobalCapability::CapCreateStatsEnabled,
                GlobalCapability::CapBounceMsgBody,
                GlobalCapability::CapReportVersion,
                GlobalCapability::CapSplitMergeTransactions,
                GlobalCapability::CapShortDequeue,
                GlobalCapability::CapStoreOutMsgQueueSize,
                GlobalCapability::CapMsgMetadata,
                GlobalCapability::CapDeferMessages,
                GlobalCapability::CapFullCollatedData
            ]
        );

        #[cfg(feature = "serde")]
        {
            let json = serde_json::to_string(&GlobalCapabilities::new(0x3ff)).unwrap();
            let parsed: GlobalCapabilities = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed.into_inner(), 0x3ff);
            assert_eq!(
                serde_json::from_str::<Vec<String>>(&json).unwrap(),
                capabilities
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            );
        }
    }
}
