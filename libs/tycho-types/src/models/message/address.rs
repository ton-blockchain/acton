use std::str::FromStr;

use crate::cell::*;
use crate::error::{Error, ParseAddrError};
use crate::models::block::ShardIdent;
use crate::models::message::{load_ext_addr, store_ext_addr};
use crate::num::*;
use crate::util::*;

/// Basic internal address trait.
pub trait Addr {
    /// Returns the workchain part of the address.
    fn workchain(&self) -> i32;
    /// Returns the high bits of the address as a number.
    fn prefix(&self) -> u64;
}

/// Any possible address of specification
#[derive(Default, Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum AnyAddr {
    /// Empty address. 2 zero bits
    #[default]
    None,
    /// External address
    Ext(ExtAddr),
    /// Standard internal address
    Std(StdAddr),
    /// Variable-length internal address
    Var(VarAddr),
}

impl AnyAddr {
    /// Returns the number of data bits that this struct occupies.
    pub fn bit_len(&self) -> u16 {
        match self {
            AnyAddr::None => 2, // 2 bits of zeroes
            AnyAddr::Ext(addr) => addr.bit_len(),
            AnyAddr::Std(addr) => addr.bit_len(),
            AnyAddr::Var(addr) => addr.bit_len(),
        }
    }
}

impl Store for AnyAddr {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        match self {
            Self::None => store_ext_addr(builder, context, &None),
            Self::Std(addr) => addr.store_into(builder, context),
            Self::Var(addr) => addr.store_into(builder, context),
            Self::Ext(addr) => store_ext_addr(builder, context, &Some(addr.clone())),
        }
    }
}

impl<'a> Load<'a> for AnyAddr {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let addr = if ok!(slice.get_bit(0)) {
            let addr = ok!(IntAddr::load_from(slice));
            match addr {
                IntAddr::Std(addr) => Self::Std(addr),
                IntAddr::Var(addr) => Self::Var(addr),
            }
        } else {
            match ok!(load_ext_addr(slice)) {
                Some(addr) => Self::Ext(addr),
                None => Self::None,
            }
        };

        Ok(addr)
    }
}

// for all
impl From<Option<ExtAddr>> for AnyAddr {
    fn from(value: Option<ExtAddr>) -> Self {
        if let Some(addr) = value {
            AnyAddr::Ext(addr)
        } else {
            AnyAddr::None
        }
    }
}

impl From<Option<IntAddr>> for AnyAddr {
    fn from(value: Option<IntAddr>) -> Self {
        match value {
            Some(IntAddr::Std(addr)) => AnyAddr::Std(addr),
            Some(IntAddr::Var(addr)) => AnyAddr::Var(addr),
            None => AnyAddr::None,
        }
    }
}

impl From<IntAddr> for AnyAddr {
    fn from(value: IntAddr) -> Self {
        match value {
            IntAddr::Std(addr) => AnyAddr::Std(addr),
            IntAddr::Var(addr) => AnyAddr::Var(addr),
        }
    }
}

impl From<ExtAddr> for AnyAddr {
    fn from(value: ExtAddr) -> Self {
        AnyAddr::Ext(value)
    }
}

impl From<StdAddr> for AnyAddr {
    fn from(value: StdAddr) -> Self {
        AnyAddr::Std(value)
    }
}

impl From<VarAddr> for AnyAddr {
    fn from(value: VarAddr) -> Self {
        AnyAddr::Var(value)
    }
}

/// Internal message address.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub enum IntAddr {
    /// Standard internal address.
    Std(StdAddr),
    /// Variable-length internal address.
    Var(VarAddr),
}

impl Default for IntAddr {
    #[inline]
    fn default() -> Self {
        Self::Std(StdAddr::default())
    }
}

impl IntAddr {
    /// The maximum number of bits that address occupies.
    pub const BITS_MAX: u16 = 1 + VarAddr::BITS_MAX;

    /// Returns `true` if this address is for a masterchain block.
    ///
    /// See [`ShardIdent::MASTERCHAIN`]
    #[inline]
    pub const fn is_masterchain(&self) -> bool {
        self.workchain() == ShardIdent::MASTERCHAIN.workchain()
    }

    /// Returns the workchain part of the address.
    #[inline]
    pub const fn workchain(&self) -> i32 {
        match self {
            Self::Std(addr) => addr.workchain as i32,
            Self::Var(addr) => addr.workchain,
        }
    }

    /// Returns the workchain part of the address.
    #[inline]
    pub const fn anycast(&self) -> &Option<Box<Anycast>> {
        match self {
            Self::Std(addr) => &addr.anycast,
            Self::Var(addr) => &addr.anycast,
        }
    }

    /// Returns the underlying standard address.
    #[inline]
    pub const fn as_std(&self) -> Option<&StdAddr> {
        match self {
            Self::Std(addr) => Some(addr),
            Self::Var(_) => None,
        }
    }

    /// Returns the underlying variable-length address.
    #[inline]
    pub const fn as_var(&self) -> Option<&VarAddr> {
        match self {
            Self::Std(_) => None,
            Self::Var(addr) => Some(addr),
        }
    }

    /// Returns the number of data bits that this struct occupies.
    pub const fn bit_len(&self) -> u16 {
        match self {
            Self::Std(addr) => addr.bit_len(),
            Self::Var(addr) => addr.bit_len(),
        }
    }

    /// Returns the high bits of the address as a number.
    pub fn prefix(&self) -> u64 {
        match self {
            Self::Std(addr) => addr.prefix(),
            Self::Var(addr) => addr.prefix(),
        }
    }
}

impl Addr for IntAddr {
    #[inline]
    fn workchain(&self) -> i32 {
        IntAddr::workchain(self)
    }

    #[inline]
    fn prefix(&self) -> u64 {
        IntAddr::prefix(self)
    }
}

impl From<(i8, HashBytes)> for IntAddr {
    #[inline]
    fn from((workchain, address): (i8, HashBytes)) -> Self {
        IntAddr::Std(StdAddr::new(workchain, address))
    }
}

impl FromStr for IntAddr {
    type Err = ParseAddrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // TODO: impl from_str for VarAddr
        Ok(Self::Std(ok!(StdAddr::from_str(s))))
    }
}

impl std::fmt::Display for IntAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntAddr::Std(addr) => std::fmt::Display::fmt(addr, f),
            IntAddr::Var(_) => f.write_str("varaddr"), // TODO: impl display
        }
    }
}

impl Store for IntAddr {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        match self {
            Self::Std(addr) => addr.store_into(builder, context),
            Self::Var(addr) => addr.store_into(builder, context),
        }
    }
}

impl<'a> Load<'a> for IntAddr {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        if !ok!(slice.load_bit()) {
            return Err(Error::InvalidTag);
        }

        Ok(if unlikely(ok!(slice.load_bit())) {
            let anycast = ok!(Option::<Box<Anycast>>::load_from(slice));
            let address_len = ok!(Uint9::load_from(slice));
            let workchain = ok!(slice.load_u32()) as i32;
            if !slice.has_remaining(address_len.into_inner(), 0) {
                return Err(Error::CellUnderflow);
            }

            let mut address = vec![0; address_len.into_inner().div_ceil(8) as usize];
            ok!(slice.load_raw(&mut address, address_len.into_inner()));

            Self::Var(VarAddr {
                anycast,
                address_len,
                workchain,
                address,
            })
        } else {
            Self::Std(StdAddr {
                anycast: ok!(Option::<Box<Anycast>>::load_from(slice)),
                workchain: ok!(slice.load_u8()) as i8,
                address: ok!(slice.load_u256()),
            })
        })
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for IntAddr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Std(addr) => addr.serialize(serializer),
            Self::Var(_) => {
                // TODO: impl serde for `VarAddr`
                serializer.serialize_str("varaddr")
            }
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for IntAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // TODO: impl serde for `VarAddr`
        StdAddr::deserialize(deserializer).map(IntAddr::Std)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for IntAddr {
    #[inline]
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        if u.ratio(1u8, 20u8)? {
            u.arbitrary().map(Self::Var)
        } else {
            u.arbitrary().map(Self::Std)
        }
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        Self::try_size_hint(depth).unwrap_or_default()
    }

    #[inline]
    fn try_size_hint(
        depth: usize,
    ) -> Result<(usize, Option<usize>), arbitrary::MaxRecursionReached> {
        Ok(arbitrary::size_hint::and(
            <u8 as arbitrary::Arbitrary>::try_size_hint(depth)?,
            arbitrary::size_hint::or(StdAddr::size_hint(depth), VarAddr::size_hint(depth)),
        ))
    }
}

/// Standard internal address.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct StdAddr {
    /// Optional anycast info.
    pub anycast: Option<Box<Anycast>>,
    /// Workchain id (one-byte range).
    pub workchain: i8,
    /// Account id.
    pub address: HashBytes,
}

impl StdAddr {
    /// The number of data bits that address without anycast occupies.
    ///
    /// - 2 bits id (`0b10`)
    /// - 1 bit Maybe None
    /// - 8 bits workchain
    /// - 256 bits address
    pub const BITS_WITHOUT_ANYCAST: u16 = 2 + 1 + 8 + 256;

    /// The maximum number of bits that address with anycast occupies.
    pub const BITS_MAX: u16 = Self::BITS_WITHOUT_ANYCAST + Anycast::BITS_MAX;

    /// Address with all values set as zero (e.g. `0:000...000`).
    ///
    /// Same as `StdAddr::default()`.
    pub const ZERO: Self = Self::new(0, HashBytes::ZERO);

    /// Constructs a new masterchain standard address without anycast info.
    pub const fn new_masterchain(address: HashBytes) -> Self {
        Self {
            anycast: None,
            workchain: ShardIdent::MASTERCHAIN.workchain() as i8,
            address,
        }
    }

    /// Constructs a new standard address without anycast info.
    #[inline]
    pub const fn new(workchain: i8, address: HashBytes) -> Self {
        Self {
            anycast: None,
            workchain,
            address,
        }
    }

    /// Parses a base64-encoded address.
    #[cfg(feature = "base64")]
    pub fn from_str_ext(
        s: &str,
        format: StdAddrFormat,
    ) -> Result<(Self, Base64StdAddrFlags), ParseAddrError> {
        use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE, Engine as _};

        match s.len() {
            0 => Err(ParseAddrError::Empty),
            66..=69 if format.allow_raw => Self::from_str(s).map(|addr| (addr, Default::default())),
            48 if format.allow_base64 || format.allow_base64_url => {
                let mut buffer = [0u8; 36];

                let base64_url = s.contains(['_', '-']);

                let Ok(36) = if base64_url {
                    BASE64_URL_SAFE
                } else {
                    BASE64_STANDARD
                }
                .decode_slice(s, &mut buffer) else {
                    return Err(ParseAddrError::BadFormat);
                };

                #[cfg(not(fuzzing))]
                {
                    let crc = crate::crc::crc_16(&buffer[..34]);
                    if buffer[34] as u16 != (crc >> 8) || buffer[35] as u16 != (crc & 0xff) {
                        return Err(ParseAddrError::BadFormat);
                    }
                }

                let addr = StdAddr::new(
                    buffer[1] as i8,
                    HashBytes(buffer[2..34].try_into().unwrap()),
                );
                let flags = Base64StdAddrFlags {
                    testnet: buffer[0] & 0x80 != 0,
                    base64_url,
                    bounceable: buffer[0] & 0x40 == 0,
                };
                Ok((addr, flags))
            }
            _ => Err(ParseAddrError::BadFormat),
        }
    }

    /// Returns `true` if this address is for a masterchain block.
    ///
    /// See [`ShardIdent::MASTERCHAIN`]
    #[inline]
    pub const fn is_masterchain(&self) -> bool {
        self.workchain as i32 == ShardIdent::MASTERCHAIN.workchain()
    }

    /// Returns `true` if this address is `0:000...000`.
    pub const fn is_zero(&self) -> bool {
        self.workchain == 0 && self.anycast.is_none() && self.address.is_zero()
    }

    /// Returns the number of data bits that this struct occupies.
    pub const fn bit_len(&self) -> u16 {
        let mut bit_len = Self::BITS_WITHOUT_ANYCAST;
        if let Some(anycast) = &self.anycast {
            bit_len += anycast.bit_len();
        }
        bit_len
    }

    /// Returns the high bits of the address as a number.
    pub const fn prefix(&self) -> u64 {
        u64::from_be_bytes(*self.address.first_chunk())
    }

    /// Returns a pretty-printer for base64-encoded address.
    #[cfg(feature = "base64")]
    pub const fn display_base64(&self, bounceable: bool) -> DisplayBase64StdAddr<'_> {
        DisplayBase64StdAddr {
            addr: self,
            flags: Base64StdAddrFlags {
                testnet: false,
                base64_url: false,
                bounceable,
            },
        }
    }

    /// Returns a pretty-printer for URL-safe base64-encoded address.
    #[cfg(feature = "base64")]
    pub const fn display_base64_url(&self, bounceable: bool) -> DisplayBase64StdAddr<'_> {
        DisplayBase64StdAddr {
            addr: self,
            flags: Base64StdAddrFlags {
                testnet: false,
                base64_url: true,
                bounceable,
            },
        }
    }
}

impl Default for StdAddr {
    #[inline]
    fn default() -> Self {
        Self::ZERO
    }
}

impl Addr for StdAddr {
    #[inline]
    fn workchain(&self) -> i32 {
        self.workchain as i32
    }

    #[inline]
    fn prefix(&self) -> u64 {
        StdAddr::prefix(self)
    }
}

impl std::fmt::Display for StdAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(anycast) = &self.anycast {
            ok!(f.write_fmt(format_args!("{anycast}:")))
        }

        f.write_fmt(format_args!("{}:{}", self.workchain, self.address))
    }
}

impl From<(i8, HashBytes)> for StdAddr {
    #[inline]
    fn from((workchain, address): (i8, HashBytes)) -> Self {
        Self::new(workchain, address)
    }
}

impl From<(i8, [u8; 32])> for StdAddr {
    #[inline]
    fn from((workchain, address): (i8, [u8; 32])) -> Self {
        Self::new(workchain, HashBytes(address))
    }
}

impl From<StdAddr> for IntAddr {
    #[inline]
    fn from(value: StdAddr) -> Self {
        Self::Std(value)
    }
}

impl FromStr for StdAddr {
    type Err = ParseAddrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ParseAddrError::Empty);
        }

        let mut result = Self::default();

        let mut parts = s.split(':');
        match parts.next() {
            Some(part) => match part.parse() {
                Ok(workchain) => result.workchain = workchain,
                Err(_) => return Err(ParseAddrError::InvalidWorkchain),
            },
            None => return Err(ParseAddrError::Empty),
        }

        match parts.next() {
            Some(part) => match hex::decode_to_slice(part, &mut result.address.0) {
                Ok(()) => {}
                Err(_) => return Err(ParseAddrError::InvalidAccountId),
            },
            None => return Err(ParseAddrError::InvalidAccountId),
        }

        if parts.next().is_none() {
            Ok(result)
        } else {
            Err(ParseAddrError::UnexpectedPart)
        }
    }
}

impl Store for StdAddr {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        if !builder.has_capacity(self.bit_len(), 0) {
            return Err(Error::CellOverflow);
        }
        ok!(builder.store_small_uint(0b10, 2));
        ok!(self.anycast.store_into(builder, context));
        ok!(builder.store_u8(self.workchain as u8));
        builder.store_u256(&self.address)
    }
}

impl<'a> Load<'a> for StdAddr {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        if !ok!(slice.load_bit()) || ok!(slice.load_bit()) {
            return Err(Error::InvalidTag);
        }

        Ok(Self {
            anycast: ok!(Option::<Box<Anycast>>::load_from(slice)),
            workchain: ok!(slice.load_u8()) as i8,
            address: ok!(slice.load_u256()),
        })
    }
}

impl crate::dict::DictKey for StdAddr {
    const BITS: u16 = StdAddr::BITS_WITHOUT_ANYCAST;
}

impl crate::dict::StoreDictKey for StdAddr {
    fn store_into_data(&self, builder: &mut CellDataBuilder) -> Result<(), Error> {
        if self.anycast.is_some() || !builder.has_capacity_bits(Self::BITS_WITHOUT_ANYCAST) {
            return Err(Error::InvalidData);
        }
        ok!(builder.store_small_uint(0b100, 3)); // `0b10` (tag) | `0b0` ("none" for anycast)
        ok!(builder.store_u8(self.workchain as u8));
        builder.store_u256(&self.address)
    }
}

impl crate::dict::LoadDictKey for StdAddr {
    fn load_from_data(data: &CellDataBuilder) -> Option<Self> {
        let [first_byte, second_byte, data @ ..] = data.raw_data();

        // 2 bits id, 1 bit maybe (None), 8 bits workchain, 256 bits address

        const PREFIX_BITS: u8 = 0b1000_0000;
        const PREFIX_MASK: u8 = 0b1110_0000;

        const R: u8 = 3;
        const SHIFT: u8 = 8 - R;
        const REV_SHIFT: u8 = 120 + R;

        if unlikely((first_byte ^ PREFIX_BITS) & PREFIX_MASK != 0) {
            return None;
        }

        let mut result = Self {
            anycast: None,
            // 100xxxxx | xxxaaaaa -> xxxxxxxx
            workchain: ((first_byte << R) | (second_byte >> SHIFT)) as i8,
            address: HashBytes::ZERO,
        };

        // SAFETY: transmuting [u8; 32] to [u128; 2] is safe
        let [mut hi, mut lo]: [u128; 2] =
            unsafe { std::mem::transmute::<[u8; 32], _>(data[..32].try_into().unwrap()) };

        // Numbers are in big endian order, swap bytes on little endian arch
        #[cfg(target_endian = "little")]
        {
            hi = hi.swap_bytes();
            lo = lo.swap_bytes();
        }

        // SAFETY: transmuting [[u8; 16]; 2] to [u8; 32] is safe
        result.address = unsafe {
            std::mem::transmute::<[[u8; 16]; 2], HashBytes>([
                ((hi >> SHIFT) | ((*second_byte as u128) << REV_SHIFT)).to_be_bytes(),
                ((lo >> SHIFT) | (hi << REV_SHIFT)).to_be_bytes(),
            ])
        };

        Some(result)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for StdAddr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.collect_str(self)
        } else {
            (self.workchain, &self.address).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for StdAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Error, Visitor};

        struct StdAddrVisitor;

        impl Visitor<'_> for StdAddrVisitor {
            type Value = StdAddr;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a standard address")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                StdAddr::from_str(v).map_err(E::custom)
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(StdAddrVisitor)
        } else {
            <(i8, HashBytes)>::deserialize(deserializer)
                .map(|(workchain, address)| Self::new(workchain, address))
        }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for StdAddr {
    #[inline]
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        Ok(Self {
            anycast: u.ratio(1u8, 20u8)?.then(|| u.arbitrary()).transpose()?,
            workchain: u.arbitrary()?,
            address: u.arbitrary()?,
        })
    }

    #[inline]
    fn size_hint(depth: usize) -> (usize, Option<usize>) {
        arbitrary::size_hint::and(Option::<Anycast>::size_hint(depth), (33, Some(33)))
    }
}

/// A helper struct to work with base64-encoded addresses.
#[cfg(feature = "base64")]
pub struct StdAddrBase64Repr<const URL_SAFE: bool = true>;

#[cfg(all(feature = "base64", feature = "serde"))]
impl<const URL_SAFE: bool> StdAddrBase64Repr<URL_SAFE> {
    /// Serializes address into a base64-encoded string.
    pub fn serialize<S>(addr: &StdAddr, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&DisplayBase64StdAddr {
            addr,
            flags: Base64StdAddrFlags {
                testnet: false,
                base64_url: URL_SAFE,
                bounceable: false,
            },
        })
    }

    /// Deserializes address as a base64-encoded string.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<StdAddr, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Error, Visitor};

        struct StdAddrBase64Visitor;

        impl Visitor<'_> for StdAddrBase64Visitor {
            type Value = StdAddr;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a standard address")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                StdAddr::from_str_ext(v, StdAddrFormat::any())
                    .map(|(addr, _)| addr)
                    .map_err(E::custom)
            }
        }

        deserializer.deserialize_str(StdAddrBase64Visitor)
    }
}

/// Parsing options for [`StdAddr::from_str_ext`]
#[cfg(feature = "base64")]
#[derive(Debug, Clone, Copy)]
pub struct StdAddrFormat {
    /// Allow raw address (0:000...000).
    pub allow_raw: bool,
    /// Allow base64-encoded address.
    pub allow_base64: bool,
    /// Allow URL-safe base64 encoding.
    pub allow_base64_url: bool,
}

#[cfg(feature = "base64")]
impl StdAddrFormat {
    /// Allows any address format.
    pub const fn any() -> Self {
        StdAddrFormat {
            allow_raw: true,
            allow_base64: true,
            allow_base64_url: true,
        }
    }
}

/// Base64-encoded address flags.
#[cfg(feature = "base64")]
#[derive(Default, Debug, Copy, Clone, Eq, PartialEq)]
pub struct Base64StdAddrFlags {
    /// Address belongs to testnet.
    pub testnet: bool,
    /// Use URL-safe base64 encoding.
    pub base64_url: bool,
    /// Whether to set `bounce` flag during transfer.
    pub bounceable: bool,
}

/// Pretty-printer for [`StdAddr`] in base64 format.
#[cfg(feature = "base64")]
pub struct DisplayBase64StdAddr<'a> {
    /// Address to display.
    pub addr: &'a StdAddr,
    /// Encoding flags.
    pub flags: Base64StdAddrFlags,
}

#[cfg(feature = "base64")]
impl std::fmt::Display for DisplayBase64StdAddr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use base64::prelude::{BASE64_STANDARD, BASE64_URL_SAFE, Engine as _};

        let mut buffer = [0u8; 36];
        buffer[0] = (0x51 - (self.flags.bounceable as i32) * 0x40
            + (self.flags.testnet as i32) * 0x80) as u8;
        buffer[1] = self.addr.workchain as u8;
        buffer[2..34].copy_from_slice(self.addr.address.as_array());

        let crc = crate::crc::crc_16(&buffer[..34]);
        buffer[34] = (crc >> 8) as u8;
        buffer[35] = (crc & 0xff) as u8;

        let mut output = [0u8; 48];
        if self.flags.base64_url {
            BASE64_URL_SAFE
        } else {
            BASE64_STANDARD
        }
        .encode_slice(buffer, &mut output)
        .unwrap();

        // SAFETY: output is guaranteed to contain only ASCII.
        let output = unsafe { std::str::from_utf8_unchecked(&output) };
        f.write_str(output)
    }
}

/// Variable-length internal address.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct VarAddr {
    /// Optional anycast info.
    pub anycast: Option<Box<Anycast>>,
    /// Address length in bits.
    pub address_len: Uint9,
    /// Workchain id (full range).
    pub workchain: i32,
    /// Variable-length address.
    pub address: Vec<u8>,
}

impl VarAddr {
    /// The maximum number of bits that address occupies.
    ///
    /// - 2 bits id (`0b11`)
    /// - 1 + `Anycast::BITS_MAX` maybe anycast
    /// - 9 bits `address_len`
    /// - 32 bits workchain
    /// - `address_len` bits of address
    pub const BITS_MAX: u16 =
        2 + 1 + Anycast::BITS_MAX + Uint9::BITS + 32 + Uint9::MAX.into_inner();

    /// Returns `true` if this address is for a masterchain block.
    ///
    /// See [`ShardIdent::MASTERCHAIN`]
    #[inline]
    pub const fn is_masterchain(&self) -> bool {
        self.workchain == ShardIdent::MASTERCHAIN.workchain()
    }

    /// Returns the number of data bits that this struct occupies.
    pub const fn bit_len(&self) -> u16 {
        let mut bit_len = 2 + 1 + Uint9::BITS + 32 + self.address_len.into_inner();
        if let Some(anycast) = &self.anycast {
            bit_len += anycast.bit_len();
        }
        bit_len
    }

    /// Returns the high bits of the address as a number.
    pub fn prefix(&self) -> u64 {
        let mut prefix = [0; 8];
        let total_bytes = std::cmp::min(self.address.len(), 8);
        prefix[..total_bytes].copy_from_slice(&self.address[..total_bytes]);
        u64::from_be_bytes(prefix)
    }
}

impl From<VarAddr> for IntAddr {
    #[inline]
    fn from(value: VarAddr) -> Self {
        Self::Var(value)
    }
}

impl Store for VarAddr {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        if !builder.has_capacity(self.bit_len(), 0) {
            return Err(Error::CellOverflow);
        }
        ok!(builder.store_small_uint(0b11, 2));
        ok!(self.anycast.store_into(builder, context));
        ok!(self.address_len.store_into(builder, context));
        ok!(builder.store_u32(self.workchain as u32));
        builder.store_raw(&self.address, self.address_len.into_inner())
    }
}

impl Addr for VarAddr {
    #[inline]
    fn workchain(&self) -> i32 {
        self.workchain
    }

    #[inline]
    fn prefix(&self) -> u64 {
        VarAddr::prefix(self)
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for VarAddr {
    #[inline]
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let anycast = u.ratio(1u8, 20u8)?.then(|| u.arbitrary()).transpose()?;
        let address_len = u.arbitrary::<Uint9>()?;
        let workchain = u.arbitrary()?;

        let bit_len = address_len.into_inner() as usize;
        let mut address = u.bytes(bit_len.div_ceil(8))?.to_vec();
        if let Some(last_byte) = address.last_mut() {
            let rem = bit_len % 8;
            if rem != 0 {
                *last_byte &= u8::MAX << (8 - rem);
            }
        }

        Ok(Self {
            anycast,
            address_len,
            workchain,
            address,
        })
    }

    #[inline]
    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (1 + 2 + 4, None)
    }
}

/// External address.
///
/// ```text
/// addr_none$00 = MsgAddressExt;
/// addr_extern$01 len:(## 9) external_address:(bits len) = MsgAddressExt;
/// ```
#[derive(Default, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct ExtAddr {
    /// Number of bits stored in data.
    pub data_bit_len: Uint9,
    /// External address data
    pub data: Box<[u8]>,
}

impl ExtAddr {
    /// Creates non-empty external address.
    pub fn new<T>(data_bit_len: u16, data: T) -> Option<Self>
    where
        T: Into<Vec<u8>>,
    {
        let data_bit_len = Uint9::new(data_bit_len);
        if data_bit_len.is_valid() {
            let mut data = data.into();

            // Normalize data size.
            let bits = data_bit_len.into_inner() as usize;
            let bytes = bits.div_ceil(8);
            if let Some(additional) = bytes.checked_sub(data.len())
                && additional > 0
            {
                // Use `_exact` here to avoid realloc during the final `into_boxed_slice`.
                data.reserve_exact(additional);
            }
            data.resize(bytes, 0);

            // Normalize the trailing bit (remove it).
            let rem = bits % 8;
            if rem > 0 {
                debug_assert!(bytes > 0, "non-zero bits cannot fit into zero bytes");
                let tag_mask = 1u8 << (7 - rem);
                let data_mask = !(tag_mask - 1) << 1;
                data[bytes - 1] &= data_mask;
            }

            Some(Self {
                data_bit_len,
                data: data.into_boxed_slice(),
            })
        } else {
            None
        }
    }

    /// Returns the number of data bits that this struct occupies.
    pub const fn bit_len(&self) -> u16 {
        Uint9::BITS + self.data_bit_len.into_inner()
    }
}

impl std::fmt::Display for ExtAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bitstring = Bitstring {
            bytes: &self.data,
            bit_len: self.data_bit_len.into_inner(),
        };
        write!(f, ":{bitstring}")
    }
}

impl std::fmt::Debug for ExtAddr {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl FromStr for ExtAddr {
    type Err = ParseAddrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(ParseAddrError::Empty);
        }

        let Some(s) = s.strip_prefix(':') else {
            return Err(ParseAddrError::UnexpectedPart);
        };

        let Ok((data, bit_len)) = Bitstring::from_hex_str(s) else {
            return Err(ParseAddrError::InvalidAccountId);
        };

        ExtAddr::new(bit_len, data).ok_or(ParseAddrError::UnexpectedPart)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for ExtAddr {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.collect_str(self)
        } else {
            (self.data_bit_len.into_inner(), &self.data).serialize(serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ExtAddr {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{Error, Visitor};

        struct ExtAddrVisitor;

        impl Visitor<'_> for ExtAddrVisitor {
            type Value = ExtAddr;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an external address")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                ExtAddr::from_str(v).map_err(E::custom)
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(ExtAddrVisitor)
        } else {
            <(u16, Vec<u8>)>::deserialize(deserializer).and_then(|(data_bit_len, data)| {
                ExtAddr::new(data_bit_len, data)
                    .ok_or_else(|| Error::custom("invalid external address data length"))
            })
        }
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for ExtAddr {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let data_bit_len = u.arbitrary::<Uint9>()?;

        let bit_len = data_bit_len.into_inner() as usize;
        let mut data = u.bytes(bit_len.div_ceil(8))?.to_vec();
        if let Some(last_byte) = data.last_mut() {
            let rem = bit_len % 8;
            if rem != 0 {
                *last_byte &= u8::MAX << (8 - rem);
            }
        }

        Ok(Self {
            data_bit_len,
            data: data.into_boxed_slice(),
        })
    }

    #[inline]
    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (2, None)
    }
}

/// Anycast prefix info.
///
/// ```text
/// anycast_info$_ depth:(#<= 30) { depth >= 1 } rewrite_pfx:(bits depth) = Anycast;
/// ```
#[derive(Debug, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Anycast {
    /// Prefix length in bits.
    pub depth: SplitDepth,
    /// Rewrite prefix data.
    pub rewrite_prefix: Vec<u8>,
}

impl Anycast {
    /// The minimum allowed number of bits in the rewrite prefix.
    pub const MIN_DEPTH: u8 = 1;
    /// The maximum allowed number of bits in the rewrite prefix.
    pub const MAX_DEPTH: u8 = 30;

    /// The maximum number of bits that an Anycast occupies.
    pub const BITS_MAX: u16 = SplitDepth::BITS + Self::MAX_DEPTH as u16;

    /// Constructs anycast info from rewrite prefix.
    pub fn from_slice(rewrite_prefix: &CellSlice<'_>) -> Result<Self, Error> {
        let depth = ok!(SplitDepth::from_bit_len(rewrite_prefix.size_bits()));

        let mut data = vec![0; depth.into_bit_len().div_ceil(8) as usize];
        ok!(rewrite_prefix.get_raw(0, &mut data, depth.into_bit_len()));

        Ok(Self {
            depth,
            rewrite_prefix: data,
        })
    }

    /// Returns the number of data bits that this struct occupies.
    pub const fn bit_len(&self) -> u16 {
        SplitDepth::BITS + self.depth.into_bit_len()
    }
}

impl std::fmt::Display for Anycast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let bitstring = Bitstring {
            bytes: &self.rewrite_prefix,
            bit_len: self.depth.into_bit_len(),
        };
        std::fmt::Display::fmt(&bitstring, f)
    }
}

impl Store for Anycast {
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        if !builder.has_capacity(self.bit_len(), 0) {
            return Err(Error::CellOverflow);
        }
        ok!(self.depth.store_into(builder, context));
        builder.store_raw(&self.rewrite_prefix, self.depth.into_bit_len())
    }
}

impl<'a> Load<'a> for Anycast {
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        let depth = ok!(SplitDepth::load_from(slice));
        if !slice.has_remaining(depth.into_bit_len(), 0) {
            return Err(Error::CellUnderflow);
        }

        let mut rewrite_prefix = vec![0; depth.into_bit_len().div_ceil(8) as usize];
        ok!(slice.load_raw(&mut rewrite_prefix, depth.into_bit_len()));

        Ok(Self {
            depth,
            rewrite_prefix,
        })
    }
}

#[cfg(feature = "arbitrary")]
impl<'a> arbitrary::Arbitrary<'a> for Anycast {
    #[inline]
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let split_depth = SplitDepth::arbitrary(u)?;
        let bit_len = split_depth.into_bit_len();

        let bytes = u.bytes(bit_len.div_ceil(8) as _)?;

        let b = CellBuilder::from_raw_data(bytes, bit_len).unwrap();
        Ok(Self::from_slice(&b.as_data_slice()).unwrap())
    }

    #[inline]
    fn size_hint(_: usize) -> (usize, Option<usize>) {
        (2, Some(5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dict::Dict;

    #[test]
    fn dict_with_std_addr_keys() {
        let mut dict = Dict::<StdAddr, u32>::new();
        dict.set(StdAddr::new(-1, HashBytes([0x33; 32])), 123)
            .unwrap();
        dict.set(StdAddr::new(0, HashBytes([0x10; 32])), 321)
            .unwrap();
        dict.set(StdAddr::new(-1, HashBytes([0x55; 32])), 234)
            .unwrap();
        dict.set(StdAddr::new(0, HashBytes([0x20; 32])), 432)
            .unwrap();

        for entry in dict.iter() {
            let (addr, value) = entry.unwrap();
            println!("{addr}: {value}");
        }
    }

    #[test]
    fn anycast_str() {
        // 0 bit
        let empty_res = Anycast::from_slice(&Cell::empty_cell().as_slice().unwrap());
        assert_eq!(empty_res.unwrap_err(), Error::IntOverflow);

        // 1 bit
        let mut prefix = CellBuilder::new();
        prefix.store_bit_one().unwrap();
        let anycast = Anycast::from_slice(&prefix.as_data_slice()).unwrap();
        assert_eq!(anycast.to_string(), "c_");

        // 8 bit
        let mut prefix = CellBuilder::new();
        prefix.store_u8(0xa5).unwrap();
        let anycast = Anycast::from_slice(&prefix.as_data_slice()).unwrap();
        assert_eq!(anycast.to_string(), "a5");

        // 30 bit
        let mut prefix = CellBuilder::new();
        prefix.store_uint(0xb00b1e5, 28).unwrap();
        prefix.store_zeros(2).unwrap();
        let anycast = Anycast::from_slice(&prefix.as_data_slice()).unwrap();
        assert_eq!(anycast.to_string(), "b00b1e52_");
    }

    #[test]
    fn address_prefix() {
        let addr = "0:ece57bcc6c530283becbbd8a3b24d3c5987cdddc3c8b7b33be6e4a6312490415"
            .parse::<StdAddr>()
            .unwrap();
        assert_eq!(addr.prefix(), 0xece57bcc6c530283);

        let var_addr = VarAddr {
            anycast: None,
            address_len: Uint9::new(32),
            workchain: 0,
            address: vec![0xb0, 0xba, 0xca, 0xfe],
        };
        assert_eq!(var_addr.prefix(), 0xb0bacafe00000000);

        let var_addr = VarAddr {
            anycast: None,
            address_len: Uint9::new(72),
            workchain: 0,
            address: vec![0xb0, 0xba, 0xca, 0xfe, 0xb0, 0x0b, 0x1e, 0x5a, 0xff],
        };
        assert_eq!(var_addr.prefix(), 0xb0bacafeb00b1e5a);
    }

    #[test]
    fn base64_address() {
        let addr = "0:84545d4d2cada0ce811705d534c298ca42d29315d03a16eee794cefd191dfa79"
            .parse::<StdAddr>()
            .unwrap();
        assert_eq!(
            addr.display_base64(true).to_string(),
            "EQCEVF1NLK2gzoEXBdU0wpjKQtKTFdA6Fu7nlM79GR36eWpw"
        );
        assert_eq!(
            StdAddr::from_str_ext(
                "EQCEVF1NLK2gzoEXBdU0wpjKQtKTFdA6Fu7nlM79GR36eWpw",
                StdAddrFormat::any()
            )
            .unwrap(),
            (addr, Base64StdAddrFlags {
                testnet: false,
                base64_url: false,
                bounceable: true,
            })
        );

        let addr = "0:dddde93b1d3398f0b4305c08de9a032e0bc1b257c4ce2c72090aea1ff3e9ecfd"
            .parse::<StdAddr>()
            .unwrap();
        assert_eq!(
            addr.display_base64_url(false).to_string(),
            "UQDd3ek7HTOY8LQwXAjemgMuC8GyV8TOLHIJCuof8-ns_Tyv"
        );
        assert_eq!(
            addr.display_base64(false).to_string(),
            "UQDd3ek7HTOY8LQwXAjemgMuC8GyV8TOLHIJCuof8+ns/Tyv"
        );

        assert_eq!(
            StdAddr::from_str_ext(
                "UQDd3ek7HTOY8LQwXAjemgMuC8GyV8TOLHIJCuof8+ns/Tyv",
                StdAddrFormat::any()
            )
            .unwrap(),
            (addr.clone(), Base64StdAddrFlags {
                testnet: false,
                base64_url: false,
                bounceable: false,
            })
        );

        assert_eq!(
            addr.display_base64_url(true).to_string(),
            "EQDd3ek7HTOY8LQwXAjemgMuC8GyV8TOLHIJCuof8-ns_WFq"
        );
        assert_eq!(
            addr.display_base64(true).to_string(),
            "EQDd3ek7HTOY8LQwXAjemgMuC8GyV8TOLHIJCuof8+ns/WFq"
        );

        assert_eq!(
            StdAddr::from_str_ext(
                "EQDd3ek7HTOY8LQwXAjemgMuC8GyV8TOLHIJCuof8-ns_WFq",
                StdAddrFormat::any()
            )
            .unwrap(),
            (addr, Base64StdAddrFlags {
                testnet: false,
                base64_url: true,
                bounceable: true,
            })
        );
    }

    #[test]
    fn ext_addr_works() {
        for bits in 0..=32 {
            let addr = ExtAddr::new(bits, vec![0xaau8; 2]).unwrap();
            println!("{addr}");
        }
    }
}
