//! Integer types used in blockchain models.

use std::num::NonZeroU8;

pub use self::varuint248::VarUint248;
use crate::cell::*;
use crate::error::{Error, ParseIntError};
use crate::util::unlikely;

mod varuint248;

macro_rules! impl_serde {
    ($ident:ident, $inner: ty) => {
        #[cfg(feature = "serde")]
        impl serde::Serialize for $ident {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.0.serialize(serializer)
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for $ident {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::de::{Error, Unexpected};

                struct Expected;

                impl serde::de::Expected for Expected {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        f.write_str(stringify!($ident))
                    }
                }

                let res = Self::new(ok!(<$inner>::deserialize(deserializer)));
                if res.is_valid() {
                    Ok(res)
                } else {
                    Err(D::Error::invalid_type(
                        Unexpected::Other("big number"),
                        &Expected,
                    ))
                }
            }
        }
    };
}

macro_rules! impl_ops {
    ($ident:ident, $inner:ty) => {
        impl From<$ident> for $inner {
            #[inline]
            fn from(value: $ident) -> Self {
                value.0
            }
        }

        impl TryFrom<$inner> for $ident {
            type Error = ParseIntError;

            #[inline]
            fn try_from(inner: $inner) -> Result<Self, Self::Error> {
                let result = Self::new(inner);
                if result.is_valid() {
                    Ok(result)
                } else {
                    Err(ParseIntError::Overflow)
                }
            }
        }

        #[cfg(feature = "bigint")]
        impl From<$ident> for num_bigint::BigInt {
            #[inline]
            fn from(value: $ident) -> Self {
                Self::from(value.0)
            }
        }

        #[cfg(feature = "bigint")]
        impl From<$ident> for num_bigint::BigUint {
            #[inline]
            fn from(value: $ident) -> Self {
                Self::from(value.0)
            }
        }

        impl std::str::FromStr for $ident {
            type Err = ParseIntError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match std::str::FromStr::from_str(s) {
                    Ok(inner) => {
                        let result = Self::new(inner);
                        if result.is_valid() {
                            Ok(result)
                        } else {
                            Err(ParseIntError::Overflow)
                        }
                    }
                    Err(e) => Err(ParseIntError::InvalidString(e)),
                }
            }
        }

        impl PartialEq<$inner> for $ident {
            #[inline]
            fn eq(&self, other: &$inner) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<$ident> for $inner {
            #[inline]
            fn eq(&self, other: &$ident) -> bool {
                *self == other.0
            }
        }

        impl std::fmt::Display for $ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl std::fmt::Binary for $ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Binary::fmt(&self.0, f)
            }
        }

        impl std::fmt::LowerHex for $ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::LowerHex::fmt(&self.0, f)
            }
        }

        impl std::fmt::UpperHex for $ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::UpperHex::fmt(&self.0, f)
            }
        }

        impl std::ops::Add for $ident {
            type Output = Self;

            #[inline]
            fn add(mut self, rhs: Self) -> Self::Output {
                self.0 += rhs.0;
                self
            }
        }

        impl std::ops::Add<$inner> for $ident {
            type Output = Self;

            #[inline]
            fn add(mut self, rhs: $inner) -> Self::Output {
                self.0 += rhs;
                self
            }
        }

        impl std::ops::AddAssign for $ident {
            #[inline]
            fn add_assign(&mut self, rhs: Self) {
                self.0 += rhs.0;
            }
        }

        impl std::ops::AddAssign<$inner> for $ident {
            fn add_assign(&mut self, rhs: $inner) {
                self.0 += rhs;
            }
        }

        impl std::ops::Sub for $ident {
            type Output = Self;

            #[inline]
            fn sub(mut self, rhs: Self) -> Self::Output {
                self.0 -= rhs.0;
                self
            }
        }

        impl std::ops::Sub<$inner> for $ident {
            type Output = Self;

            #[inline]
            fn sub(mut self, rhs: $inner) -> Self::Output {
                self.0 -= rhs;
                self
            }
        }

        impl std::ops::SubAssign for $ident {
            #[inline]
            fn sub_assign(&mut self, rhs: Self) {
                self.0 -= rhs.0;
            }
        }

        impl std::ops::SubAssign<$inner> for $ident {
            #[inline]
            fn sub_assign(&mut self, rhs: $inner) {
                self.0 -= rhs;
            }
        }

        impl std::ops::Mul for $ident {
            type Output = Self;

            #[inline]
            fn mul(mut self, rhs: Self) -> Self::Output {
                self.0 *= rhs.0;
                self
            }
        }

        impl std::ops::Mul<$inner> for $ident {
            type Output = Self;

            #[inline]
            fn mul(mut self, rhs: $inner) -> Self::Output {
                self.0 *= rhs;
                self
            }
        }

        impl std::ops::MulAssign for $ident {
            #[inline]
            fn mul_assign(&mut self, rhs: Self) {
                self.0 *= rhs.0;
            }
        }

        impl std::ops::MulAssign<$inner> for $ident {
            #[inline]
            fn mul_assign(&mut self, rhs: $inner) {
                self.0 *= rhs;
            }
        }

        impl std::ops::Div for $ident {
            type Output = Self;

            #[inline]
            fn div(mut self, rhs: Self) -> Self::Output {
                self.0 /= rhs.0;
                self
            }
        }

        impl std::ops::Div<$inner> for $ident {
            type Output = Self;

            #[inline]
            fn div(mut self, rhs: $inner) -> Self::Output {
                self.0 /= rhs;
                self
            }
        }

        impl std::ops::DivAssign for $ident {
            #[inline]
            fn div_assign(&mut self, rhs: Self) {
                self.0 /= rhs.0;
            }
        }

        impl std::ops::DivAssign<$inner> for $ident {
            #[inline]
            fn div_assign(&mut self, rhs: $inner) {
                self.0 /= rhs;
            }
        }

        impl std::ops::Shr<u8> for $ident {
            type Output = Self;

            #[inline]
            fn shr(mut self, rhs: u8) -> Self::Output {
                self.0 >>= rhs;
                self
            }
        }

        impl std::ops::ShrAssign<u8> for $ident {
            #[inline]
            fn shr_assign(&mut self, rhs: u8) {
                self.0 >>= rhs;
            }
        }

        impl std::ops::Shl<u8> for $ident {
            type Output = Self;

            #[inline]
            fn shl(mut self, rhs: u8) -> Self::Output {
                self.0 <<= rhs;
                self
            }
        }

        impl std::ops::ShlAssign<u8> for $ident {
            #[inline]
            fn shl_assign(&mut self, rhs: u8) {
                self.0 <<= rhs;
            }
        }
    };
}

macro_rules! impl_var_uints {
    ($($(#[doc = $doc:expr])* $vis:vis struct $ident:ident($inner:ty[..$max_bytes:literal]);)*) => {
        $(
            impl_var_uints!{@impl $(#[doc = $doc])* $vis $ident $inner, $max_bytes}
        )*
    };

    (@impl $(#[doc = $doc:expr])* $vis:vis $ident:ident $inner:ty, $max_bytes:literal) => {
        $(#[doc = $doc])*
        #[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
        #[repr(transparent)]
        $vis struct $ident($inner);

        impl $ident {
            /// The additive identity for this integer type, i.e. `0`.
            pub const ZERO: Self = $ident(0);

            /// The multiplicative identity for this integer type, i.e. `1`.
            pub const ONE: Self = $ident(1);

            /// The smallest value that can be represented by this integer type.
            pub const MIN: Self = $ident(0);

            /// The largest value that can be represented by this integer type.
            pub const MAX: Self = $ident(((1 as $inner) << Self::VALUE_BITS) - 1);

            /// The number of data bits that the length occupies.
            pub const LEN_BITS: u16 = 8 - ($max_bytes as u8).leading_zeros() as u16;

            /// The maximum number of data bits that this struct occupies.
            pub const MAX_BITS: u16 = Self::LEN_BITS + Self::VALUE_BITS;

            /// The maximum number of bits for the value.
            pub const VALUE_BITS: u16 = $max_bytes * 8;

            /// Creates a new integer value from a primitive integer.
            #[inline]
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }

            /// Converts integer into an underlying primitive integer.
            #[inline]
            pub const fn into_inner(self) -> $inner {
                self.0
            }

            /// Returns `true` if an underlying primitive integer is zero.
            #[inline]
            pub const fn is_zero(&self) -> bool {
                self.0 == 0
            }

            /// Returns `true` if an underlying primitive integer fits into the repr.
            #[inline]
            pub const fn is_valid(&self) -> bool {
                self.0 <= Self::MAX.0
            }

            /// Returns number of data bits that this struct occupies.
            /// Returns `None` if an underlying primitive integer is too large.
            pub const fn bit_len(&self) -> Option<u16> {
                let bytes = (std::mem::size_of::<Self>() as u32 - self.0.leading_zeros() / 8) as u8;
                if unlikely(bytes > $max_bytes) {
                    None
                } else {
                    Some(Self::LEN_BITS + bytes as u16 * 8)
                }
            }

            /// Returns number of data bits that this struct occupies.
            /// Returns [`MAX_BITS`] if an underlying primitive integer is too large.
            ///
            /// [`MAX_BITS`]: Self::MAX_BITS
            pub const fn unwrap_bit_len(&self) -> u16 {
                let bytes = (std::mem::size_of::<Self>() as u32 - self.0.leading_zeros() / 8) as u8;
                if unlikely(bytes > $max_bytes) {
                    Self::MAX_BITS
                } else {
                    Self::LEN_BITS + bytes as u16 * 8
                }
            }

            /// Saturating integer addition. Computes `self + rhs`,
            /// saturating at the numeric bounds instead of overflowing.
            #[inline]
            #[must_use]
            pub const fn saturating_add(self, rhs: Self) -> Self {
                match self.0.checked_add(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => $ident(value),
                    _ => Self::MAX,
                }
            }

            /// Saturating integer addition. Computes `self - rhs`,
            /// saturating at the numeric bounds instead of overflowing.
            #[inline]
            #[must_use]
            pub const fn saturating_sub(self, rhs: Self) -> Self {
                match self.0.checked_sub(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => $ident(value),
                    Some(_) => Self::MAX,
                    None => Self::ZERO,
                }
            }

            /// Saturating integer multiplication. Computes `self * rhs`,
            /// returning `None` if overflow occurred.
            #[inline]
            #[must_use]
            pub const fn saturating_mul(self, rhs: Self) -> Self {
                match self.0.checked_mul(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => $ident(value),
                    _ => Self::MAX,
                }
            }

            /// Checked integer addition. Computes `self + rhs`, returning `None` if overflow occurred.
            #[inline]
            #[must_use]
            pub const fn checked_add(self, rhs: Self) -> Option<Self> {
                match self.0.checked_add(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => Some($ident(value)),
                    _ => None,
                }
            }

            /// Checked integer subtraction. Computes `self - rhs`, returning `None` if overflow occurred.
            #[inline]
            #[must_use]
            pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
                match self.0.checked_sub(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => Some($ident(value)),
                    _ => None,
                }
            }

            /// Checked integer multiplication. Computes `self * rhs`, returning `None` if overflow occurred.
            #[inline]
            #[must_use]
            pub const fn checked_mul(self, rhs: Self) -> Option<Self> {
                match self.0.checked_mul(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => Some($ident(value)),
                    _ => None,
                }
            }

            /// Checked integer division. Computes `self / rhs`, returning None if `rhs == 0`
            /// or overflow occurred.
            #[inline]
            #[must_use]
            pub const fn checked_div(self, rhs: Self) -> Option<Self> {
                match self.0.checked_div(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => Some($ident(value)),
                    _ => None,
                }
            }

            /// Tries to add an other value to the current one.
            pub fn try_add_assign(&mut self, other: Self) -> Result<(), Error> {
                match self.checked_add(other) {
                    Some(new_value) => {
                        *self = new_value;
                        Ok(())
                    },
                    None => Err(Error::IntOverflow),
                }
            }

            /// Tries to subtract an other value from the current one.
            pub fn try_sub_assign(&mut self, other: Self) -> Result<(), Error> {
                match self.checked_sub(other) {
                    Some(new_value) => {
                        *self = new_value;
                        Ok(())
                    },
                    None => Err(Error::IntOverflow),
                }
            }

            /// Tries to multiply the current value by the other value.
            pub fn try_mul_assign(&mut self, other: Self) -> Result<(), Error> {
                match self.checked_mul(other) {
                    Some(new_value) => {
                        *self = new_value;
                        Ok(())
                    },
                    None => Err(Error::IntOverflow),
                }
            }

            /// Tries to divide the current value by the other value.
            pub fn try_div_assign(&mut self, other: Self) -> Result<(), Error> {
                match self.checked_div(other) {
                    Some(new_value) => {
                        *self = new_value;
                        Ok(())
                    },
                    None => Err(Error::IntOverflow),
                }
            }
        }

        impl ExactSize for $ident {
            #[inline]
            fn exact_size(&self) -> Size {
                Size {
                    bits: self.bit_len().unwrap_or_default(),
                    refs: 0,
                }
            }
        }

        impl_ops! { $ident, $inner }
    };
}

impl_var_uints! {
    /// Variable-length 24-bit integer.
    ///
    /// Stored as 2 bits of `len` (`0..=3`), followed by `len` bytes.
    pub struct VarUint24(u32[..3]);

    /// Variable-length 56-bit integer.
    ///
    /// Stored as 3 bits of `len` (`0..=7`), followed by `len` bytes.
    pub struct VarUint56(u64[..7]);

    /// Variable-length 120-bit integer. Used for native currencies.
    ///
    /// Stored as 4 bits of `len` (`0..=15`), followed by `len` bytes.
    pub struct Tokens(u128[..15]);
}

impl_serde!(VarUint24, u32);
impl_serde!(VarUint56, u64);

#[cfg(feature = "serde")]
impl serde::Serialize for Tokens {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.collect_str(&self.0)
        } else {
            self.0.serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Tokens {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Error, Unexpected, Visitor};

        struct Expected;

        impl serde::de::Expected for Expected {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(stringify!($ident))
            }
        }

        struct TokensVisitor;

        impl Visitor<'_> for TokensVisitor {
            type Value = u128;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string with a number")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                v.parse().map_err(E::custom)
            }
        }

        let res = Self::new(ok!(if deserializer.is_human_readable() {
            deserializer.deserialize_str(TokensVisitor)
        } else {
            u128::deserialize(deserializer)
        }));

        if res.is_valid() {
            Ok(res)
        } else {
            Err(D::Error::invalid_type(
                Unexpected::Other("big number"),
                &Expected,
            ))
        }
    }
}

impl Store for VarUint24 {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        let bytes = (4 - self.0.leading_zeros() / 8) as u8;
        let bits = bytes as u16 * 8;

        if unlikely(bytes > 3 || !builder.has_capacity(Self::LEN_BITS + bits, 0)) {
            return Err(Error::CellOverflow);
        }

        ok!(builder.store_small_uint(bytes, Self::LEN_BITS));
        builder.store_uint(self.0 as u64, bits)
    }
}

impl<'a> Load<'a> for VarUint24 {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let bytes = ok!(slice.load_small_uint(Self::LEN_BITS));
        match slice.load_uint(bytes as u16 * 8) {
            Ok(value) => Ok(Self(value as u32)),
            Err(e) => Err(e),
        }
    }
}

impl Store for VarUint56 {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        let bytes = (8 - self.0.leading_zeros() / 8) as u8;
        let bits = bytes as u16 * 8;

        if unlikely(bytes > 7 || !builder.has_capacity(Self::LEN_BITS + bits, 0)) {
            return Err(Error::CellOverflow);
        }

        ok!(builder.store_small_uint(bytes, Self::LEN_BITS));
        builder.store_uint(self.0, bits)
    }
}

impl<'a> Load<'a> for VarUint56 {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let bytes = ok!(slice.load_small_uint(Self::LEN_BITS));
        match slice.load_uint(bytes as u16 * 8) {
            Ok(value) => Ok(Self(value)),
            Err(e) => Err(e),
        }
    }
}

impl Store for Tokens {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        let bytes = (16 - self.0.leading_zeros() / 8) as u8;
        let bits = bytes as u16 * 8;

        if unlikely(bytes > 15 || !builder.has_capacity(Self::LEN_BITS + bits, 0)) {
            return Err(Error::CellOverflow);
        }

        ok!(builder.store_small_uint(bytes, Self::LEN_BITS));
        store_u128(builder, self.0, bits)
    }
}

impl<'a> Load<'a> for Tokens {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let bytes = ok!(slice.load_small_uint(Self::LEN_BITS));
        match load_u128(slice, bytes) {
            Ok(value) => Ok(Self(value)),
            Err(e) => Err(e),
        }
    }
}

macro_rules! impl_small_uints {
    ($($(#[doc = $doc:expr])* $vis:vis struct $ident:ident($bits:literal);)*) => {
        $(
            impl_small_uints!{@impl $(#[doc = $doc])* $vis $ident, $bits}
        )*
    };

    (@impl $(#[doc = $doc:expr])* $vis:vis $ident:ident, $bits:literal) => {
        $(#[doc = $doc])*
        #[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
        #[repr(transparent)]
        $vis struct $ident(u16);

        impl $ident {
            /// The additive identity for this integer type, i.e. `0`.
            pub const ZERO: Self = $ident(0);

            /// The multiplicative identity for this integer type, i.e. `1`.
            pub const ONE: Self = $ident(1);

            /// The smallest value that can be represented by this integer type.
            pub const MIN: Self = $ident(0);

            /// The largest value that can be represented by this integer type.
            pub const MAX: Self = $ident((1u16 << $bits) - 1);

            /// The number of data bits that this struct occupies.
            pub const BITS: u16 = $bits;

            /// Creates a new integer value from a primitive integer.
            #[inline]
            pub const fn new(value: u16) -> Self {
                Self(value)
            }

            /// Converts integer into an underlying primitive integer.
            #[inline]
            pub const fn into_inner(self) -> u16 {
                self.0
            }

            /// Returns `true` if an underlying primitive integer is zero.
            #[inline]
            pub const fn is_zero(&self) -> bool {
                self.0 == 0
            }

            /// Returns `true` if an underlying primitive integer fits into the repr.
            #[inline]
            pub const fn is_valid(&self) -> bool {
                self.0 <= Self::MAX.0
            }

            /// Saturating integer addition. Computes `self + rhs`,
            /// saturating at the numeric bounds instead of overflowing.
            #[inline]
            #[must_use]
            pub const fn saturating_add(self, rhs: Self) -> Self {
                match self.0.checked_add(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => $ident(value),
                    _ => Self::MAX,
                }
            }

            /// Saturating integer addition. Computes `self - rhs`,
            /// saturating at the numeric bounds instead of overflowing.
            #[inline]
            #[must_use]
            pub const fn saturating_sub(self, rhs: Self) -> Self {
                match self.0.checked_sub(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => $ident(value),
                    Some(_) => Self::MAX,
                    None => Self::MIN,
                }
            }

            /// Saturating integer multiplication. Computes `self * rhs`,
            /// returning `None` if overflow occurred.
            #[inline]
            #[must_use]
            pub const fn saturating_mul(self, rhs: Self) -> Self {
                match self.0.checked_mul(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => $ident(value),
                    _ => Self::MAX,
                }
            }

            /// Checked integer addition. Computes `self + rhs`, returning `None` if overflow occurred.
            #[inline]
            #[must_use]
            pub const fn checked_add(self, rhs: Self) -> Option<Self> {
                match self.0.checked_add(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => Some($ident(value)),
                    _ => None,
                }
            }

            /// Checked integer subtraction. Computes `self - rhs`, returning `None` if overflow occurred.
            #[inline]
            #[must_use]
            pub const fn checked_sub(self, rhs: Self) -> Option<Self> {
                match self.0.checked_sub(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => Some($ident(value)),
                    _ => None,
                }
            }

            /// Checked integer multiplication. Computes `self * rhs`, returning `None` if overflow occurred.
            #[inline]
            #[must_use]
            pub const fn checked_mul(self, rhs: Self) -> Option<Self> {
                match self.0.checked_mul(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => Some($ident(value)),
                    _ => None,
                }
            }

            /// Checked integer division. Computes `self / rhs`, returning None if `rhs == 0`
            /// or overflow occurred.
            #[inline]
            #[must_use]
            pub const fn checked_div(self, rhs: Self) -> Option<Self> {
                match self.0.checked_div(rhs.0) {
                    Some(value) if value <= Self::MAX.0 => Some($ident(value)),
                    _ => None,
                }
            }
        }

        impl ExactSize for $ident {
            #[inline]
            fn exact_size(&self) -> Size {
                Size { bits: $bits, refs: 0 }
            }
        }

        impl Store for $ident {
            fn store_into(
                &self,
                builder: &mut CellBuilder,
                _: &dyn CellContext
            ) -> Result<(), Error> {
                if !self.is_valid() {
                    return Err(Error::IntOverflow);
                }
                builder.store_uint(self.0 as u64, Self::BITS)
            }
        }

        impl<'a> Load<'a> for $ident {
            fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
                match slice.load_uint(Self::BITS) {
                    Ok(value) => Ok(Self(value as u16)),
                    Err(e) => Err(e),
                }
            }
        }

        impl crate::dict::DictKey for $ident {
            const BITS: u16 = $bits;
        }

        impl crate::dict::StoreDictKey for $ident {
            #[inline]
            fn store_into_data(&self, builder: &mut CellDataBuilder) -> Result<(), Error> {
                if !self.is_valid() {
                    return Err(Error::IntOverflow);
                }
                builder.store_uint(self.0 as u64, Self::BITS)
            }
        }

        impl crate::dict::LoadDictKey for $ident {
            #[inline]
            fn load_from_data(data: &CellDataBuilder) -> Option<Self> {
                let d = data.raw_data();
                Some($ident(u16::from_be_bytes([d[0], d[1]]) >> (16 - $bits)))
            }
        }

        impl_ops! { $ident, u16 }
    };
}

impl_small_uints! {
    /// Fixed-length 9-bit integer.
    pub struct Uint9(9);

    /// Fixed-length 12-bit integer.
    pub struct Uint12(12);

    /// Fixed-length 15-bit integer.
    pub struct Uint15(15);
}

impl_serde!(Uint9, u16);
impl_serde!(Uint12, u16);
impl_serde!(Uint15, u16);

#[cfg(feature = "arbitrary")]
macro_rules! impl_arbitrary {
    ($($ty:ty => $n:literal),*$(,)?) => {
        $(impl<'a> arbitrary::Arbitrary<'a> for $ty {
            #[inline]
            fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
                u.int_in_range(0..=<$ty>::MAX.into_inner()).map(<$ty>::new)
            }

            #[inline]
            fn size_hint(_: usize) -> (usize, Option<usize>) {
                ($n, Some($n))
            }
        })*
    };
}

#[cfg(feature = "arbitrary")]
impl_arbitrary! {
    Uint9 => 2,
    Uint12 => 2,
    Uint15 => 2,
    VarUint24 => 4,
    VarUint56 => 8,
    Tokens => 16,
}

/// Account split depth. Fixed-length 5-bit integer of range `1..=30`
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[repr(transparent)]
pub struct SplitDepth(NonZeroU8);

impl SplitDepth {
    /// The minimum allowed number of bits in the rewrite prefix.
    pub const MIN: Self = match NonZeroU8::new(1) {
        Some(value) => Self(value),
        None => unreachable!(),
    };

    /// The maximum allowed number of bits in the rewrite prefix.
    pub const MAX: Self = match NonZeroU8::new(30) {
        Some(value) => Self(value),
        None => unreachable!(),
    };

    /// The number of data bits that this struct occupies.
    pub const BITS: u16 = 5;

    /// Creates a new integer value from a primitive integer.
    #[inline]
    pub const fn new(value: u8) -> Result<Self, Error> {
        match NonZeroU8::new(value) {
            Some(value) => Ok(Self(value)),
            None => Err(Error::IntOverflow),
        }
    }

    /// Creates a new integer value from bit len.
    #[inline]
    pub const fn from_bit_len(bit_len: u16) -> Result<Self, Error> {
        if bit_len < u8::MAX as u16 {
            Self::new(bit_len as u8)
        } else {
            Err(Error::IntOverflow)
        }
    }

    /// Converts split depths into the number of bits.
    #[inline]
    pub const fn into_bit_len(self) -> u16 {
        self.0.get() as u16
    }
}

#[cfg(feature = "bigint")]
impl From<SplitDepth> for num_bigint::BigInt {
    #[inline]
    fn from(value: SplitDepth) -> Self {
        Self::from(value.0.get())
    }
}

#[cfg(feature = "bigint")]
impl From<SplitDepth> for num_bigint::BigUint {
    #[inline]
    fn from(value: SplitDepth) -> Self {
        Self::from(value.0.get())
    }
}

impl ExactSize for SplitDepth {
    #[inline]
    fn exact_size(&self) -> Size {
        Size {
            bits: Self::BITS,
            refs: 0,
        }
    }
}

impl Store for SplitDepth {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        builder.store_small_uint(self.0.get(), Self::BITS)
    }
}

impl<'a> Load<'a> for SplitDepth {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match slice.load_small_uint(Self::BITS) {
            Ok(value) => Self::new(value),
            Err(e) => Err(e),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for SplitDepth {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.get().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for SplitDepth {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match u8::deserialize(deserializer) {
            Ok(value) => Self::new(value).map_err(serde::de::Error::custom),
            Err(e) => Err(e),
        }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for SplitDepth {
    #[inline]
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        const MIN: u8 = SplitDepth::MIN.into_bit_len() as u8;
        const MAX: u8 = SplitDepth::MAX.into_bit_len() as u8;
        Ok(Self::new(u.int_in_range(MIN..=MAX)?).unwrap())
    }

    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (1, Some(1))
    }
}

fn store_u128(builder: &mut CellBuilder, value: u128, mut bits: u16) -> Result<(), Error> {
    if let Some(high_bits) = bits.checked_sub(64) {
        ok!(builder.store_uint((value >> 64) as u64, high_bits));
        bits -= high_bits;
    }
    builder.store_uint(value as u64, bits)
}

fn load_u128(slice: &mut CellSlice<'_>, mut bytes: u8) -> Result<u128, Error> {
    let mut result: u128 = 0;
    if let Some(high_bytes) = bytes.checked_sub(8)
        && high_bytes > 0
    {
        result = (ok!(slice.load_uint(high_bytes as u16 * 8)) as u128) << 64;
        bytes -= high_bytes;
    }

    match slice.load_uint(bytes as u16 * 8) {
        Ok(value) => Ok(result | value as u128),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::CellBuilder;

    macro_rules! impl_operation_tests {
        ($ident:ident$(, $check_max_div:ident)?) => {
            assert_eq!($ident::new(10) + $ident::new(4), $ident::new(14));
            assert_eq!($ident::new(10) + 4, $ident::new(14));

            assert_eq!($ident::new(10) - $ident::new(4), $ident::new(6));
            assert_eq!($ident::new(10) - 4, $ident::new(6));

            assert_eq!($ident::new(10) * $ident::new(4), $ident::new(40));
            assert_eq!($ident::new(10) * 4, $ident::new(40));

            assert_eq!($ident::new(10) / $ident::new(2), $ident::new(5));
            assert_eq!($ident::new(10) / 2, $ident::new(5));

            assert_eq!($ident::new(10) >> 2, $ident::new(2));
            assert_eq!($ident::new(10) << 2, $ident::new(40));

            let mut value = $ident::new(10);
            value += 4;
            assert_eq!(value, $ident::new(14));

            let mut value = $ident::new(10);
            value -= 4;
            assert_eq!(value, $ident::new(6));

            let mut value = $ident::new(10);
            value *= 4;
            assert_eq!(value, $ident::new(40));

            let mut value = $ident::new(10);
            value /= 2;
            assert_eq!(value, $ident::new(5));

            let mut value = $ident::new(10);
            value >>= 2;
            assert_eq!(value, $ident::new(2));

            let mut value = $ident::new(10);
            value <<= 2;
            assert_eq!(value, $ident::new(40));

            assert!(!($ident::MAX + 1).is_valid());

            assert_eq!($ident::MAX.checked_add($ident::new(1)), None);
            assert_eq!(
                ($ident::MAX - 1).checked_add($ident::new(1)),
                Some($ident::MAX)
            );

            assert_eq!(($ident::MAX + 10).checked_sub($ident::new(1)), None);
            assert_eq!(
                ($ident::MAX + 10).checked_sub($ident::MAX),
                Some($ident::new(10)),
            );
            assert_eq!($ident::new(10).checked_sub($ident::MAX), None);

            assert_eq!($ident::MAX.checked_mul($ident::new(2)), None);
            assert_eq!(
                ($ident::MAX / 2).checked_mul($ident::new(2)),
                Some($ident::MAX - 1)
            );

            $(
                let $check_max_div = ();
                _ = $check_max_div;
                assert_eq!((($ident::MAX + 1) * 2).checked_div($ident::new(2)), None);
                assert_eq!(
                    ($ident::MAX * 2).checked_div($ident::new(2)),
                    Some($ident::MAX)
                );
                assert_eq!($ident::ONE.checked_div($ident::ZERO), None);
            )?
        };
    }

    macro_rules! impl_serialization_tests {
        ($ident:ident, $max_bits:literal) => {
            let context = Cell::empty_context();

            for i in 0..$max_bits {
                let value = $ident::ONE << i;
                let mut builder = CellBuilder::new();

                if value <= $ident::MAX {
                    value.store_into(&mut builder, context).unwrap();
                    let cell = builder.build().unwrap();
                    assert_eq!(value.bit_len().unwrap(), cell.bit_len());
                } else {
                    assert!(value.store_into(&mut builder, context).is_err());
                }
            }
        };
    }

    macro_rules! impl_deserialization_tests {
        ($ident:ident, $max_bits:literal, $value:literal) => {
            let context = Cell::empty_context();

            let mut value = $ident::new($value);
            for _ in 0..=$max_bits {
                let mut builder = CellBuilder::new();
                value.store_into(&mut builder, context).unwrap();
                let cell = builder.build().unwrap();

                let parsed_value = cell.parse::<$ident>().unwrap();
                assert_eq!(parsed_value, value);

                value >>= 1;
            }
        };
    }

    macro_rules! impl_fixed_len_serialization_tests {
        ($ident:ident, $max_bits:literal) => {
            let context = Cell::empty_context();

            for i in 0..$max_bits {
                let value = $ident::ONE << i;
                let mut builder = CellBuilder::new();

                if value <= $ident::MAX {
                    value.store_into(&mut builder, context).unwrap();
                    let cell = builder.build().unwrap();
                    assert_eq!($ident::BITS, cell.bit_len());
                } else {
                    assert!(value.store_into(&mut builder, context).is_err());
                }
            }
        };
    }

    #[test]
    fn fixed_len_operations() {
        impl_operation_tests!(Uint9, check_max_div);
        impl_operation_tests!(Uint12, check_max_div);
        impl_operation_tests!(Uint15);
    }

    #[test]
    fn fixed_len_serialization() {
        impl_fixed_len_serialization_tests!(Uint9, 16);
        impl_fixed_len_serialization_tests!(Uint12, 16);
        impl_fixed_len_serialization_tests!(Uint15, 16);
    }

    #[test]
    fn fixed_len_deserialization() {
        impl_deserialization_tests!(Uint9, 9, 0b100110011);
        impl_deserialization_tests!(Uint12, 12, 0b111100110011);
        impl_deserialization_tests!(Uint15, 15, 0b11111100110011);
    }

    #[test]
    fn var_uint24_operations() {
        impl_operation_tests!(VarUint24, check_max_div);
    }

    #[test]
    fn var_uint56_operations() {
        impl_operation_tests!(VarUint56, check_max_div);
    }

    #[test]
    fn tokens_operations() {
        impl_operation_tests!(Tokens, check_max_div);
    }

    #[test]
    fn var_uint24_serialization() {
        impl_serialization_tests!(VarUint24, 32);
    }

    #[test]
    fn var_uint56_serialization() {
        impl_serialization_tests!(VarUint56, 64);
    }

    #[test]
    fn tokens_serialization() {
        impl_serialization_tests!(Tokens, 128);
    }

    #[test]
    fn var_uint24_deserialization() {
        impl_deserialization_tests!(VarUint24, 24, 0xabcdef);
    }

    #[test]
    fn var_uint56_deserialization() {
        impl_deserialization_tests!(VarUint56, 56, 0xabcdef89abcdef);
    }

    #[test]
    fn tokens_deserialization() {
        impl_deserialization_tests!(Tokens, 120, 0xabcdef89abcdefdeadbeeffafacafe);
    }

    fn _num_must_use() {
        #[expect(unused_must_use)]
        {
            Uint9::new(10).checked_add(Uint9::ZERO);
        }

        #[expect(unused_must_use)]
        {
            Uint12::new(10).checked_add(Uint12::ZERO);
        }

        #[expect(unused_must_use)]
        {
            Uint15::new(10).checked_add(Uint15::ZERO);
        }

        #[expect(unused_must_use)]
        {
            VarUint24::new(10).checked_add(VarUint24::ZERO);
        }

        #[expect(unused_must_use)]
        {
            VarUint56::new(10).checked_add(VarUint56::ZERO);
        }

        #[expect(unused_must_use)]
        {
            Tokens::new(10).checked_add(Tokens::ZERO);
        }

        #[expect(unused_must_use)]
        {
            VarUint248::new(10).checked_add(&VarUint248::ZERO);
        }
    }
}
