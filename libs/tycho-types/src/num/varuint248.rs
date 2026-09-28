use std::mem::MaybeUninit;
use std::str::FromStr;

use crate::cell::{CellBuilder, CellContext, CellSlice, ExactSize, Load, Size, Store};
use crate::error::Error;
use crate::util::unlikely;

/// Variable-length 248-bit integer.
///
/// Stored as 5 bits of `len` (`0..=31`), followed by `len` bytes.
#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct VarUint248([u128; 2]);

impl VarUint248 {
    /// The additive identity for this integer type, i.e. `0`.
    pub const ZERO: Self = Self([0, 0]);

    /// The multiplicative identity for this integer type, i.e. `1`.
    pub const ONE: Self = Self([1, 0]);

    /// The smallest value that can be represented by this integer type.
    pub const MIN: Self = Self::new(1);

    /// The largest value that can be represented by this integer type.
    pub const MAX: Self = Self::from_words(u128::MAX >> 8, u128::MAX);

    /// The number of data bits that the length occupies.
    pub const LEN_BITS: u16 = 5;

    /// The maximum number of data bits that this struct occupies.
    pub const MAX_BITS: u16 = Self::LEN_BITS + 31 * 8;

    /// Creates a new integer value from a primitive integer.
    #[inline]
    pub const fn new(value: u128) -> Self {
        Self::from_words(0, value)
    }

    /// Constructs self from a pair of high and low underlying integers.
    #[inline]
    pub const fn from_words(hi: u128, lo: u128) -> Self {
        #[cfg(target_endian = "little")]
        {
            Self([lo, hi])
        }
        #[cfg(target_endian = "big")]
        {
            Self([hi, lo])
        }
    }

    /// Returns a tuple of high and low underlying integers.
    #[inline]
    pub const fn into_words(self) -> (u128, u128) {
        #[cfg(target_endian = "little")]
        {
            (self.0[1], self.0[0])
        }
        #[cfg(target_endian = "big")]
        {
            (self.0[0], self.0[1])
        }
    }

    /// Converts a string slice in a given base to an integer.
    pub fn from_str_radix(src: &str, radix: u32) -> Result<Self, std::num::ParseIntError> {
        from_str_radix(src, radix, None)
    }

    /// Returns `true` if an underlying primitive integer is zero.
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.0[0] == 0 && self.0[1] == 0
    }

    /// Returns `true` if an underlying primitive integer fits into the repr.
    #[inline]
    pub const fn is_valid(&self) -> bool {
        *self.high() <= (u128::MAX >> 8)
    }

    /// Returns number of data bits that this struct occupies.
    /// Returns `None` if an underlying primitive integer is too large.
    pub const fn bit_len(&self) -> Option<u16> {
        let bytes = (32 - self.leading_zeros() / 8) as u8;
        if unlikely(bytes > 31) {
            None
        } else {
            Some(Self::LEN_BITS + bytes as u16 * 8)
        }
    }

    /// Returns the number of leading zeros in the binary representation of self.
    pub const fn leading_zeros(&self) -> u32 {
        let (hi, lo) = self.into_words();
        if hi == 0 {
            128 + lo.leading_zeros()
        } else {
            hi.leading_zeros()
        }
    }

    /// Saturating integer addition. Computes `self + rhs`,
    /// saturating at the numeric bounds instead of overflowing.
    #[must_use]
    pub const fn saturating_add(self, rhs: &Self) -> Self {
        match self.checked_add(rhs) {
            Some(value) => value,
            None => Self::MAX,
        }
    }

    /// Saturating integer addition. Computes `self - rhs`,
    /// saturating at the numeric bounds instead of overflowing.
    #[must_use]
    pub const fn saturating_sub(self, rhs: &Self) -> Self {
        let (lo, carry_lo) = self.low().overflowing_sub(*rhs.low());
        let (hi, carry_c) = self.high().overflowing_sub(carry_lo as _);
        let (hi, carry_hi) = hi.overflowing_sub(*rhs.high());

        if carry_c || carry_hi {
            return Self::ZERO;
        }

        let res = Self::from_words(hi, lo);
        if res.is_valid() { res } else { Self::MAX }
    }

    /// Saturating integer multiplication. Computes `self * rhs`,
    /// returning `None` if overflow occurred.
    #[must_use]
    pub fn saturating_mul(self, rhs: &Self) -> Self {
        match self.checked_mul(rhs) {
            Some(value) => value,
            None => Self::MAX,
        }
    }

    /// Checked integer addition. Computes `self + rhs`,
    /// returning `None` if overflow occurred.
    #[must_use]
    pub const fn checked_add(&self, rhs: &Self) -> Option<Self> {
        let (lo, carry_lo) = self.low().overflowing_add(*rhs.low());
        let (hi, carry_c) = self.high().overflowing_add(carry_lo as _);
        let (hi, carry_hi) = hi.overflowing_add(*rhs.high());

        if carry_c || carry_hi {
            return None;
        }

        let res = Self::from_words(hi, lo);
        if res.is_valid() { Some(res) } else { None }
    }

    /// Checked integer subtraction. Computes `self - rhs`,
    /// returning `None` if overflow occurred.
    #[must_use]
    pub const fn checked_sub(&self, rhs: &Self) -> Option<Self> {
        let (lo, carry_lo) = self.low().overflowing_sub(*rhs.low());
        let (hi, carry_c) = self.high().overflowing_sub(carry_lo as _);
        let (hi, carry_hi) = hi.overflowing_sub(*rhs.high());

        if carry_c || carry_hi {
            return None;
        }

        let res = Self::from_words(hi, lo);
        if res.is_valid() { Some(res) } else { None }
    }

    /// Checked integer multiplication. Computes `self * rhs`,
    /// returning `None` if overflow occurred.
    #[must_use]
    pub fn checked_mul(&self, rhs: &Self) -> Option<Self> {
        let mut res = umulddi3(self.low(), rhs.low());

        let (hi_lo, overflow_hi_lo) = self.high().overflowing_mul(*rhs.low());
        let (lo_hi, overflow_lo_hi) = self.low().overflowing_mul(*rhs.high());
        let (hi, overflow_hi) = hi_lo.overflowing_add(lo_hi);
        let (high, overflow_high) = res.high().overflowing_add(hi);
        *res.high_mut() = high;

        let overflow_hi_hi = (*self.high() != 0) & (*rhs.high() != 0);

        if overflow_hi_lo
            || overflow_lo_hi
            || overflow_hi
            || overflow_high
            || overflow_hi_hi
            || !self.is_valid()
        {
            None
        } else {
            Some(res)
        }
    }

    /// The lower part of the integer.
    pub const fn low(&self) -> &u128 {
        &self.0[0]
    }

    /// The higher part of the integer.
    pub const fn high(&self) -> &u128 {
        &self.0[1]
    }

    /// A mutable reference to the lower part of the integer.
    pub fn low_mut(&mut self) -> &mut u128 {
        &mut self.0[0]
    }

    /// A mutable reference to the higher part of the integer.
    pub fn high_mut(&mut self) -> &mut u128 {
        &mut self.0[1]
    }

    /// Cast to a primitive `isize`.
    #[inline]
    pub const fn as_isize(self) -> isize {
        let (_, lo) = self.into_words();
        lo as _
    }

    /// Cast to a primitive `usize`.
    #[inline]
    pub const fn as_usize(self) -> usize {
        let (_, lo) = self.into_words();
        lo as _
    }
}

macro_rules! impl_from {
    ($($ty:ty),*$(,)?) => {
        $(impl From<$ty> for VarUint248 {
            #[inline]
            fn from(value: $ty) -> Self {
                Self::new(value as _)
            }
        })*
    };
}

impl_from! { u8, u16, u32, u64, u128, usize }

#[cfg(feature = "bigint")]
impl From<VarUint248> for num_bigint::BigInt {
    #[inline]
    fn from(value: VarUint248) -> Self {
        num_bigint::BigUint::from(value).into()
    }
}

#[cfg(feature = "bigint")]
impl From<VarUint248> for num_bigint::BigUint {
    fn from(value: VarUint248) -> Self {
        let (high, low) = value.into_words();
        if high == 0 {
            return Self::from(low);
        }

        let mut res = Self::from(high);
        res <<= 128;
        res += low;
        res
    }
}

impl ExactSize for VarUint248 {
    #[inline]
    fn exact_size(&self) -> Size {
        Size {
            bits: self.bit_len().unwrap_or_default(),
            refs: 0,
        }
    }
}

impl PartialEq<u128> for VarUint248 {
    fn eq(&self, other: &u128) -> bool {
        self.into_words() == (0, *other)
    }
}

impl Ord for VarUint248 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.into_words().cmp(&other.into_words())
    }
}

impl PartialOrd for VarUint248 {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialOrd<u128> for VarUint248 {
    #[inline]
    fn partial_cmp(&self, other: &u128) -> Option<std::cmp::Ordering> {
        Some(self.into_words().cmp(&(0, *other)))
    }
}

impl Store for VarUint248 {
    fn store_into(&self, builder: &mut CellBuilder, _: &dyn CellContext) -> Result<(), Error> {
        let bytes = (32 - self.leading_zeros() / 8) as u8;
        let mut bits = bytes as u16 * 8;

        if unlikely(bytes > 31 || !builder.has_capacity(Self::LEN_BITS + bits, 0)) {
            return Err(Error::CellOverflow);
        }

        ok!(builder.store_small_uint(bytes, Self::LEN_BITS));

        let (hi, lo) = self.into_words();
        if let Some(high_bits) = bits.checked_sub(128) {
            ok!(super::store_u128(builder, hi, high_bits));
            bits -= high_bits;
        }
        super::store_u128(builder, lo, bits)
    }
}

impl<'a> Load<'a> for VarUint248 {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let mut bytes = ok!(slice.load_small_uint(Self::LEN_BITS));

        let mut hi: u128 = 0;
        if let Some(high_bytes) = bytes.checked_sub(16)
            && high_bytes > 0
        {
            hi = ok!(super::load_u128(slice, high_bytes));
            bytes -= high_bytes;
        }

        match super::load_u128(slice, bytes) {
            Ok(lo) => Ok(Self::from_words(hi, lo)),
            Err(e) => Err(e),
        }
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for VarUint248 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.collect_str(self)
        } else {
            self.0.serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for VarUint248 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Error, Unexpected, Visitor};

        struct VarUint248Visitor;

        impl Visitor<'_> for VarUint248Visitor {
            type Value = VarUint248;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a formatted 248-bit integer")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                from_str_radix(v, 10, None).map_err(Error::custom)
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                let Ok(string) = std::str::from_utf8(v) else {
                    return Err(Error::invalid_value(Unexpected::Bytes(v), &self));
                };
                self.visit_str(string)
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(VarUint248Visitor)
        } else {
            <_>::deserialize(deserializer).map(Self)
        }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for VarUint248 {
    #[inline]
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self::from_words(
            u.int_in_range(0..=(u128::MAX >> 8))?,
            u.arbitrary()?,
        ))
    }

    #[inline]
    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (32, Some(32))
    }
}

impl std::ops::Add<u128> for VarUint248 {
    type Output = Self;

    #[inline]
    fn add(mut self, rhs: u128) -> Self::Output {
        self += Self::new(rhs);
        self
    }
}

impl std::ops::Add for VarUint248 {
    type Output = Self;

    #[inline]
    fn add(mut self, rhs: Self) -> Self::Output {
        self += &rhs;
        self
    }
}

impl std::ops::Add<&Self> for VarUint248 {
    type Output = Self;

    #[inline]
    fn add(mut self, rhs: &Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl std::ops::AddAssign<u128> for VarUint248 {
    fn add_assign(&mut self, rhs: u128) {
        std::ops::AddAssign::<&Self>::add_assign(self, &Self::new(rhs))
    }
}

impl std::ops::AddAssign for VarUint248 {
    fn add_assign(&mut self, rhs: Self) {
        std::ops::AddAssign::<&Self>::add_assign(self, &rhs)
    }
}

impl std::ops::AddAssign<&Self> for VarUint248 {
    fn add_assign(&mut self, rhs: &Self) {
        let (lo, carry) = self.low().overflowing_add(*rhs.low());
        *self.low_mut() = lo;
        *self.high_mut() = rhs
            .high()
            .wrapping_add(carry as _)
            .wrapping_add(*rhs.high());
    }
}

impl std::ops::Sub<u128> for VarUint248 {
    type Output = Self;

    #[inline]
    fn sub(mut self, rhs: u128) -> Self::Output {
        self -= &Self::new(rhs);
        self
    }
}

impl std::ops::Sub for VarUint248 {
    type Output = Self;

    #[inline]
    fn sub(mut self, rhs: Self) -> Self::Output {
        self -= &rhs;
        self
    }
}

impl std::ops::Sub<&Self> for VarUint248 {
    type Output = Self;

    #[inline]
    fn sub(mut self, rhs: &Self) -> Self::Output {
        self -= rhs;
        self
    }
}

impl std::ops::SubAssign<u128> for VarUint248 {
    fn sub_assign(&mut self, rhs: u128) {
        std::ops::SubAssign::<&Self>::sub_assign(self, &Self::new(rhs))
    }
}

impl std::ops::SubAssign for VarUint248 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        std::ops::SubAssign::<&Self>::sub_assign(self, &rhs)
    }
}

impl std::ops::SubAssign<&Self> for VarUint248 {
    fn sub_assign(&mut self, rhs: &Self) {
        let (lo, carry) = self.low().overflowing_sub(*rhs.low());
        *self.low_mut() = lo;
        *self.high_mut() = self
            .high()
            .wrapping_sub(carry as _)
            .wrapping_sub(*rhs.high());
    }
}

impl std::ops::Shr<i32> for VarUint248 {
    type Output = Self;

    #[inline]
    fn shr(mut self, rhs: i32) -> Self::Output {
        self >>= rhs as usize;
        self
    }
}

impl std::ops::Shr<u32> for VarUint248 {
    type Output = Self;

    #[inline]
    fn shr(mut self, rhs: u32) -> Self::Output {
        self >>= rhs as usize;
        self
    }
}

impl std::ops::Shr<usize> for VarUint248 {
    type Output = Self;

    #[inline]
    fn shr(mut self, rhs: usize) -> Self::Output {
        self >>= rhs;
        self
    }
}

impl std::ops::ShrAssign<usize> for VarUint248 {
    fn shr_assign(&mut self, n: usize) {
        debug_assert!(n < 256, "shr overflow");

        match n {
            0 => {}
            1..=127 => {
                let hi = *self.high();
                *self.high_mut() >>= n;
                *self.low_mut() >>= n;
                *self.low_mut() |= hi << (128 - n);
            }
            _ => {
                let hi = *self.high();
                *self.high_mut() = 0;
                *self.low_mut() = hi >> (n & 127);
            }
        }
    }
}

impl std::ops::Shl<i32> for VarUint248 {
    type Output = Self;

    #[inline]
    fn shl(mut self, rhs: i32) -> Self::Output {
        self <<= rhs as usize;
        self
    }
}

impl std::ops::Shl<u32> for VarUint248 {
    type Output = Self;

    #[inline]
    fn shl(mut self, rhs: u32) -> Self::Output {
        self <<= rhs as usize;
        self
    }
}

impl std::ops::Shl<usize> for VarUint248 {
    type Output = Self;

    fn shl(mut self, rhs: usize) -> Self::Output {
        self <<= rhs;
        self
    }
}

impl std::ops::ShlAssign<usize> for VarUint248 {
    fn shl_assign(&mut self, n: usize) {
        debug_assert!(n < 256, "shl overflow");

        match n {
            0 => {}
            1..=127 => {
                let lo = *self.low();
                *self.low_mut() <<= n;
                *self.high_mut() <<= n;
                *self.high_mut() |= lo >> (128 - n);
            }
            _ => {
                let lo = *self.low();
                *self.low_mut() = 0;
                *self.high_mut() = lo << (n & 127);
            }
        }
    }
}

impl std::ops::Mul<u128> for VarUint248 {
    type Output = Self;

    #[inline]
    fn mul(mut self, rhs: u128) -> Self::Output {
        self *= rhs;
        self
    }
}

impl std::ops::Mul for VarUint248 {
    type Output = Self;

    #[inline]
    fn mul(mut self, rhs: Self) -> Self::Output {
        self *= rhs;
        self
    }
}

impl std::ops::Mul<&Self> for VarUint248 {
    type Output = Self;

    #[inline]
    fn mul(mut self, rhs: &Self) -> Self::Output {
        self *= rhs;
        self
    }
}

impl std::ops::MulAssign<u128> for VarUint248 {
    #[inline]
    fn mul_assign(&mut self, rhs: u128) {
        std::ops::MulAssign::<&Self>::mul_assign(self, &Self::new(rhs))
    }
}

impl std::ops::MulAssign for VarUint248 {
    #[inline]
    fn mul_assign(&mut self, rhs: Self) {
        std::ops::MulAssign::<&Self>::mul_assign(self, &rhs)
    }
}

impl std::ops::MulAssign<&Self> for VarUint248 {
    fn mul_assign(&mut self, rhs: &Self) {
        // See https://github.com/nlordell/ethnum-rs/blob/main/src/intrinsics/native/mul.rs

        let mut r = umulddi3(self.low(), rhs.low());
        let a_hi_mul_b_lo = self.high().wrapping_mul(*rhs.low());
        let a_lo_mul_b_hi = self.low().wrapping_mul(*rhs.high());
        *r.high_mut() = r
            .high()
            .wrapping_add(a_hi_mul_b_lo.wrapping_add(a_lo_mul_b_hi));

        *self = r;
    }
}

impl std::ops::Div<u128> for VarUint248 {
    type Output = Self;

    #[inline]
    fn div(mut self, rhs: u128) -> Self::Output {
        self /= VarUint248::new(rhs);
        self
    }
}

impl std::ops::Div for VarUint248 {
    type Output = Self;

    #[inline]
    fn div(mut self, rhs: Self) -> Self::Output {
        self /= rhs;
        self
    }
}

impl std::ops::Div<&Self> for VarUint248 {
    type Output = Self;

    #[inline]
    fn div(mut self, rhs: &Self) -> Self::Output {
        self /= rhs;
        self
    }
}

impl std::ops::DivAssign<u128> for VarUint248 {
    #[inline]
    fn div_assign(&mut self, rhs: u128) {
        std::ops::DivAssign::<&Self>::div_assign(self, &VarUint248::new(rhs))
    }
}

impl std::ops::DivAssign for VarUint248 {
    #[inline]
    fn div_assign(&mut self, rhs: Self) {
        std::ops::DivAssign::<&Self>::div_assign(self, &rhs)
    }
}

impl std::ops::DivAssign<&Self> for VarUint248 {
    fn div_assign(&mut self, rhs: &Self) {
        let (a, b) = (*self, rhs);

        // SAFETY: `udivmod` does not write `MaybeUninit::uninit()` to `res` and
        // `VarUint248` does not implement `Drop`.
        let res = unsafe { &mut *(self as *mut Self).cast() };
        udivmod(res, &a, b, None);
    }
}

impl std::ops::Rem<u128> for VarUint248 {
    type Output = Self;

    #[inline]
    fn rem(mut self, rhs: u128) -> Self::Output {
        self %= VarUint248::new(rhs);
        self
    }
}

impl std::ops::Rem for VarUint248 {
    type Output = Self;

    #[inline]
    fn rem(mut self, rhs: Self) -> Self::Output {
        self %= rhs;
        self
    }
}

impl std::ops::Rem<&Self> for VarUint248 {
    type Output = Self;

    #[inline]
    fn rem(mut self, rhs: &Self) -> Self::Output {
        self %= rhs;
        self
    }
}

impl std::ops::RemAssign<u128> for VarUint248 {
    fn rem_assign(&mut self, rhs: u128) {
        std::ops::RemAssign::<&Self>::rem_assign(self, &VarUint248::new(rhs))
    }
}

impl std::ops::RemAssign for VarUint248 {
    #[inline]
    fn rem_assign(&mut self, rhs: Self) {
        std::ops::RemAssign::<&Self>::rem_assign(self, &rhs)
    }
}

impl std::ops::RemAssign<&Self> for VarUint248 {
    fn rem_assign(&mut self, rhs: &Self) {
        let mut res = MaybeUninit::uninit();
        let (a, b) = (*self, rhs);

        // SAFETY: `udivmod` does not write `MaybeUninit::uninit()` to `rem` and
        // `VarUint248` does not implement `Drop`.
        let r = unsafe { &mut *(self as *mut Self).cast() };
        udivmod(&mut res, &a, b, Some(r));
    }
}

#[inline]
pub fn umulddi3(a: &u128, b: &u128) -> VarUint248 {
    const BITS_IN_DWORD_2: u32 = 64;
    const LOWER_MASK: u128 = u128::MAX >> BITS_IN_DWORD_2;

    let mut low = (a & LOWER_MASK) * (b & LOWER_MASK);
    let mut t = low >> BITS_IN_DWORD_2;
    low &= LOWER_MASK;
    t += (a >> BITS_IN_DWORD_2) * (b & LOWER_MASK);
    low += (t & LOWER_MASK) << BITS_IN_DWORD_2;
    let mut high = t >> BITS_IN_DWORD_2;
    t = low >> BITS_IN_DWORD_2;
    low &= LOWER_MASK;
    t += (b >> BITS_IN_DWORD_2) * (a & LOWER_MASK);
    low += (t & LOWER_MASK) << BITS_IN_DWORD_2;
    high += t >> BITS_IN_DWORD_2;
    high += (a >> BITS_IN_DWORD_2) * (b >> BITS_IN_DWORD_2);

    VarUint248::from_words(high, low)
}

fn udivmod(
    res: &mut MaybeUninit<VarUint248>,
    a: &VarUint248,
    b: &VarUint248,
    rem: Option<&mut MaybeUninit<VarUint248>>,
) {
    // See https://github.com/nlordell/ethnum-rs/blob/main/src/intrinsics/native/divmod.rs

    if a.high() | b.high() == 0 {
        if let Some(rem) = rem {
            rem.write(VarUint248::from_words(0, a.low() % b.low()));
        }
        res.write(VarUint248::from_words(0, a.low() / b.low()));
        return;
    }

    let dividend = *a;
    let divisor = *b;
    let quotient: VarUint248;
    let mut remainder: VarUint248;

    if divisor > dividend {
        if let Some(rem) = rem {
            rem.write(dividend);
        }
        res.write(VarUint248::ZERO);
        return;
    }
    // When the divisor fits in 128 bits, we can use an optimized path.
    if *divisor.high() == 0 {
        remainder = VarUint248::ZERO;
        if dividend.high() < divisor.low() {
            // The result fits in 128 bits.
            quotient = VarUint248::from_words(
                0,
                udiv256_by_128_to_128(
                    *dividend.high(),
                    *dividend.low(),
                    *divisor.low(),
                    remainder.low_mut(),
                ),
            );
        } else {
            // First, divide with the high part to get the remainder in dividend.s.high.
            // After that dividend.s.high < divisor.s.low.
            quotient = VarUint248::from_words(
                dividend.high() / divisor.low(),
                udiv256_by_128_to_128(
                    dividend.high() % divisor.low(),
                    *dividend.low(),
                    *divisor.low(),
                    remainder.low_mut(),
                ),
            );
        }
        if let Some(rem) = rem {
            rem.write(remainder);
        }
        res.write(quotient);
        return;
    }

    (quotient, remainder) = div_mod_knuth(&dividend, &divisor);

    if let Some(rem) = rem {
        rem.write(remainder);
    }
    res.write(quotient);
}

#[inline(always)]
fn udiv256_by_128_to_128(u1: u128, u0: u128, mut v: u128, r: &mut u128) -> u128 {
    // See https://github.com/nlordell/ethnum-rs/blob/main/src/intrinsics/native/divmod.rs

    const N_UDWORD_BITS: u32 = 128;
    const B: u128 = 1 << (N_UDWORD_BITS / 2); // Number base (128 bits)
    let (un1, un0): (u128, u128); // Norm. dividend LSD's
    let (vn1, vn0): (u128, u128); // Norm. divisor digits
    let (mut q1, mut q0): (u128, u128); // Quotient digits
    let (un128, un21, un10): (u128, u128, u128); // Dividend digit pairs

    let s = v.leading_zeros();
    if s > 0 {
        // Normalize the divisor.
        v <<= s;
        un128 = (u1 << s) | (u0 >> (N_UDWORD_BITS - s));
        un10 = u0 << s; // Shift dividend left
    } else {
        // Avoid undefined behavior of (u0 >> 64).
        un128 = u1;
        un10 = u0;
    }

    // Break divisor up into two 64-bit digits.
    vn1 = v >> (N_UDWORD_BITS / 2);
    vn0 = v & 0xFFFF_FFFF_FFFF_FFFF;

    // Break right half of dividend into two digits.
    un1 = un10 >> (N_UDWORD_BITS / 2);
    un0 = un10 & 0xFFFF_FFFF_FFFF_FFFF;

    // Compute the first quotient digit, q1.
    q1 = un128 / vn1;
    let mut rhat = un128 - q1 * vn1;

    // q1 has at most error 2. No more than 2 iterations.
    while q1 >= B || q1 * vn0 > B * rhat + un1 {
        q1 -= 1;
        rhat += vn1;
        if rhat >= B {
            break;
        }
    }

    un21 = un128
        .wrapping_mul(B)
        .wrapping_add(un1)
        .wrapping_sub(q1.wrapping_mul(v));

    // Compute the second quotient digit.
    q0 = un21 / vn1;
    rhat = un21 - q0 * vn1;

    // q0 has at most error 2. No more than 2 iterations.
    while q0 >= B || q0 * vn0 > B * rhat + un0 {
        q0 -= 1;
        rhat += vn1;
        if rhat >= B {
            break;
        }
    }

    *r = (un21
        .wrapping_mul(B)
        .wrapping_add(un0)
        .wrapping_sub(q0.wrapping_mul(v)))
        >> s;
    q1 * B + q0
}

#[inline]
pub fn div_mod_knuth(u: &VarUint248, v: &VarUint248) -> (VarUint248, VarUint248) {
    // See https://github.com/nlordell/ethnum-rs/blob/main/src/intrinsics/native/divmod.rs

    const N_UDWORD_BITS: u32 = 128;

    #[inline]
    fn full_shl(a: &VarUint248, shift: u32) -> [u128; 3] {
        debug_assert!(shift < N_UDWORD_BITS);
        let mut u = [0_u128; 3];
        let u_lo = a.low() << shift;
        let u_hi = *a >> (N_UDWORD_BITS - shift);
        u[0] = u_lo;
        u[1] = *u_hi.low();
        u[2] = *u_hi.high();

        u
    }

    #[inline]
    fn full_shr(u: &[u128; 3], shift: u32) -> VarUint248 {
        debug_assert!(shift < N_UDWORD_BITS);
        let mut res = VarUint248::ZERO;
        *res.low_mut() = u[0] >> shift;
        *res.high_mut() = u[1] >> shift;
        // carry
        if shift > 0 {
            let sh = N_UDWORD_BITS - shift;
            *res.low_mut() |= u[1] << sh;
            *res.high_mut() |= u[2] << sh;
        }

        res
    }

    // returns (lo, hi)
    #[inline]
    const fn split_u128_to_u128(a: u128) -> (u128, u128) {
        (a & 0xFFFFFFFFFFFFFFFF, a >> (N_UDWORD_BITS / 2))
    }

    // returns (lo, hi)
    #[inline]
    const fn fullmul_u128(a: u128, b: u128) -> (u128, u128) {
        let (a0, a1) = split_u128_to_u128(a);
        let (b0, b1) = split_u128_to_u128(b);

        let mut t = a0 * b0;
        let mut k: u128;
        let w3: u128;
        (w3, k) = split_u128_to_u128(t);

        t = a1 * b0 + k;
        let (w1, w2) = split_u128_to_u128(t);
        t = a0 * b1 + w1;
        k = t >> 64;

        let w_hi = a1 * b1 + w2 + k;
        let w_lo = (t << 64) + w3;

        (w_lo, w_hi)
    }

    #[inline]
    fn fullmul_u256_u128(a: &VarUint248, b: u128) -> [u128; 3] {
        let mut acc = [0_u128; 3];
        let mut lo: u128;
        let mut carry: u128;
        let c: bool;
        if b != 0 {
            (lo, carry) = fullmul_u128(*a.low(), b);
            acc[0] = lo;
            acc[1] = carry;
            (lo, carry) = fullmul_u128(*a.high(), b);
            (acc[1], c) = acc[1].overflowing_add(lo);
            acc[2] = carry + c as u128;
        }

        acc
    }

    #[inline]
    const fn add_carry(a: u128, b: u128, c: bool) -> (u128, bool) {
        let (res1, overflow1) = b.overflowing_add(c as u128);
        let (res2, overflow2) = u128::overflowing_add(a, res1);

        (res2, overflow1 || overflow2)
    }

    #[inline]
    const fn sub_carry(a: u128, b: u128, c: bool) -> (u128, bool) {
        let (res1, overflow1) = b.overflowing_add(c as u128);
        let (res2, overflow2) = u128::overflowing_sub(a, res1);

        (res2, overflow1 || overflow2)
    }

    // D1.
    // Make sure 128th bit in v's highest word is set.
    // If we shift both u and v, it won't affect the quotient
    // and the remainder will only need to be shifted back.
    let shift = v.high().leading_zeros();
    debug_assert!(shift < N_UDWORD_BITS);
    let v = *v << shift;
    debug_assert!(v.high() >> (N_UDWORD_BITS - 1) == 1);
    // u will store the remainder (shifted)
    let mut u = full_shl(u, shift);

    // quotient
    let mut q = VarUint248::ZERO;
    let v_n_1 = *v.high();
    let v_n_2 = *v.low();

    // D2. D7. - unrolled loop j == 0, n == 2, m == 0 (only one possible iteration)
    let mut r_hat: u128 = 0;
    let u_jn = u[2];

    // D3.
    // q_hat is our guess for the j-th quotient digit
    // q_hat = min(b - 1, (u_{j+n} * b + u_{j+n-1}) / v_{n-1})
    // b = 1 << WORD_BITS
    // Theorem B: q_hat >= q_j >= q_hat - 2
    let mut q_hat = if u_jn < v_n_1 {
        // let (mut q_hat, mut r_hat) = _div_mod_u128(u_jn, u[j + n - 1], v_n_1);
        let mut q_hat = udiv256_by_128_to_128(u_jn, u[1], v_n_1, &mut r_hat);
        let mut overflow: bool;
        // this loop takes at most 2 iterations
        loop {
            let another_iteration = {
                // check if q_hat * v_{n-2} > b * r_hat + u_{j+n-2}
                let (lo, hi) = fullmul_u128(q_hat, v_n_2);
                hi > r_hat || (hi == r_hat && lo > u[0])
            };
            if !another_iteration {
                break;
            }
            q_hat -= 1;
            (r_hat, overflow) = r_hat.overflowing_add(v_n_1);
            // if r_hat overflowed, we're done
            if overflow {
                break;
            }
        }
        q_hat
    } else {
        // here q_hat >= q_j >= q_hat - 1
        u128::MAX
    };

    // ex. 20:
    // since q_hat * v_{n-2} <= b * r_hat + u_{j+n-2},
    // either q_hat == q_j, or q_hat == q_j + 1

    // D4.
    // let's assume optimistically q_hat == q_j
    // subtract (q_hat * v) from u[j..]
    let q_hat_v = fullmul_u256_u128(&v, q_hat);
    // u[j..] -= q_hat_v;
    let mut c = false;
    (u[0], c) = sub_carry(u[0], q_hat_v[0], c);
    (u[1], c) = sub_carry(u[1], q_hat_v[1], c);
    (u[2], c) = sub_carry(u[2], q_hat_v[2], c);

    // D6.
    // actually, q_hat == q_j + 1 and u[j..] has overflowed
    // highly unlikely ~ (1 / 2^127)
    if c {
        q_hat -= 1;
        // add v to u[j..]
        c = false;
        (u[0], c) = add_carry(u[0], *v.low(), c);
        (u[1], c) = add_carry(u[1], *v.high(), c);
        u[2] = u[2].wrapping_add(c as u128);
    }

    // D5.
    *q.low_mut() = q_hat;

    // D8.
    let remainder = full_shr(&u, shift);

    (q, remainder)
}

impl std::fmt::Display for VarUint248 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt_var_uint248(*self, f)
    }
}

const DEC_DIGITS_LUT: &[u8; 200] = b"\
    0001020304050607080910111213141516171819\
    2021222324252627282930313233343536373839\
    4041424344454647484950515253545556575859\
    6061626364656667686970717273747576777879\
    8081828384858687888990919293949596979899";

fn fmt_var_uint248(mut n: VarUint248, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    // See https://github.com/nlordell/ethnum-rs/blob/main/src/fmt.rs

    // 2^256 is about 1*10^78, so 79 gives an extra byte of space
    let mut buf = [MaybeUninit::<u8>::uninit(); 79];
    let mut curr = buf.len() as isize;
    let buf_ptr = &mut buf[0] as *mut _ as *mut u8;
    let lut_ptr = DEC_DIGITS_LUT.as_ptr();

    // SAFETY: Since `d1` and `d2` are always less than or equal to `198`, we
    // can copy from `lut_ptr[d1..d1 + 1]` and `lut_ptr[d2..d2 + 1]`. To show
    // that it's OK to copy into `buf_ptr`, notice that at the beginning
    // `curr == buf.len() == 39 > log(n)` since `n < 2^128 < 10^39`, and at
    // each step this is kept the same as `n` is divided. Since `n` is always
    // non-negative, this means that `curr > 0` so `buf_ptr[curr..curr + 1]`
    // is safe to access.
    unsafe {
        // eagerly decode 4 characters at a time
        while n >= 10000 {
            let rem = (n % 10000).as_isize();
            n /= 10000;

            let d1 = (rem / 100) << 1;
            let d2 = (rem % 100) << 1;
            curr -= 4;

            // We are allowed to copy to `buf_ptr[curr..curr + 3]` here since
            // otherwise `curr < 0`. But then `n` was originally at least `10000^10`
            // which is `10^40 > 2^128 > n`.
            std::ptr::copy_nonoverlapping(lut_ptr.offset(d1), buf_ptr.offset(curr), 2);
            std::ptr::copy_nonoverlapping(lut_ptr.offset(d2), buf_ptr.offset(curr + 2), 2);
        }

        // if we reach here numbers are <= 9999, so at most 4 chars long
        let mut n = n.as_isize(); // possibly reduce 64bit math

        // decode 2 more chars, if > 2 chars
        if n >= 100 {
            let d1 = (n % 100) << 1;
            n /= 100;
            curr -= 2;
            std::ptr::copy_nonoverlapping(lut_ptr.offset(d1), buf_ptr.offset(curr), 2);
        }

        // decode last 1 or 2 chars
        if n < 10 {
            curr -= 1;
            *buf_ptr.offset(curr) = (n as u8) + b'0';
        } else {
            let d1 = n << 1;
            curr -= 2;
            std::ptr::copy_nonoverlapping(lut_ptr.offset(d1), buf_ptr.offset(curr), 2);
        }
    }

    // SAFETY: `curr` > 0 (since we made `buf` large enough), and all the chars are valid
    // UTF-8 since `DEC_DIGITS_LUT` is
    let buf_slice = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(
            buf_ptr.offset(curr),
            buf.len() - curr as usize,
        ))
    };
    f.pad_integral(true, "", buf_slice)
}

impl FromStr for VarUint248 {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        from_str_radix(s, 10, None)
    }
}

fn from_str_radix(
    src: &str,
    radix: u32,
    prefix: Option<&str>,
) -> Result<VarUint248, std::num::ParseIntError> {
    use std::num::IntErrorKind::*;

    // See https://github.com/nlordell/ethnum-rs/blob/main/src/parse.rs

    const fn pie(kind: std::num::IntErrorKind) -> std::num::ParseIntError {
        unsafe { std::mem::transmute(kind) }
    }

    assert!(
        (2..=36).contains(&radix),
        "from_str_radix_int: must lie in the range `[2, 36]` - found {radix}",
    );

    if src.is_empty() {
        return Err(pie(Empty));
    }

    // all valid digits are ascii, so we will just iterate over the utf8 bytes
    // and cast them to chars. .to_digit() will safely return None for anything
    // other than a valid ascii digit for the given radix, including the first-byte
    // of multi-byte sequences
    let src = src.as_bytes();

    let prefixed_digits = match src[0] {
        b'+' | b'-' if src[1..].is_empty() => {
            return Err(pie(InvalidDigit));
        }
        b'+' => &src[1..],
        _ => src,
    };

    let digits = match prefix {
        Some(prefix) => prefixed_digits
            .strip_prefix(prefix.as_bytes())
            .ok_or(pie(InvalidDigit))?,
        None => prefixed_digits,
    };
    if digits.is_empty() {
        return Err(pie(InvalidDigit));
    }

    let mut result = VarUint248::ZERO;

    if radix <= 16 && digits.len() <= 64 {
        for &c in digits {
            result *= radix as u128;
            let Some(x) = (c as char).to_digit(radix) else {
                return Err(pie(InvalidDigit));
            };
            result += x as u128;
        }
    } else {
        for &c in digits {
            match result.checked_mul(&VarUint248::from(radix)) {
                Some(mul) => result = mul,
                None => return Err(pie(PosOverflow)),
            }

            let Some(x) = (c as char).to_digit(radix) else {
                return Err(pie(InvalidDigit));
            };

            match result.checked_add(&VarUint248::from(x)) {
                Some(add) => result = add,
                None => return Err(pie(PosOverflow)),
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_range_is_correct() {
        assert!(VarUint248::ZERO.is_valid());
        assert!(VarUint248::ONE.is_valid());
        assert!(VarUint248::new(123123).is_valid());
        assert!(VarUint248::new(u128::MAX).is_valid());
        // ...
        assert!(VarUint248::from_words(u128::MAX >> 8, u128::MAX).is_valid());

        assert!(!VarUint248::from_words(u128::MAX >> 7, u128::MAX).is_valid());
        assert!(!VarUint248::from_words(u128::MAX, u128::MAX).is_valid());

        assert_eq!(
            VarUint248::new(123).saturating_sub(&VarUint248::new(321)),
            VarUint248::ZERO
        );

        assert_eq!(
            VarUint248::from_words(u128::MAX, u128::MAX).saturating_sub(&VarUint248::new(1)),
            VarUint248::MAX
        );
        assert_eq!(
            VarUint248::from_words(u128::MAX, u128::MAX)
                .saturating_sub(&VarUint248::from_words(u128::MAX, u128::MAX)),
            VarUint248::ZERO
        );
    }

    #[test]
    fn store_into_cell() {
        for i in 0..128 {
            let lo = 1u128 << i;

            let value = VarUint248::new(lo);
            let cell = CellBuilder::build_from(value).unwrap();
            assert_eq!(value.bit_len().unwrap(), cell.bit_len());
        }
    }

    #[test]
    fn load_from_cell() {
        let mut lo: u128 = 0xababcdef89abcdefdeadbeeffafacafe;
        for _ in 0..=128 {
            let value = VarUint248::new(lo);

            let cell = CellBuilder::build_from(value).unwrap();

            let parsed_value = cell.parse::<VarUint248>().unwrap();
            assert_eq!(parsed_value, value);

            lo >>= 1;
        }
    }

    #[test]
    fn identity() {
        assert_eq!(VarUint248::ZERO + VarUint248::ZERO, VarUint248::ZERO);
        assert_eq!(VarUint248::ONE * VarUint248::ZERO, VarUint248::ZERO);
        assert_eq!(VarUint248::ONE * VarUint248::ONE, VarUint248::ONE);
    }

    #[test]
    fn cmp() {
        // 1e38
        let x = VarUint248::from_words(0, 100000000000000000000000000000000000000);
        // 1e48
        let y = VarUint248::from_words(2938735877, 18960114910927365649471927446130393088);
        assert!(x < y);
        assert_eq!(x.cmp(&y), std::cmp::Ordering::Less);
        assert!(y > x);
        assert_eq!(y.cmp(&x), std::cmp::Ordering::Greater);

        let x = VarUint248::new(100);
        let y = VarUint248::new(100);
        assert!(x <= y);
        assert_eq!(x.cmp(&y), std::cmp::Ordering::Equal);
    }

    #[test]
    fn mul() {
        assert_eq!(VarUint248::new(6) * 7, VarUint248::new(42));
        assert_eq!(
            VarUint248::new(6).checked_mul(&VarUint248::new(7)),
            Some(VarUint248::new(42))
        );

        assert_eq!(
            VarUint248::MAX.checked_mul(&VarUint248::ONE),
            Some(VarUint248::MAX)
        );
        assert_eq!(
            VarUint248::MAX.checked_mul(&VarUint248::ZERO),
            Some(VarUint248::ZERO)
        );

        assert_eq!(
            VarUint248::new(u128::MAX) * u128::MAX,
            VarUint248::from_words(!0 << 1, 1),
        );
        assert_eq!(
            VarUint248::new(u128::MAX).checked_mul(&VarUint248::new(u128::MAX)),
            Some(VarUint248::from_words(!0 << 1, 1))
        );

        assert_eq!(VarUint248::MAX.checked_mul(&VarUint248::new(257)), None);
        assert_eq!(VarUint248::MAX.checked_mul(&VarUint248::MAX), None);
    }

    #[test]
    fn division() {
        // 0 X
        // ---
        // 0 X
        assert_eq!(VarUint248::new(100) / 9, 11);

        // 0 X
        // ---
        // K X
        assert_eq!(VarUint248::new(!0u128) / (VarUint248::ONE << 128), 0u128);

        // K 0
        // ---
        // K 0
        assert_eq!(
            VarUint248::from_words(100, 0) / VarUint248::from_words(10, 0),
            10
        );

        // K K
        // ---
        // K 0
        assert_eq!(
            VarUint248::from_words(100, 1337) / (VarUint248::ONE << 130u32),
            25
        );
        assert_eq!(
            VarUint248::from_words(1337, !0) / VarUint248::from_words(63, 0),
            21
        );

        // K X
        // ---
        // 0 K
        assert_eq!(
            VarUint248::from_words(42, 0) / VarUint248::ONE,
            VarUint248::from_words(42, 0),
        );
        assert_eq!(
            VarUint248::from_words(42, 42) / (VarUint248::ONE << 42),
            42u128 << (128 - 42),
        );
        assert_eq!(
            VarUint248::from_words(1337, !0) / 0xc0ffee,
            35996389033280467545299711090127855,
        );
        assert_eq!(
            VarUint248::from_words(42, 0) / 99,
            144362216269489045105674075880144089708,
        );

        // K X
        // ---
        // K K
        assert_eq!(
            VarUint248::from_words(100, 100) / VarUint248::from_words(1000, 1000),
            0,
        );
        assert_eq!(
            VarUint248::from_words(1337, !0) / VarUint248::from_words(43, !0),
            30,
        );
    }

    #[test]
    #[should_panic]
    fn division_by_zero() {
        _ = VarUint248::ONE / 0;
    }

    #[test]
    fn remainder() {
        // 0 X
        // ---
        // 0 X
        assert_eq!(VarUint248::new(100) % 9, 1);

        // 0 X
        // ---
        // K X
        assert_eq!(
            VarUint248::new(!0u128) % (VarUint248::ONE << 128u32),
            !0u128
        );

        // K 0
        // ---
        // K 0
        assert_eq!(
            VarUint248::from_words(100, 0) % VarUint248::from_words(10, 0),
            0
        );

        // K K
        // ---
        // K 0
        assert_eq!(
            VarUint248::from_words(100, 1337) % (VarUint248::ONE << 130u32),
            1337
        );
        assert_eq!(
            VarUint248::from_words(1337, !0) % VarUint248::from_words(63, 0),
            VarUint248::from_words(14, !0),
        );

        // K X
        // ---
        // 0 K
        assert_eq!(VarUint248::from_words(42, 0) % VarUint248::ONE, 0);
        assert_eq!(
            VarUint248::from_words(42, 42) % (VarUint248::ONE << 42),
            42u128
        );
        assert_eq!(VarUint248::from_words(1337, !0) % 0xc0ffee, 1910477);
        assert_eq!(VarUint248::from_words(42, 0) % 99, 60);

        // K X
        // ---
        // K K
        assert_eq!(
            VarUint248::from_words(100, 100) % VarUint248::from_words(1000, 1000),
            VarUint248::from_words(100, 100),
        );
        assert_eq!(
            VarUint248::from_words(1337, !0) % VarUint248::from_words(43, !0),
            VarUint248::from_words(18, 29),
        );
    }

    #[test]
    #[should_panic]
    fn remainder_by_zero() {
        _ = VarUint248::ONE % 0;
    }
}
