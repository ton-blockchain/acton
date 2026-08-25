use num_bigint::BigInt;
use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};
use tycho_types::cell::{
    Cell, CellBuilder, CellFamily, CellSlice, CellSliceParts, CellSliceRange, Load, Store,
};
use tycho_types::dict::RawDict;
use tycho_types::error::Error as CellError;

/// An error produced by an ABI cell or stack codec.
#[derive(Debug)]
pub enum AbiError {
    Cell(CellError),
    InvalidData(String),
    Unsupported(String),
    MissingCustomCodec { type_name: String },
    CustomCodecTypeMismatch { type_name: String },
}

impl fmt::Display for AbiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cell(error) => error.fmt(formatter),
            Self::InvalidData(message) | Self::Unsupported(message) => formatter.write_str(message),
            Self::MissingCustomCodec { type_name } => {
                write!(
                    formatter,
                    "custom pack/unpack is not registered for `{type_name}`"
                )
            }
            Self::CustomCodecTypeMismatch { type_name } => {
                write!(
                    formatter,
                    "custom pack/unpack has the wrong Rust type for `{type_name}`"
                )
            }
        }
    }
}

impl std::error::Error for AbiError {}

impl From<CellError> for AbiError {
    fn from(value: CellError) -> Self {
        Self::Cell(value)
    }
}

/// Static cell serializer implemented by non-generic generated declarations.
pub trait AbiStore {
    fn store_into(&self, builder: &mut CellBuilder) -> Result<(), AbiError>;

    fn to_cell(&self) -> Result<Cell, AbiError> {
        let mut builder = CellBuilder::new();
        self.store_into(&mut builder)?;
        Ok(builder.build()?)
    }
}

/// Static cell deserializer implemented by non-generic generated declarations.
pub trait AbiLoad: Sized {
    fn load_from(slice: &mut CellSlice<'_>) -> Result<Self, AbiError>;

    fn from_cell(cell: &Cell) -> Result<Self, AbiError> {
        let mut slice = cell.as_slice()?;
        let value = Self::load_from(&mut slice)?;
        ensure_empty(&slice)?;
        Ok(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellRef<T> {
    pub r#ref: Box<T>,
}

impl<T> CellRef<T> {
    #[must_use]
    pub fn new(value: T) -> Self {
        Self {
            r#ref: Box::new(value),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedSlice {
    pub range: CellSliceRange,
    pub cell: Cell,
}

impl OwnedSlice {
    #[must_use]
    pub fn full(cell: Cell) -> Self {
        Self {
            range: CellSliceRange::full(&cell),
            cell,
        }
    }

    pub fn as_slice(&self) -> Result<CellSlice<'_>, AbiError> {
        Ok(self.range.apply(&self.cell)?)
    }
}

impl From<CellSliceParts> for OwnedSlice {
    fn from((range, cell): CellSliceParts) -> Self {
        Self { range, cell }
    }
}

impl From<OwnedSlice> for CellSliceParts {
    fn from(value: OwnedSlice) -> Self {
        (value.range, value.cell)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitString(pub OwnedSlice);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dictionary<K, V>(pub Vec<(K, V)>);

impl<K, V> Dictionary<K, V> {
    #[must_use]
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.0.push((key, value));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.0.iter().map(|(key, value)| (key, value))
    }
}

pub fn invalid_data<T>(message: impl Into<String>) -> Result<T, AbiError> {
    Err(AbiError::InvalidData(message.into()))
}

pub fn unsupported<T>(message: impl Into<String>) -> Result<T, AbiError> {
    Err(AbiError::Unsupported(message.into()))
}

pub fn ensure_empty(slice: &CellSlice<'_>) -> Result<(), AbiError> {
    if slice.size_bits() == 0 && slice.size_refs() == 0 {
        Ok(())
    } else {
        Err(AbiError::InvalidData(format!(
            "expected end of slice, got {} bits and {} refs",
            slice.size_bits(),
            slice.size_refs()
        )))
    }
}

pub fn store_tlb<T: Store>(builder: &mut CellBuilder, value: &T) -> Result<(), AbiError> {
    value.store_into(builder, <Cell as CellFamily>::empty_context())?;
    Ok(())
}

pub fn load_tlb<T>(slice: &mut CellSlice<'_>) -> Result<T, AbiError>
where
    T: for<'a> Load<'a>,
{
    Ok(T::load_from(slice)?)
}

pub fn store_address_opt(
    builder: &mut CellBuilder,
    value: &Option<tycho_types::models::StdAddr>,
) -> Result<(), AbiError> {
    let value = value
        .as_ref()
        .map_or(tycho_types::models::AnyAddr::None, |value| {
            tycho_types::models::AnyAddr::Std(value.clone())
        });
    store_tlb(builder, &value)
}

pub fn load_address_opt(
    slice: &mut CellSlice<'_>,
) -> Result<Option<tycho_types::models::StdAddr>, AbiError> {
    match load_tlb::<tycho_types::models::AnyAddr>(slice)? {
        tycho_types::models::AnyAddr::None => Ok(None),
        tycho_types::models::AnyAddr::Std(address) => Ok(Some(address)),
        tycho_types::models::AnyAddr::Var(_) => Err(AbiError::InvalidData(
            "expected a standard internal address or null".to_owned(),
        )),
        tycho_types::models::AnyAddr::Ext(_) => Err(AbiError::InvalidData(
            "expected an internal address or null".to_owned(),
        )),
    }
}

pub fn matches_prefix(
    slice: &CellSlice<'_>,
    prefix_num: u64,
    prefix_len: u16,
) -> Result<bool, AbiError> {
    if slice.size_bits() < prefix_len {
        return Ok(false);
    }
    let mut probe = *slice;
    Ok(probe.load_uint(prefix_len)? == prefix_num)
}

pub fn check_prefix(
    slice: &mut CellSlice<'_>,
    prefix_num: u64,
    prefix_len: u16,
    type_name: &str,
) -> Result<(), AbiError> {
    let actual = slice.load_uint(prefix_len)?;
    if actual == prefix_num {
        Ok(())
    } else {
        Err(AbiError::InvalidData(format!(
            "Incorrect prefix for '{type_name}': expected {}, got {}",
            format_prefix(prefix_num, prefix_len),
            format_prefix(actual, prefix_len),
        )))
    }
}

#[must_use]
pub fn format_prefix(prefix_num: u64, prefix_len: u16) -> String {
    if prefix_len.is_multiple_of(4) {
        format!(
            "0x{prefix_num:0width$x}",
            width = usize::from(prefix_len / 4)
        )
    } else {
        format!("0b{prefix_num:0width$b}", width = usize::from(prefix_len))
    }
}

pub fn store_fixed_int(
    builder: &mut CellBuilder,
    value: &BigInt,
    bits: u16,
    signed: bool,
) -> Result<(), AbiError> {
    if signed {
        builder.store_bigint(value, bits, true)?;
    } else {
        let value = value.to_biguint().ok_or_else(|| {
            AbiError::InvalidData(format!("negative value does not fit into uint{bits}"))
        })?;
        builder.store_biguint(&value, bits, false)?;
    }
    Ok(())
}

pub fn load_fixed_int(
    slice: &mut CellSlice<'_>,
    bits: u16,
    signed: bool,
) -> Result<BigInt, AbiError> {
    Ok(slice.load_bigint(bits, signed)?)
}

pub fn store_var_int(
    builder: &mut CellBuilder,
    value: &BigInt,
    len_bits: u16,
    signed: bool,
) -> Result<(), AbiError> {
    if signed {
        builder.store_var_bigint(value, len_bits, true)?;
    } else {
        let value = value.to_biguint().ok_or_else(|| {
            AbiError::InvalidData("negative value does not fit into varuint".to_owned())
        })?;
        builder.store_var_biguint(&value, len_bits)?;
    }
    Ok(())
}

pub fn load_var_int(
    slice: &mut CellSlice<'_>,
    len_bits: u16,
    signed: bool,
) -> Result<BigInt, AbiError> {
    Ok(slice.load_var_bigint(len_bits, signed)?)
}

pub fn store_slice(builder: &mut CellBuilder, value: &OwnedSlice) -> Result<(), AbiError> {
    builder.store_slice(value.as_slice()?)?;
    Ok(())
}

pub fn load_remaining(slice: &mut CellSlice<'_>) -> Result<OwnedSlice, AbiError> {
    let mut builder = CellBuilder::new();
    builder.store_slice(*slice)?;
    let value = OwnedSlice::full(builder.build()?);
    slice.skip_first(slice.size_bits(), slice.size_refs())?;
    Ok(value)
}

pub fn store_bits(
    builder: &mut CellBuilder,
    value: &BitString,
    expected_bits: u16,
) -> Result<(), AbiError> {
    let slice = value.0.as_slice()?;
    if slice.size_bits() != expected_bits || slice.size_refs() != 0 {
        return Err(AbiError::InvalidData(format!(
            "expected {expected_bits} bits and 0 refs, got {} bits and {} refs",
            slice.size_bits(),
            slice.size_refs()
        )));
    }
    builder.store_slice(slice)?;
    Ok(())
}

pub fn load_bits(slice: &mut CellSlice<'_>, bits: u16) -> Result<BitString, AbiError> {
    let mut limited = *slice;
    limited.only_first(bits, 0)?;
    let mut builder = CellBuilder::new();
    builder.store_slice(limited)?;
    let cell = builder.build()?;
    slice.skip_first(bits, 0)?;
    Ok(BitString(OwnedSlice::full(cell)))
}

#[must_use]
pub fn string_to_cell(value: &str) -> Cell {
    let mut next = None;
    for chunk in value.as_bytes().chunks(127).rev() {
        let mut builder = CellBuilder::new();
        builder
            .store_raw(
                chunk,
                u16::try_from(chunk.len() * 8).expect("snake chunk fits into u16"),
            )
            .expect("127 bytes fit into a cell");
        if let Some(cell) = next {
            builder
                .store_reference(cell)
                .expect("a snake cell has at most one reference");
        }
        next = Some(builder.build().expect("a snake chunk fits into a cell"));
    }
    next.unwrap_or_default()
}

pub fn store_string(builder: &mut CellBuilder, value: &str) -> Result<(), AbiError> {
    builder.store_reference(string_to_cell(value))?;
    Ok(())
}

pub fn load_string(slice: &mut CellSlice<'_>) -> Result<String, AbiError> {
    let cell = slice.load_reference_cloned()?;
    crate::Tuple::parse_snake_string(&cell)
        .ok_or_else(|| AbiError::InvalidData("expected a snake string".to_owned()))
}

fn store_maybe_ref(builder: &mut CellBuilder, value: Option<Cell>) -> Result<(), AbiError> {
    builder.store_bit(value.is_some())?;
    if let Some(value) = value {
        builder.store_reference(value)?;
    }
    Ok(())
}

fn load_maybe_ref(slice: &mut CellSlice<'_>) -> Result<Option<Cell>, AbiError> {
    if slice.load_bit()? {
        Ok(Some(slice.load_reference_cloned()?))
    } else {
        Ok(None)
    }
}

pub fn store_array<T>(
    builder: &mut CellBuilder,
    values: &[T],
    mut store_item: impl FnMut(&T, &mut CellBuilder) -> Result<(), AbiError>,
) -> Result<(), AbiError> {
    let length = u8::try_from(values.len())
        .map_err(|_| AbiError::InvalidData("array<T> length exceeds 255".to_owned()))?;
    let mut tail = None;
    for value in values.iter().rev() {
        let mut chunk = CellBuilder::new();
        store_maybe_ref(&mut chunk, tail)?;
        store_item(value, &mut chunk)?;
        tail = Some(chunk.build()?);
    }
    builder.store_u8(length)?;
    store_maybe_ref(builder, tail)
}

pub fn load_array<T>(
    slice: &mut CellSlice<'_>,
    mut load_item: impl FnMut(&mut CellSlice<'_>) -> Result<T, AbiError>,
) -> Result<Vec<T>, AbiError> {
    let expected = usize::from(slice.load_u8()?);
    let mut head = load_maybe_ref(slice)?;
    let mut values = Vec::with_capacity(expected);
    while let Some(cell) = head {
        let mut chunk = cell.as_slice()?;
        head = load_maybe_ref(&mut chunk)?;
        while chunk.size_bits() != 0 || chunk.size_refs() != 0 {
            values.push(load_item(&mut chunk)?);
        }
    }
    if values.len() != expected {
        return Err(AbiError::InvalidData(format!(
            "mismatch array binary data: expected {expected} elements, got {}",
            values.len()
        )));
    }
    Ok(values)
}

pub fn store_lisp_list<T>(
    builder: &mut CellBuilder,
    values: &[T],
    mut store_item: impl FnMut(&T, &mut CellBuilder) -> Result<(), AbiError>,
) -> Result<(), AbiError> {
    let mut tail = Cell::default();
    for value in values {
        let mut item = CellBuilder::new();
        store_item(value, &mut item)?;
        item.store_reference(tail)?;
        tail = item.build()?;
    }
    builder.store_reference(tail)?;
    Ok(())
}

pub fn load_lisp_list<T>(
    slice: &mut CellSlice<'_>,
    mut load_item: impl FnMut(&mut CellSlice<'_>) -> Result<T, AbiError>,
) -> Result<Vec<T>, AbiError> {
    let mut head = slice.load_reference_cloned()?;
    let mut values = Vec::new();
    while head.reference_count() != 0 {
        let mut item = head.as_slice()?;
        let tail = item.load_reference_cloned()?;
        let value = load_item(&mut item)?;
        ensure_empty(&item)?;
        values.insert(0, value);
        head = tail;
    }
    Ok(values)
}

pub fn store_dictionary<const N: u16, K, V>(
    builder: &mut CellBuilder,
    values: &Dictionary<K, V>,
    mut store_key: impl FnMut(&K, &mut CellBuilder) -> Result<(), AbiError>,
    mut store_value: impl FnMut(&V, &mut CellBuilder) -> Result<(), AbiError>,
) -> Result<(), AbiError> {
    let root = build_dictionary_root::<N, _, _>(values, &mut store_key, &mut store_value)?;
    root.store_into(builder, <Cell as CellFamily>::empty_context())?;
    Ok(())
}

pub fn build_dictionary_root<const N: u16, K, V>(
    values: &Dictionary<K, V>,
    mut store_key: impl FnMut(&K, &mut CellBuilder) -> Result<(), AbiError>,
    mut store_value: impl FnMut(&V, &mut CellBuilder) -> Result<(), AbiError>,
) -> Result<Option<Cell>, AbiError> {
    let mut dictionary = RawDict::<N>::new();
    for (key, value) in values.iter() {
        let mut key_builder = CellBuilder::new();
        store_key(key, &mut key_builder)?;
        if key_builder.size_bits() != N || key_builder.size_refs() != 0 {
            return Err(AbiError::InvalidData(format!(
                "dictionary key must contain {N} bits and no refs"
            )));
        }
        let key_cell = key_builder.build()?;
        let mut value_builder = CellBuilder::new();
        store_value(value, &mut value_builder)?;
        dictionary.set(key_cell.as_slice()?, value_builder)?;
    }
    Ok(dictionary.into_root())
}

pub fn load_dictionary<const N: u16, K, V>(
    slice: &mut CellSlice<'_>,
    mut load_key: impl FnMut(&mut CellSlice<'_>) -> Result<K, AbiError>,
    mut load_value: impl FnMut(&mut CellSlice<'_>) -> Result<V, AbiError>,
) -> Result<Dictionary<K, V>, AbiError> {
    let dictionary = RawDict::<N>::load_from(slice)?;
    load_dictionary_entries(dictionary, &mut load_key, &mut load_value)
}

pub fn load_dictionary_root<const N: u16, K, V>(
    root: Option<&Cell>,
    mut load_key: impl FnMut(&mut CellSlice<'_>) -> Result<K, AbiError>,
    mut load_value: impl FnMut(&mut CellSlice<'_>) -> Result<V, AbiError>,
) -> Result<Dictionary<K, V>, AbiError> {
    let Some(root) = root else {
        return Ok(Dictionary::new());
    };
    let mut root_slice = root.as_slice()?;
    let dictionary =
        RawDict::<N>::load_from_root_ext(&mut root_slice, <Cell as CellFamily>::empty_context())?;
    ensure_empty(&root_slice)?;
    load_dictionary_entries(dictionary, &mut load_key, &mut load_value)
}

fn load_dictionary_entries<const N: u16, K, V>(
    dictionary: RawDict<N>,
    load_key: &mut impl FnMut(&mut CellSlice<'_>) -> Result<K, AbiError>,
    load_value: &mut impl FnMut(&mut CellSlice<'_>) -> Result<V, AbiError>,
) -> Result<Dictionary<K, V>, AbiError> {
    let mut values = Dictionary::new();
    for entry in dictionary.iter() {
        let (key, mut value_slice) = entry?;
        let mut key_slice = key.as_data_slice();
        let key = load_key(&mut key_slice)?;
        ensure_empty(&key_slice)?;
        let value = load_value(&mut value_slice)?;
        ensure_empty(&value_slice)?;
        values.insert(key, value);
    }
    Ok(values)
}

type PackFn = dyn Fn(&dyn Any, &mut CellBuilder) -> Result<(), AbiError> + Send + Sync;
type UnpackFn = dyn for<'a> Fn(&mut CellSlice<'a>) -> Result<Box<dyn Any>, AbiError> + Send + Sync;

#[derive(Clone, Default)]
struct CustomCodec {
    pack: Option<Arc<PackFn>>,
    unpack: Option<Arc<UnpackFn>>,
}

fn custom_codecs() -> &'static Mutex<HashMap<String, CustomCodec>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, CustomCodec>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register_custom_codec<T: 'static>(
    type_name: impl Into<String>,
    pack: Option<impl Fn(&T, &mut CellBuilder) -> Result<(), AbiError> + Send + Sync + 'static>,
    unpack: Option<
        impl for<'a> Fn(&mut CellSlice<'a>) -> Result<T, AbiError> + Send + Sync + 'static,
    >,
) -> Result<(), AbiError> {
    let type_name = type_name.into();
    let pack = pack.map(|pack| {
        let type_name = type_name.clone();
        Arc::new(move |value: &dyn Any, builder: &mut CellBuilder| {
            let value =
                value
                    .downcast_ref::<T>()
                    .ok_or_else(|| AbiError::CustomCodecTypeMismatch {
                        type_name: type_name.clone(),
                    })?;
            pack(value, builder)
        }) as Arc<PackFn>
    });
    let unpack = unpack.map(|unpack| {
        Arc::new(move |slice: &mut CellSlice<'_>| {
            unpack(slice).map(|value| Box::new(value) as Box<dyn Any>)
        }) as Arc<UnpackFn>
    });
    let mut codecs = custom_codecs()
        .lock()
        .map_err(|_| AbiError::InvalidData("custom codec registry lock is poisoned".to_owned()))?;
    if codecs.contains_key(&type_name) {
        return Err(AbiError::InvalidData(format!(
            "custom pack/unpack for `{type_name}` is already registered"
        )));
    }
    codecs.insert(type_name, CustomCodec { pack, unpack });
    drop(codecs);
    Ok(())
}

pub fn custom_store<T: 'static>(
    type_name: &str,
    value: &T,
    builder: &mut CellBuilder,
) -> Result<(), AbiError> {
    let codec = custom_codecs()
        .lock()
        .map_err(|_| AbiError::InvalidData("custom codec registry lock is poisoned".to_owned()))?
        .get(type_name)
        .cloned()
        .ok_or_else(|| AbiError::MissingCustomCodec {
            type_name: type_name.to_owned(),
        })?;
    let pack = codec.pack.ok_or_else(|| AbiError::MissingCustomCodec {
        type_name: type_name.to_owned(),
    })?;
    pack(value, builder)
}

pub fn custom_load<T: 'static>(type_name: &str, slice: &mut CellSlice<'_>) -> Result<T, AbiError> {
    let codec = custom_codecs()
        .lock()
        .map_err(|_| AbiError::InvalidData("custom codec registry lock is poisoned".to_owned()))?
        .get(type_name)
        .cloned()
        .ok_or_else(|| AbiError::MissingCustomCodec {
            type_name: type_name.to_owned(),
        })?;
    let unpack = codec.unpack.ok_or_else(|| AbiError::MissingCustomCodec {
        type_name: type_name.to_owned(),
    })?;
    unpack(slice)?
        .downcast::<T>()
        .map(|value| *value)
        .map_err(|_| AbiError::CustomCodecTypeMismatch {
            type_name: type_name.to_owned(),
        })
}
