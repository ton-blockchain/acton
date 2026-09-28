//! General stuff.

use std::mem::MaybeUninit;

use crate::error::Error;

/// Extension trait for [`BigInt`] or [`BigUint`].
///
/// [`BigInt`]: num_bigint::BigInt
/// [`BigUint`]: num_bigint::BigUint
#[cfg(feature = "bigint")]
pub trait BigIntExt {
    /// Determines the fewest bits necessary to serialize self
    /// as an optionally signed integer.
    fn bitsize(&self, signed: bool) -> u16;

    /// Returns `true` if this number sign aligns with the `signed`.
    fn has_correct_sign(&self, signed: bool) -> bool;
}

#[cfg(feature = "bigint")]
impl BigIntExt for num_bigint::BigInt {
    fn bitsize(&self, signed: bool) -> u16 {
        let mut bits = self.bits() as u16;
        if signed {
            match self.sign() {
                num_bigint::Sign::NoSign => bits,
                num_bigint::Sign::Plus => bits + 1,
                num_bigint::Sign::Minus => {
                    // Check if `int` magnitude is not a power of 2
                    let mut digits = self.iter_u64_digits().rev();
                    if let Some(hi) = digits.next()
                        && (!hi.is_power_of_two() || !digits.all(|digit| digit == 0))
                    {
                        bits += 1;
                    }
                    bits
                }
            }
        } else {
            bits
        }
    }

    fn has_correct_sign(&self, signed: bool) -> bool {
        signed || self.sign() != num_bigint::Sign::Minus
    }
}

#[cfg(feature = "bigint")]
impl BigIntExt for num_bigint::BigUint {
    fn bitsize(&self, signed: bool) -> u16 {
        let bits = self.bits() as u16;
        bits + (signed && bits != 0) as u16
    }

    fn has_correct_sign(&self, _: bool) -> bool {
        true
    }
}

/// Brings [unlikely](core::intrinsics::unlikely) to stable rust.
#[inline(always)]
pub(crate) const fn unlikely(b: bool) -> bool {
    #[allow(clippy::needless_bool, clippy::bool_to_int_with_if)]
    if (1i32).checked_div(if b { 0 } else { 1 }).is_none() {
        true
    } else {
        false
    }
}

/// Reads n-byte integer as `u32` from the bytes pointer.
///
/// # Safety
///
/// The following must be true:
/// - size must be in range 1..=4.
/// - data must be at least `size` bytes long.
pub(crate) unsafe fn read_be_u32_fast(data_ptr: *const u8, size: usize) -> u32 {
    unsafe {
        match size {
            1 => *data_ptr as u32,
            2 => u16::from_be_bytes(*(data_ptr as *const [u8; 2])) as u32,
            3 => {
                let mut bytes = [0u8; 4];
                std::ptr::copy_nonoverlapping(data_ptr, bytes.as_mut_ptr().add(1), 3);
                u32::from_be_bytes(bytes)
            }
            4 => u32::from_be_bytes(*(data_ptr as *const [u8; 4])),
            _ => std::hint::unreachable_unchecked(),
        }
    }
}

/// Reads n-byte integer as `u64` from the bytes pointer.
///
/// # Safety
///
/// The following must be true:
/// - size must be in range 1..=8.
/// - data must be at least `size` bytes long.
pub(crate) unsafe fn read_be_u64_fast(data_ptr: *const u8, size: usize) -> u64 {
    unsafe {
        match size {
            1..=4 => read_be_u32_fast(data_ptr, size) as u64,
            5..=8 => {
                let mut bytes = [0u8; 8];
                std::ptr::copy_nonoverlapping(data_ptr, bytes.as_mut_ptr().add(8 - size), size);
                u64::from_be_bytes(bytes)
            }
            _ => std::hint::unreachable_unchecked(),
        }
    }
}

#[cfg(any(feature = "base64", test))]
#[inline]
pub(crate) fn encode_base64<T: AsRef<[u8]>>(data: T) -> String {
    use base64::Engine;
    fn encode_base64_impl(data: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(data)
    }
    encode_base64_impl(data.as_ref())
}

#[cfg(any(feature = "base64", test))]
#[inline]
pub(crate) fn decode_base64<T: AsRef<[u8]>>(data: T) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::Engine;
    fn decode_base64_impl(data: &[u8]) -> Result<Vec<u8>, base64::DecodeError> {
        base64::engine::general_purpose::STANDARD.decode(data)
    }
    decode_base64_impl(data.as_ref())
}

#[cfg(any(feature = "base64", test))]
#[allow(unused)]
#[inline]
pub(crate) fn decode_base64_slice<T: AsRef<[u8]>>(
    data: T,
    target: &mut [u8],
) -> Result<(), base64::DecodeSliceError> {
    use base64::Engine;
    fn decode_base64_slice_impl(
        data: &[u8],
        target: &mut [u8],
    ) -> Result<(), base64::DecodeSliceError> {
        base64::engine::general_purpose::STANDARD
            .decode_slice(data, target)
            .map(|_| ())
    }
    decode_base64_slice_impl(data.as_ref(), target)
}

/// Small on-stack vector of max length N.
pub struct ArrayVec<T, const N: usize> {
    inner: [MaybeUninit<T>; N],
    len: u8,
}

impl<T, const N: usize> ArrayVec<T, N> {
    /// Ensure that provided length is small enough.
    const _ASSERT_LEN: () = assert!(N <= u8::MAX as usize);

    /// Returns an empty vector.
    pub const fn new() -> Self {
        Self {
            // SAFETY: An uninitialized `[MaybeUninit<_>; LEN]` is valid.
            inner: unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() },
            len: 0,
        }
    }

    /// Returns the number of elements in the vector, also referred to as its ‘length’.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns true if the vector contains no elements.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Appends an element to the back of a collection.
    ///
    /// # Safety
    ///
    /// The following must be true:
    /// - The length of this vector is less than `N`.
    #[inline]
    pub unsafe fn push(&mut self, item: T) {
        unsafe {
            debug_assert!((self.len as usize) < N);

            *self.inner.get_unchecked_mut(self.len as usize) = MaybeUninit::new(item);
            self.len += 1;
        }
    }

    /// Returns a reference to an element.
    pub const fn get(&self, n: u8) -> Option<&T> {
        if n < self.len {
            let references = self.inner.as_ptr() as *const T;
            // SAFETY: {len} elements were initialized, n < len
            Some(unsafe { &*references.add(n as usize) })
        } else {
            None
        }
    }

    /// Returns the inner data without dropping its elements.
    ///
    /// # Safety
    ///
    /// The caller is responsible for calling the destructor for
    /// `len` initialized items in the returned array.
    #[inline]
    pub unsafe fn into_inner(self) -> [MaybeUninit<T>; N] {
        unsafe {
            let this = std::mem::ManuallyDrop::new(self);
            std::ptr::read(&this.inner)
        }
    }
}

impl<T, const N: usize> Default for ArrayVec<T, N> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<R, const N: usize> AsRef<[R]> for ArrayVec<R, N> {
    #[inline]
    fn as_ref(&self) -> &[R] {
        // SAFETY: {len} elements were initialized
        unsafe { std::slice::from_raw_parts(self.inner.as_ptr() as *const R, self.len as usize) }
    }
}

impl<T: Clone, const N: usize> Clone for ArrayVec<T, N> {
    fn clone(&self) -> Self {
        let mut res = Self::default();
        for item in self.as_ref() {
            // SAFETY: {len} elements were initialized
            unsafe { res.push(item.clone()) };
        }
        res
    }
}

impl<T, const N: usize> Drop for ArrayVec<T, N> {
    fn drop(&mut self) {
        debug_assert!(self.len as usize <= N);

        let references_ptr = self.inner.as_mut_ptr() as *mut T;
        for i in 0..self.len {
            // SAFETY: len items were initialized
            unsafe { std::ptr::drop_in_place(references_ptr.add(i as usize)) };
        }
    }
}

impl<T, const N: usize> IntoIterator for ArrayVec<T, N> {
    type Item = T;
    type IntoIter = ArrayVecIntoIter<T, N>;

    fn into_iter(self) -> Self::IntoIter {
        let this = std::mem::ManuallyDrop::new(self);
        ArrayVecIntoIter {
            // SAFETY: inner still exists.
            inner: unsafe { std::ptr::read(&this.inner) },
            offset: 0,
            len: this.len as usize,
        }
    }
}

/// An [`IntoIterator`] wrapper for an [`ArrayVec`].
pub struct ArrayVecIntoIter<T, const N: usize> {
    inner: [MaybeUninit<T>; N],
    offset: usize,
    len: usize,
}

impl<T, const N: usize> Iterator for ArrayVecIntoIter<T, N> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.len {
            return None;
        }

        // SAFETY: len items were initialized.
        let item = unsafe { self.inner.get_unchecked(self.offset).assume_init_read() };
        self.offset += 1;

        Some(item)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len - self.offset;
        (len, Some(len))
    }
}

impl<T, const N: usize> Drop for ArrayVecIntoIter<T, N> {
    fn drop(&mut self) {
        debug_assert!(self.offset <= self.len && self.len <= N);

        let references_ptr = self.inner.as_mut_ptr() as *mut T;
        for i in self.offset..self.len {
            // SAFETY: len items were initialized
            unsafe { std::ptr::drop_in_place(references_ptr.add(i)) };
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum IterStatus {
    /// Iterator is still valid.
    Valid,
    /// Iterator started with a pruned branch cell.
    UnexpectedCell,
    /// [`RawDict`] has invalid structure.
    Broken,
}

impl IterStatus {
    #[inline]
    pub(crate) const fn is_valid(self) -> bool {
        matches!(self, Self::Valid)
    }

    #[inline]
    pub(crate) const fn is_unexpected_cell(self) -> bool {
        matches!(self, Self::UnexpectedCell)
    }
}

/// Used to get a mutable reference of the inner type if possible.
pub trait TryAsMut<T: ?Sized> {
    /// Tries to convert this type into a mutable reference of the (usually inferred) input type.
    fn try_as_mut(&mut self) -> Option<&mut T>;
}

/// A wrapper around arbitrary data with the specified bit length.
pub struct Bitstring<'a> {
    /// Underlying bytes (with or without termination bit).
    pub bytes: &'a [u8],
    /// Length of data in bits.
    pub bit_len: u16,
}

impl Bitstring<'_> {
    /// Parses a bitstring from a hex string.
    ///
    /// Returns the parsed data and the bit length.
    /// Tag bit is removed if present.
    pub fn from_hex_str(s: &str) -> Result<(Vec<u8>, u16), Error> {
        fn hex_char(c: u8) -> Result<u8, Error> {
            match c {
                b'A'..=b'F' => Ok(c - b'A' + 10),
                b'a'..=b'f' => Ok(c - b'a' + 10),
                b'0'..=b'9' => Ok(c - b'0'),
                _ => Err(Error::InvalidData),
            }
        }

        if !s.is_ascii() || s.len() > 128 * 2 {
            return Err(Error::InvalidData);
        }

        let s = s.as_bytes();
        let (mut s, with_tag) = match s.strip_suffix(b"_") {
            Some(s) => (s, true),
            None => (s, false),
        };

        let mut half_byte = None;
        if s.len() % 2 != 0
            && let Some((last, prefix)) = s.split_last()
        {
            half_byte = Some(ok!(hex_char(*last)));
            s = prefix;
        }

        let Ok(mut data) = hex::decode(s) else {
            return Err(Error::InvalidData);
        };

        let mut bit_len = data.len() as u16 * 8;
        if let Some(half_byte) = half_byte {
            bit_len += 4;
            data.push(half_byte << 4);
        }

        if with_tag {
            bit_len = data.len() as u16 * 8;
            for byte in data.iter_mut().rev() {
                if *byte == 0 {
                    bit_len -= 8;
                } else {
                    let trailing = byte.trailing_zeros();
                    bit_len -= 1 + trailing as u16;

                    // NOTE: `trailing` is in range 0..=7,
                    // so we must split the shift in two parts.
                    *byte &= (0xff << trailing) << 1;
                    break;
                }
            }

            data.truncate(bit_len.div_ceil(8) as usize);
        }

        Ok((data, bit_len))
    }

    fn fmt_hex<const UPPER: bool>(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const CHUNK_LEN: usize = 16;

        let bit_len = std::cmp::min(self.bit_len as usize, self.bytes.len() * 8) as u16;
        let byte_len = bit_len.div_ceil(8) as usize;
        let bytes = &self.bytes[..byte_len];

        let rem = bit_len % 8;
        let (bytes, last_byte) = match bytes.split_last() {
            Some((last_byte, bytes)) if rem != 0 => {
                let tag_mask: u8 = 1 << (7 - rem);
                let data_mask = !(tag_mask - 1);
                let last_byte = (*last_byte & data_mask) | tag_mask;
                (bytes, Some(last_byte))
            }
            _ => (bytes, None),
        };

        let mut chunk = [0u8; CHUNK_LEN * 2];
        for data in bytes.chunks(CHUNK_LEN) {
            let chunk = &mut chunk[..data.len() * 2];

            if UPPER {
                encode_to_hex_slice(data, chunk, HEX_CHARS_UPPER).unwrap();
            } else {
                encode_to_hex_slice(data, chunk, HEX_CHARS_LOWER).unwrap();
            }

            // SAFETY: result was constructed from valid ascii `HEX_CHARS_LOWER`
            ok!(f.write_str(unsafe { std::str::from_utf8_unchecked(chunk) }));
        }

        if let Some(mut last_byte) = last_byte {
            let tag = if rem != 4 { "_" } else { "" };
            let rem = 1 + (rem > 4) as usize;
            if rem == 1 {
                last_byte >>= 4;
            }

            if UPPER {
                ok!(write!(f, "{last_byte:0rem$X}{tag}"));
            } else {
                ok!(write!(f, "{last_byte:0rem$x}{tag}"));
            }
        }

        Ok(())
    }
}

impl std::fmt::Display for Bitstring<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(self, f)
    }
}

impl std::fmt::LowerHex for Bitstring<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Self::fmt_hex::<false>(self, f)
    }
}

impl std::fmt::UpperHex for Bitstring<'_> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Self::fmt_hex::<true>(self, f)
    }
}

impl std::fmt::Binary for Bitstring<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bit_len = std::cmp::min(self.bit_len as usize, self.bytes.len() * 8) as u16;
        let byte_len = bit_len.div_ceil(8) as usize;
        let bytes = &self.bytes[..byte_len];

        let rem = (bit_len % 8) as usize;
        let (bytes, last_byte) = match bytes.split_last() {
            Some((last_byte, bytes)) if rem != 0 => (bytes, Some(*last_byte)),
            _ => (bytes, None),
        };

        for byte in bytes {
            ok!(write!(f, "{byte:08b}"));
        }

        if let Some(mut last_byte) = last_byte {
            last_byte >>= 8 - rem;
            ok!(write!(f, "{last_byte:0rem$b}"))
        }

        Ok(())
    }
}

pub(crate) fn encode_to_hex_slice(
    input: &[u8],
    output: &mut [u8],
    table: &[u8; 16],
) -> Result<(), hex::FromHexError> {
    if input.len() * 2 != output.len() {
        return Err(hex::FromHexError::InvalidStringLength);
    }

    for (byte, output) in input.iter().zip(output.chunks_exact_mut(2)) {
        let (high, low) = byte2hex(*byte, table);
        output[0] = high;
        output[1] = low;
    }

    Ok(())
}

#[inline]
#[must_use]
fn byte2hex(byte: u8, table: &[u8; 16]) -> (u8, u8) {
    let high = table[((byte & 0xf0) >> 4) as usize];
    let low = table[(byte & 0x0f) as usize];

    (high, low)
}

pub(crate) const HEX_CHARS_LOWER: &[u8; 16] = b"0123456789abcdef";
pub(crate) const HEX_CHARS_UPPER: &[u8; 16] = b"0123456789ABCDEF";

#[allow(unused)]
pub(crate) fn debug_tuple_field1_finish(
    f: &mut std::fmt::Formatter<'_>,
    name: &str,
    value1: &dyn std::fmt::Debug,
) -> std::fmt::Result {
    let mut builder = std::fmt::Formatter::debug_tuple(f, name);
    builder.field(value1);
    builder.finish()
}

pub(crate) fn debug_struct_field1_finish(
    f: &mut std::fmt::Formatter<'_>,
    name: &str,
    name1: &str,
    value1: &dyn std::fmt::Debug,
) -> std::fmt::Result {
    let mut builder = std::fmt::Formatter::debug_struct(f, name);
    builder.field(name1, value1);
    builder.finish()
}

pub(crate) fn debug_struct_field2_finish(
    f: &mut std::fmt::Formatter<'_>,
    name: &str,
    name1: &str,
    value1: &dyn std::fmt::Debug,
    name2: &str,
    value2: &dyn std::fmt::Debug,
) -> std::fmt::Result {
    let mut builder = std::fmt::Formatter::debug_struct(f, name);
    builder.field(name1, value1);
    builder.field(name2, value2);
    builder.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bitstring_from_hex_str() {
        let (data, bit_len) = Bitstring::from_hex_str("").unwrap();
        assert_eq!(bit_len, 0);
        assert!(data.is_empty());

        let (data, bit_len) = Bitstring::from_hex_str("8_").unwrap();
        assert_eq!(bit_len, 0);
        assert!(data.is_empty());

        let (data, bit_len) = Bitstring::from_hex_str("ded_").unwrap();
        assert_eq!(bit_len, 11);
        assert_eq!(data, vec![0xde, 0xc0]);

        let (data, bit_len) = Bitstring::from_hex_str("b00b1e5").unwrap();
        assert_eq!(bit_len, 28);
        assert_eq!(data, vec![0xb0, 0x0b, 0x1e, 0x50]);

        let (data, bit_len) = Bitstring::from_hex_str("b00b1e5_").unwrap();
        assert_eq!(bit_len, 27);
        assert_eq!(data, vec![0xb0, 0x0b, 0x1e, 0x40]);
    }

    #[test]
    fn bitstring_zero_char_with_completion_tag() {
        assert_eq!(
            format!("{}", Bitstring {
                bytes: &[0b_0011_0000],
                bit_len: 4
            }),
            format!("{:x}", 0b_0011)
        );
        assert_eq!(
            format!("{}", Bitstring {
                bytes: &[0b_0100_0000],
                bit_len: 2
            }),
            format!("{:x}_", 0b_0110)
        );
        assert_eq!(
            format!("{}", Bitstring {
                bytes: &[0b_0000_1000],
                bit_len: 5
            }),
            format!("{:x}{:x}_", 0b_0000, 0b_1100)
        );
        assert_eq!(
            format!("{}", Bitstring {
                bytes: &[0b_0000_1000, 0b_0100_0000],
                bit_len: 8 + 2
            }),
            format!("{:x}{:x}{:x}_", 0b_0000, 0b_1000, 0b_0110)
        );
        assert_eq!(
            format!("{}", Bitstring {
                bytes: &[0b_0100_0000, 0b_0000_1000],
                bit_len: 8 + 5
            }),
            format!("{:x}{:x}{:x}{:x}_", 0b_0100, 0b_0000, 0b_0000, 0b_1100)
        );
    }
}
