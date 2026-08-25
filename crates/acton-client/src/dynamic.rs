//! Dynamic Tolk ABI client runtime.
//!
//! This module is the Rust counterpart of upstream's `dynamic-ctx`,
//! `dynamic-serialization`, `dynamic-get-methods`, `dynamic-debug-print`, and
//! `dynamic-validation` modules. It deliberately uses the client type of a
//! field only for cells. TVM stack values always use the declared Tolk type.

use crate::{
    AbiError, BitString, Cell, ContractProvider, OwnedSlice, StackReader, StdAddr, Tuple, TupleItem,
};
use num_bigint::BigInt;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::sync::Arc;
use tolk_source_map::abi::{ABICustomPackUnpack, ABIDeclaration, ABIGetMethod, ContractABI};
use tolk_source_map::types_kernel::{Ty, TyIdx, UnionVariant, calc_width_on_stack, render_ty};
use tycho_types::cell::{CellBuilder, CellFamily, CellSlice, DynCell, Load, Store};
use tycho_types::dict::{self, SetMode};
use tycho_types::models::{AnyAddr, ExtAddr, IntAddr};
use tycho_types::util::Bitstring;

const SUPPORTED_ABI_SCHEMA_VERSION: &str = "1.0";

/// A dynamically typed value with the same shapes accepted by the upstream
/// JavaScript API.
///
/// Structs and unions are represented by [`Self::Object`]. A struct contains
/// a `$` string with its declaration name. A value-carrying union contains a
/// `$` string with its variant label and a `value` field. A `Cell<T>` is an
/// object with a `ref` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicValue {
    Null,
    Void,
    Number(BigInt),
    Bool(bool),
    String(String),
    Cell(Cell),
    Builder(Cell),
    Slice(OwnedSlice),
    Bits(BitString),
    Address(IntAddr),
    ExtAddress(ExtAddr),
    AddressNone,
    Array(Vec<Self>),
    Map(Vec<(Self, Self)>),
    Object(Vec<(String, Self)>),
    Unknown(TupleItem),
}

impl DynamicValue {
    #[must_use]
    pub fn object<I, K>(fields: I) -> Self
    where
        I: IntoIterator<Item = (K, Self)>,
        K: Into<String>,
    {
        Self::Object(
            fields
                .into_iter()
                .map(|(name, value)| (name.into(), value))
                .collect(),
        )
    }

    #[must_use]
    pub fn structure<I, K>(name: impl Into<String>, fields: I) -> Self
    where
        I: IntoIterator<Item = (K, Self)>,
        K: Into<String>,
    {
        let name = name.into();
        let mut result = Vec::new();
        result.push(("$".to_owned(), Self::String(name)));
        result.extend(
            fields
                .into_iter()
                .map(|(field_name, value)| (field_name.into(), value)),
        );
        Self::Object(result)
    }

    #[must_use]
    pub fn union(label: impl Into<String>, value: Self) -> Self {
        Self::object([("$", Self::String(label.into())), ("value", value)])
    }

    #[must_use]
    pub fn reference(value: Self) -> Self {
        Self::object([("ref", value)])
    }

    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Self> {
        let Self::Object(fields) = self else {
            return None;
        };
        fields
            .iter()
            .find_map(|(field_name, value)| (field_name == name).then_some(value))
    }

    #[must_use]
    pub fn tag(&self) -> Option<&str> {
        match self.field("$") {
            Some(Self::String(tag)) => Some(tag),
            _ => None,
        }
    }
}

impl From<BigInt> for DynamicValue {
    fn from(value: BigInt) -> Self {
        Self::Number(value)
    }
}

macro_rules! impl_number_from {
    ($($ty:ty),* $(,)?) => {
        $(
            impl From<$ty> for DynamicValue {
                fn from(value: $ty) -> Self {
                    Self::Number(BigInt::from(value))
                }
            }
        )*
    };
}

impl_number_from!(
    i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize
);

impl From<bool> for DynamicValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<String> for DynamicValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for DynamicValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<Cell> for DynamicValue {
    fn from(value: Cell) -> Self {
        Self::Cell(value)
    }
}

impl From<OwnedSlice> for DynamicValue {
    fn from(value: OwnedSlice) -> Self {
        Self::Slice(value)
    }
}

impl From<StdAddr> for DynamicValue {
    fn from(value: StdAddr) -> Self {
        Self::Address(IntAddr::Std(value))
    }
}

impl From<IntAddr> for DynamicValue {
    fn from(value: IntAddr) -> Self {
        Self::Address(value)
    }
}

impl From<ExtAddr> for DynamicValue {
    fn from(value: ExtAddr) -> Self {
        Self::ExtAddress(value)
    }
}

#[derive(Debug)]
pub enum DynamicError {
    Json(serde_json::Error),
    InvalidAbi(String),
    InvalidInput {
        field_path: String,
        expected: String,
        reason: String,
    },
    CannotPack {
        field_path: String,
        reason: String,
    },
    CannotUnpack {
        field_path: String,
        reason: String,
    },
    MethodNotFound {
        contract: String,
        method: String,
    },
    InvalidArgumentCount {
        method: String,
        expected: usize,
        actual: usize,
    },
    Abi(AbiError),
}

impl fmt::Display for DynamicError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "invalid ABI JSON: {error}"),
            Self::InvalidAbi(reason) => write!(formatter, "invalid ABI: {reason}"),
            Self::InvalidInput {
                field_path,
                expected,
                reason,
            } => write!(
                formatter,
                "invalid value passed for '{field_path}' of type '{expected}': {reason}"
            ),
            Self::CannotPack { field_path, reason } => {
                write!(
                    formatter,
                    "cannot serialize '{field_path}' dynamically: {reason}"
                )
            }
            Self::CannotUnpack { field_path, reason } => {
                write!(
                    formatter,
                    "cannot deserialize '{field_path}' dynamically: {reason}"
                )
            }
            Self::MethodNotFound { contract, method } => {
                write!(
                    formatter,
                    "cannot call get method '{method}' dynamically: method not found in contract {contract}"
                )
            }
            Self::InvalidArgumentCount {
                method,
                expected,
                actual,
            } => write!(
                formatter,
                "cannot call get method '{method}' dynamically: expected {expected} arguments, got {actual}"
            ),
            Self::Abi(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl StdError for DynamicError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Abi(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for DynamicError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<AbiError> for DynamicError {
    fn from(error: AbiError) -> Self {
        Self::Abi(error)
    }
}

impl From<tycho_types::error::Error> for DynamicError {
    fn from(error: tycho_types::error::Error) -> Self {
        Self::Abi(AbiError::Cell(error))
    }
}

#[derive(Debug)]
pub enum DynamicCallError<E> {
    Provider(E),
    Dynamic(DynamicError),
}

impl<E: fmt::Display> fmt::Display for DynamicCallError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(error) => write!(formatter, "contract provider failed: {error}"),
            Self::Dynamic(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl<E: StdError + 'static> StdError for DynamicCallError<E> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Provider(error) => Some(error),
            Self::Dynamic(error) => Some(error),
        }
    }
}

impl<E> From<DynamicError> for DynamicCallError<E> {
    fn from(error: DynamicError) -> Self {
        Self::Dynamic(error)
    }
}

pub type DynamicPackFn = Arc<
    dyn Fn(&DynamicValue, &mut CellBuilder) -> Result<(), DynamicError> + Send + Sync + 'static,
>;
pub type DynamicUnpackFn = Arc<
    dyn for<'a> Fn(&mut CellSlice<'a>) -> Result<DynamicValue, DynamicError>
        + Send
        + Sync
        + 'static,
>;

#[derive(Clone, Default)]
struct DynamicCustomCodec {
    pack: Option<DynamicPackFn>,
    unpack: Option<DynamicUnpackFn>,
}

#[derive(Debug, Clone)]
struct ResolvedField {
    name: String,
    ty_idx: TyIdx,
    union_label_ty_idx: Option<TyIdx>,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedAliasTarget {
    ty_idx: TyIdx,
    union_label_ty_idx: Option<TyIdx>,
}

#[derive(Debug, Clone)]
struct ResolvedUnionVariant {
    variant_ty_idx: TyIdx,
    prefix_num: u64,
    prefix_len: usize,
    prefix_is_implicit: bool,
    stack_type_id: Option<usize>,
    stack_width: Option<usize>,
    label: String,
    has_value_field: bool,
}

/// Parsed ABI plus the dynamic serializers and TVM stack interpreter.
pub struct DynamicAbi {
    abi: ContractABI,
    custom_codecs: HashMap<String, DynamicCustomCodec>,
}

impl DynamicAbi {
    pub fn from_json(json: &str) -> Result<Self, DynamicError> {
        let abi: ContractABI = serde_json::from_str(json)?;
        if abi.abi_schema_version != SUPPORTED_ABI_SCHEMA_VERSION {
            return Err(DynamicError::InvalidAbi(format!(
                "unsupported ABI schema version '{}', expected '{SUPPORTED_ABI_SCHEMA_VERSION}'",
                abi.abi_schema_version
            )));
        }
        Ok(Self {
            abi,
            custom_codecs: HashMap::new(),
        })
    }

    #[must_use]
    pub const fn abi(&self) -> &ContractABI {
        &self.abi
    }

    #[must_use]
    pub fn contract_name(&self) -> &str {
        &self.abi.contract_name
    }

    #[must_use]
    pub fn find_get_method(&self, name: &str) -> Option<&ABIGetMethod> {
        self.abi
            .get_methods
            .iter()
            .find(|method| method.name == name)
    }

    #[must_use]
    pub fn declaration_type_index(&self, name: &str) -> Option<TyIdx> {
        self.abi
            .declarations
            .iter()
            .find_map(|declaration| match declaration {
                ABIDeclaration::Struct {
                    name: declaration_name,
                    ty_idx,
                    ..
                }
                | ABIDeclaration::Alias {
                    name: declaration_name,
                    ty_idx,
                    ..
                }
                | ABIDeclaration::Enum {
                    name: declaration_name,
                    ty_idx,
                    ..
                } if declaration_name == name => Some(*ty_idx),
                _ => None,
            })
    }

    pub fn register_custom_codec(
        &mut self,
        type_name: impl Into<String>,
        pack: Option<DynamicPackFn>,
        unpack: Option<DynamicUnpackFn>,
    ) -> Result<(), DynamicError> {
        let type_name = type_name.into();
        if self.custom_codecs.contains_key(&type_name) {
            return Err(DynamicError::InvalidAbi(format!(
                "custom pack/unpack for '{type_name}' already registered"
            )));
        }
        self.custom_codecs
            .insert(type_name, DynamicCustomCodec { pack, unpack });
        Ok(())
    }

    pub fn pack_to_cell(&self, ty_idx: TyIdx, value: &DynamicValue) -> Result<Cell, DynamicError> {
        let mut builder = CellBuilder::new();
        let field_path = self.root_field_path(ty_idx);
        self.pack_type(&field_path, ty_idx, value, &mut builder, None)?;
        Ok(builder.build()?)
    }

    pub fn unpack_from_cell(
        &self,
        ty_idx: TyIdx,
        cell: &Cell,
    ) -> Result<DynamicValue, DynamicError> {
        let mut slice = cell.as_slice()?;
        let field_path = self.root_field_path(ty_idx);
        self.unpack_type(&field_path, ty_idx, &mut slice, None)
    }

    pub fn pack_into_builder(
        &self,
        ty_idx: TyIdx,
        value: &DynamicValue,
        builder: &mut CellBuilder,
    ) -> Result<(), DynamicError> {
        let field_path = self.root_field_path(ty_idx);
        self.pack_type(&field_path, ty_idx, value, builder, None)
    }

    pub fn unpack_from_slice(
        &self,
        ty_idx: TyIdx,
        slice: &mut CellSlice<'_>,
    ) -> Result<DynamicValue, DynamicError> {
        let field_path = self.root_field_path(ty_idx);
        self.unpack_type(&field_path, ty_idx, slice, None)
    }

    fn root_field_path(&self, ty_idx: TyIdx) -> String {
        match self.ty(ty_idx) {
            Ok(Ty::StructRef { struct_name, .. }) => struct_name.clone(),
            _ => "self".to_owned(),
        }
    }

    fn ty(&self, ty_idx: TyIdx) -> Result<&Ty, DynamicError> {
        self.abi.ty_by_idx(ty_idx).ok_or_else(|| {
            DynamicError::InvalidAbi(format!("ABI references unknown type index {ty_idx}"))
        })
    }

    fn declaration(&self, name: &str) -> Result<&ABIDeclaration, DynamicError> {
        self.abi
            .declarations
            .iter()
            .find(|declaration| declaration_name(declaration) == name)
            .ok_or_else(|| {
                DynamicError::InvalidAbi(format!("ABI declaration '{name}' was not found"))
            })
    }

    fn struct_fields(
        &self,
        ty_idx: TyIdx,
        for_stack: bool,
    ) -> Result<Vec<ResolvedField>, DynamicError> {
        let Ty::StructRef { struct_name, .. } = self.ty(ty_idx)? else {
            return Err(DynamicError::InvalidAbi(format!(
                "expected StructRef at type index {ty_idx}"
            )));
        };
        let ABIDeclaration::Struct { fields, .. } = self.declaration(struct_name)? else {
            return Err(DynamicError::InvalidAbi(format!(
                "declaration '{struct_name}' is not a struct"
            )));
        };
        let instantiation = self
            .abi
            .struct_instantiations
            .iter()
            .find(|instantiation| instantiation.ty_idx == ty_idx);
        if let Some(instantiation) = instantiation
            && instantiation.monomorphic_fields_ty_idx.len() != fields.len()
        {
            return Err(DynamicError::InvalidAbi(format!(
                "struct instantiation '{struct_name}' has {} fields, expected {}",
                instantiation.monomorphic_fields_ty_idx.len(),
                fields.len()
            )));
        }

        fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let monomorphic_ty_idx = instantiation
                    .map_or(field.ty_idx, |value| value.monomorphic_fields_ty_idx[index]);
                let (ty_idx, union_label_ty_idx) = if for_stack {
                    (monomorphic_ty_idx, instantiation.map(|_| field.ty_idx))
                } else if let Some(client_ty_idx) = field.client_ty_idx {
                    (client_ty_idx, None)
                } else {
                    (monomorphic_ty_idx, instantiation.map(|_| field.ty_idx))
                };
                Ok(ResolvedField {
                    name: field.name.clone(),
                    ty_idx,
                    union_label_ty_idx,
                })
            })
            .collect()
    }

    fn alias_target(&self, ty_idx: TyIdx) -> Result<ResolvedAliasTarget, DynamicError> {
        let Ty::AliasRef { alias_name, .. } = self.ty(ty_idx)? else {
            return Err(DynamicError::InvalidAbi(format!(
                "expected AliasRef at type index {ty_idx}"
            )));
        };
        let ABIDeclaration::Alias { target_ty_idx, .. } = self.declaration(alias_name)? else {
            return Err(DynamicError::InvalidAbi(format!(
                "declaration '{alias_name}' is not an alias"
            )));
        };
        if let Some(instantiation) = self
            .abi
            .alias_instantiations
            .iter()
            .find(|instantiation| instantiation.ty_idx == ty_idx)
        {
            return Ok(ResolvedAliasTarget {
                ty_idx: instantiation.monomorphic_target_ty_idx,
                union_label_ty_idx: Some(*target_ty_idx),
            });
        }
        Ok(ResolvedAliasTarget {
            ty_idx: *target_ty_idx,
            union_label_ty_idx: None,
        })
    }

    fn union_variants(
        &self,
        variants: &[UnionVariant],
        union_label_ty_idx: Option<TyIdx>,
    ) -> Result<Vec<ResolvedUnionVariant>, DynamicError> {
        let generic_variants =
            union_label_ty_idx.and_then(|ty_idx| match self.ty(ty_idx).ok()? {
                Ty::Union {
                    variants: label_variants,
                    ..
                } if label_variants.len() == variants.len() => Some(
                    label_variants
                        .iter()
                        .map(|variant| variant.variant_ty_idx)
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            });
        let label_type_indices = variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                generic_variants
                    .as_ref()
                    .map_or(variant.variant_ty_idx, |indices| indices[index])
            })
            .collect::<Vec<_>>();
        let simple_labels = label_type_indices
            .iter()
            .map(|ty_idx| self.simple_union_label(*ty_idx))
            .collect::<Result<Vec<_>, _>>()?;
        let has_duplicates = simple_labels
            .iter()
            .enumerate()
            .any(|(index, label)| simple_labels[..index].contains(label));

        variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                let label_ty_idx = label_type_indices[index];
                let is_null = matches!(self.ty(label_ty_idx)?, Ty::NullLiteral);
                Ok(ResolvedUnionVariant {
                    variant_ty_idx: variant.variant_ty_idx,
                    prefix_num: variant.prefix_num,
                    prefix_len: variant.prefix_len,
                    prefix_is_implicit: variant.is_prefix_implicit.unwrap_or(false),
                    stack_type_id: variant.stack_type_id,
                    stack_width: variant.stack_width,
                    label: if is_null {
                        String::new()
                    } else if has_duplicates {
                        render_ty(&self.abi, label_ty_idx)
                    } else {
                        simple_labels[index].clone()
                    },
                    has_value_field: !is_null
                        && (has_duplicates || !self.type_has_own_label(label_ty_idx)?),
                })
            })
            .collect()
    }

    fn simple_union_label(&self, ty_idx: TyIdx) -> Result<String, DynamicError> {
        Ok(match self.ty(ty_idx)? {
            Ty::Int => "int".to_owned(),
            Ty::IntN { n } => format!("int{n}"),
            Ty::UintN { n } => format!("uint{n}"),
            Ty::VarintN { n } => format!("varint{n}"),
            Ty::VaruintN { n } => format!("varuint{n}"),
            Ty::Coins => "coins".to_owned(),
            Ty::Bool => "bool".to_owned(),
            Ty::Cell => "cell".to_owned(),
            Ty::Builder => "builder".to_owned(),
            Ty::Slice => "slice".to_owned(),
            Ty::String => "string".to_owned(),
            Ty::Remaining => "RemainingBitsAndRefs".to_owned(),
            Ty::Address => "address".to_owned(),
            Ty::AddressOpt => "address?".to_owned(),
            Ty::AddressExt => "ext_address".to_owned(),
            Ty::AddressAny => "any_address".to_owned(),
            Ty::BitsN { n } => format!("bits{n}"),
            Ty::NullLiteral => "null".to_owned(),
            Ty::Callable => "callable".to_owned(),
            Ty::Void => "void".to_owned(),
            Ty::Unknown => "unknown".to_owned(),
            Ty::Nullable { inner_ty_idx, .. } => {
                format!("{}?", self.simple_union_label(*inner_ty_idx)?)
            }
            Ty::CellOf { .. } => "Cell".to_owned(),
            Ty::ArrayOf { .. } => "array".to_owned(),
            Ty::LispListOf { .. } => "lisp_list".to_owned(),
            Ty::Tensor { .. } => "tensor".to_owned(),
            Ty::ShapedTuple { .. } => "shaped".to_owned(),
            Ty::MapKV { .. } => "map".to_owned(),
            Ty::EnumRef { enum_name } => enum_name.clone(),
            Ty::StructRef { struct_name, .. } => struct_name.clone(),
            Ty::AliasRef { .. } => self.simple_union_label(self.alias_target(ty_idx)?.ty_idx)?,
            Ty::GenericT { name_t } => name_t.clone(),
            Ty::Union { variants, .. } => variants
                .iter()
                .map(|variant| self.simple_union_label(variant.variant_ty_idx))
                .collect::<Result<Vec<_>, _>>()?
                .join("|"),
        })
    }

    fn type_has_own_label(&self, ty_idx: TyIdx) -> Result<bool, DynamicError> {
        Ok(match self.ty(ty_idx)? {
            Ty::StructRef { .. } => true,
            Ty::AliasRef { .. } => self.type_has_own_label(self.alias_target(ty_idx)?.ty_idx)?,
            _ => false,
        })
    }

    fn invalid_input<T>(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        reason: impl Into<String>,
    ) -> Result<T, DynamicError> {
        Err(DynamicError::InvalidInput {
            field_path: field_path.to_owned(),
            expected: render_ty(&self.abi, ty_idx),
            reason: reason.into(),
        })
    }

    fn number<'value>(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        value: &'value DynamicValue,
    ) -> Result<&'value BigInt, DynamicError> {
        match value {
            DynamicValue::Number(value) => Ok(value),
            _ => self.invalid_input(field_path, ty_idx, "not a number"),
        }
    }

    fn fixed_number<'value>(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        value: &'value DynamicValue,
        bit_width: u32,
        signed: bool,
    ) -> Result<&'value BigInt, DynamicError> {
        let value = self.number(field_path, ty_idx, value)?;
        let in_range = if bit_width == 0 {
            value == &BigInt::from(0)
        } else if signed {
            let bound = BigInt::from(1) << (bit_width - 1);
            value >= &-bound.clone() && value < &bound
        } else {
            let bound = BigInt::from(1) << bit_width;
            value >= &BigInt::from(0) && value < &bound
        };
        if in_range {
            return Ok(value);
        }

        let reason = format!("value is out of range for {bit_width} bits. Got {value}");
        if field_path == "self" {
            Err(DynamicError::Abi(AbiError::InvalidData(reason)))
        } else {
            self.invalid_input(field_path, ty_idx, reason)
        }
    }

    fn boolean(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        value: &DynamicValue,
    ) -> Result<bool, DynamicError> {
        match value {
            DynamicValue::Bool(value) => Ok(*value),
            _ => self.invalid_input(field_path, ty_idx, "not a boolean"),
        }
    }

    fn string<'value>(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        value: &'value DynamicValue,
    ) -> Result<&'value str, DynamicError> {
        match value {
            DynamicValue::String(value) => Ok(value),
            _ => self.invalid_input(field_path, ty_idx, "not a string"),
        }
    }

    fn array<'value>(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        value: &'value DynamicValue,
        expected_len: Option<usize>,
    ) -> Result<&'value [DynamicValue], DynamicError> {
        let DynamicValue::Array(values) = value else {
            return self.invalid_input(field_path, ty_idx, "not an array");
        };
        if let Some(expected_len) = expected_len
            && values.len() != expected_len
        {
            return self.invalid_input(
                field_path,
                ty_idx,
                format!("expected {expected_len} elements, got {}", values.len()),
            );
        }
        Ok(values)
    }

    fn object<'value>(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        value: &'value DynamicValue,
    ) -> Result<&'value [(String, DynamicValue)], DynamicError> {
        match value {
            DynamicValue::Object(fields) => Ok(fields),
            _ => self.invalid_input(field_path, ty_idx, "not an object"),
        }
    }

    fn property<'value>(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        value: &'value DynamicValue,
        property_name: &str,
    ) -> Result<&'value DynamicValue, DynamicError> {
        let DynamicValue::Object(fields) = value else {
            return Err(DynamicError::InvalidInput {
                field_path: field_path.to_owned(),
                expected: render_ty(&self.abi, ty_idx),
                reason: format!("not an object with property {property_name}"),
            });
        };
        fields
            .iter()
            .find_map(|(name, value)| (name == property_name).then_some(value))
            .ok_or_else(|| DynamicError::InvalidInput {
                field_path: field_path.to_owned(),
                expected: render_ty(&self.abi, ty_idx),
                reason: format!("not an object with property {property_name}"),
            })
    }

    fn custom_pack(
        &self,
        type_name: &str,
        field_path: &str,
        value: &DynamicValue,
        builder: &mut CellBuilder,
    ) -> Result<(), DynamicError> {
        let Some(pack) = self
            .custom_codecs
            .get(type_name)
            .and_then(|codec| codec.pack.as_ref())
        else {
            return Err(DynamicError::CannotPack {
                field_path: field_path.to_owned(),
                reason: format!("custom packToBuilder was not registered for '{type_name}'"),
            });
        };
        pack(value, builder)
    }

    fn custom_unpack(
        &self,
        type_name: &str,
        field_path: &str,
        slice: &mut CellSlice<'_>,
    ) -> Result<DynamicValue, DynamicError> {
        let Some(unpack) = self
            .custom_codecs
            .get(type_name)
            .and_then(|codec| codec.unpack.as_ref())
        else {
            return Err(DynamicError::CannotUnpack {
                field_path: field_path.to_owned(),
                reason: format!("custom unpackFromSlice was not registered for '{type_name}'"),
            });
        };
        unpack(slice)
    }

    fn pack_type(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        value: &DynamicValue,
        builder: &mut CellBuilder,
        union_label_ty_idx: Option<TyIdx>,
    ) -> Result<(), DynamicError> {
        let ty = self.ty(ty_idx)?.clone();
        match ty {
            Ty::Int => Err(DynamicError::CannotPack {
                field_path: field_path.to_owned(),
                reason: "type 'int' is not serializable".to_owned(),
            }),
            Ty::IntN { n } => {
                crate::cell::store_fixed_int(
                    builder,
                    self.fixed_number(field_path, ty_idx, value, n, true)?,
                    bits(n)?,
                    true,
                )?;
                Ok(())
            }
            Ty::UintN { n } => {
                crate::cell::store_fixed_int(
                    builder,
                    self.fixed_number(field_path, ty_idx, value, n, false)?,
                    bits(n)?,
                    false,
                )?;
                Ok(())
            }
            Ty::VarintN { n } => {
                crate::cell::store_var_int(
                    builder,
                    self.number(field_path, ty_idx, value)?,
                    varint_len_bits(n)?,
                    true,
                )?;
                Ok(())
            }
            Ty::VaruintN { n } => {
                crate::cell::store_var_int(
                    builder,
                    self.number(field_path, ty_idx, value)?,
                    varint_len_bits(n)?,
                    false,
                )?;
                Ok(())
            }
            Ty::Coins => {
                crate::cell::store_var_int(
                    builder,
                    self.number(field_path, ty_idx, value)?,
                    4,
                    false,
                )?;
                Ok(())
            }
            Ty::Bool => {
                builder.store_bit(self.boolean(field_path, ty_idx, value)?)?;
                Ok(())
            }
            Ty::Cell => {
                let DynamicValue::Cell(cell) = value else {
                    return self.invalid_input(field_path, ty_idx, "not a cell");
                };
                builder.store_reference(cell.clone())?;
                Ok(())
            }
            Ty::Builder => {
                let DynamicValue::Builder(cell) = value else {
                    return self.invalid_input(field_path, ty_idx, "not a builder");
                };
                builder.store_slice(cell.as_slice()?)?;
                Ok(())
            }
            Ty::Slice | Ty::Remaining => {
                let DynamicValue::Slice(slice) = value else {
                    return self.invalid_input(field_path, ty_idx, "not a slice");
                };
                crate::cell::store_slice(builder, slice)?;
                Ok(())
            }
            Ty::String => {
                crate::cell::store_string(builder, self.string(field_path, ty_idx, value)?)?;
                Ok(())
            }
            Ty::Address => {
                let DynamicValue::Address(address) = value else {
                    return self.invalid_input(field_path, ty_idx, "not an address");
                };
                crate::cell::store_tlb(builder, address)?;
                Ok(())
            }
            Ty::AddressOpt => {
                let address = match value {
                    DynamicValue::Null | DynamicValue::AddressNone => AnyAddr::None,
                    DynamicValue::Address(IntAddr::Std(address)) => AnyAddr::Std(address.clone()),
                    DynamicValue::Address(IntAddr::Var(address)) => AnyAddr::Var(address.clone()),
                    _ => return self.invalid_input(field_path, ty_idx, "not an address or null"),
                };
                crate::cell::store_tlb(builder, &address)?;
                Ok(())
            }
            Ty::AddressExt => {
                let DynamicValue::ExtAddress(address) = value else {
                    return self.invalid_input(field_path, ty_idx, "not an external address");
                };
                crate::cell::store_tlb(builder, &AnyAddr::Ext(address.clone()))?;
                Ok(())
            }
            Ty::AddressAny => {
                let address = match value {
                    DynamicValue::AddressNone => AnyAddr::None,
                    DynamicValue::Address(IntAddr::Std(address)) => AnyAddr::Std(address.clone()),
                    DynamicValue::Address(IntAddr::Var(address)) => AnyAddr::Var(address.clone()),
                    DynamicValue::ExtAddress(address) => AnyAddr::Ext(address.clone()),
                    _ => return self.invalid_input(field_path, ty_idx, "not an address"),
                };
                crate::cell::store_tlb(builder, &address)?;
                Ok(())
            }
            Ty::BitsN { n } => {
                let DynamicValue::Bits(value) = value else {
                    return self.invalid_input(field_path, ty_idx, "not a bit slice");
                };
                crate::cell::store_bits(builder, value, bits(n)?)?;
                Ok(())
            }
            Ty::NullLiteral => {
                if matches!(value, DynamicValue::Null) {
                    Ok(())
                } else {
                    self.invalid_input(field_path, ty_idx, "not null")
                }
            }
            Ty::Nullable { inner_ty_idx, .. } => {
                if matches!(value, DynamicValue::Null) {
                    builder.store_bit_zero()?;
                } else {
                    builder.store_bit_one()?;
                    self.pack_type(field_path, inner_ty_idx, value, builder, None)?;
                }
                Ok(())
            }
            Ty::CellOf { inner_ty_idx } => {
                let inner = self.property(field_path, ty_idx, value, "ref")?;
                let mut reference = CellBuilder::new();
                self.pack_type(field_path, inner_ty_idx, inner, &mut reference, None)?;
                builder.store_reference(reference.build()?)?;
                Ok(())
            }
            Ty::ArrayOf { inner_ty_idx } => {
                let values = self.array(field_path, ty_idx, value, None)?;
                let length =
                    u8::try_from(values.len()).map_err(|_| DynamicError::InvalidInput {
                        field_path: field_path.to_owned(),
                        expected: render_ty(&self.abi, ty_idx),
                        reason: "array length exceeds 255".to_owned(),
                    })?;
                let mut tail = None;
                for item in values.iter().rev() {
                    let mut chunk = CellBuilder::new();
                    store_maybe_ref(&mut chunk, tail)?;
                    self.pack_type(
                        &format!("{field_path}[ith]"),
                        inner_ty_idx,
                        item,
                        &mut chunk,
                        None,
                    )?;
                    tail = Some(chunk.build()?);
                }
                builder.store_u8(length)?;
                store_maybe_ref(builder, tail)?;
                Ok(())
            }
            Ty::LispListOf { inner_ty_idx } => {
                let values = self.array(field_path, ty_idx, value, None)?;
                let mut tail = Cell::default();
                for (index, item) in values.iter().enumerate() {
                    let mut item_builder = CellBuilder::new();
                    self.pack_type(
                        &format!("{field_path}[{index}]"),
                        inner_ty_idx,
                        item,
                        &mut item_builder,
                        None,
                    )?;
                    item_builder.store_reference(tail)?;
                    tail = item_builder.build()?;
                }
                builder.store_reference(tail)?;
                Ok(())
            }
            Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => {
                let values = self.array(field_path, ty_idx, value, Some(items_ty_idx.len()))?;
                for (index, (item_ty_idx, item)) in items_ty_idx.into_iter().zip(values).enumerate()
                {
                    self.pack_type(
                        &format!("{field_path}[{index}]"),
                        item_ty_idx,
                        item,
                        builder,
                        None,
                    )?;
                }
                Ok(())
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                let root =
                    self.build_map_root(field_path, ty_idx, value, key_ty_idx, value_ty_idx)?;
                root.store_into(builder, <Cell as CellFamily>::empty_context())?;
                Ok(())
            }
            Ty::EnumRef { enum_name } => {
                let ABIDeclaration::Enum {
                    encoded_as_ty_idx,
                    custom_pack_unpack,
                    ..
                } = self.declaration(&enum_name)?
                else {
                    return Err(DynamicError::InvalidAbi(format!(
                        "declaration '{enum_name}' is not an enum"
                    )));
                };
                self.number(field_path, ty_idx, value)?;
                if uses_custom_pack(custom_pack_unpack.as_ref()) {
                    self.custom_pack(&enum_name, field_path, value, builder)
                } else {
                    self.pack_type(field_path, *encoded_as_ty_idx, value, builder, None)
                }
            }
            Ty::StructRef { struct_name, .. } => {
                let ABIDeclaration::Struct {
                    prefix,
                    custom_pack_unpack,
                    ..
                } = self.declaration(&struct_name)?
                else {
                    return Err(DynamicError::InvalidAbi(format!(
                        "declaration '{struct_name}' is not a struct"
                    )));
                };
                if uses_custom_pack(custom_pack_unpack.as_ref()) {
                    return self.custom_pack(&struct_name, field_path, value, builder);
                }
                self.object(field_path, ty_idx, value)?;
                if let Some(prefix) = prefix {
                    store_prefix(builder, prefix.prefix_num, prefix.prefix_len)?;
                }
                let undefined = DynamicValue::Void;
                for field in self.struct_fields(ty_idx, false)? {
                    let field_value = value.field(&field.name).unwrap_or(&undefined);
                    let nested_path = format!("{field_path}.{}", field.name);
                    if let Err(error) = self.pack_type(
                        &nested_path,
                        field.ty_idx,
                        field_value,
                        builder,
                        field.union_label_ty_idx,
                    ) {
                        return match error {
                            DynamicError::InvalidInput { .. } => Err(error),
                            other => {
                                self.invalid_input(&nested_path, field.ty_idx, other.to_string())
                            }
                        };
                    }
                }
                Ok(())
            }
            Ty::AliasRef { alias_name, .. } => {
                let ABIDeclaration::Alias {
                    custom_pack_unpack, ..
                } = self.declaration(&alias_name)?
                else {
                    return Err(DynamicError::InvalidAbi(format!(
                        "declaration '{alias_name}' is not an alias"
                    )));
                };
                if uses_custom_pack(custom_pack_unpack.as_ref()) {
                    self.custom_pack(&alias_name, field_path, value, builder)
                } else {
                    let target = self.alias_target(ty_idx)?;
                    self.pack_type(
                        field_path,
                        target.ty_idx,
                        value,
                        builder,
                        target.union_label_ty_idx,
                    )
                }
            }
            Ty::Union { variants, .. } => {
                let variants = self.union_variants(&variants, union_label_ty_idx)?;
                if let Some(null_variant) = variants
                    .iter()
                    .find(|variant| matches!(self.ty(variant.variant_ty_idx), Ok(Ty::NullLiteral)))
                    && matches!(value, DynamicValue::Null)
                {
                    store_prefix(
                        builder,
                        null_variant.prefix_num,
                        i32::try_from(null_variant.prefix_len).map_err(|_| {
                            DynamicError::InvalidAbi("union prefix width exceeds i32".to_owned())
                        })?,
                    )?;
                    return Ok(());
                }

                let tag_value = self.property(field_path, ty_idx, value, "$")?;
                let DynamicValue::String(tag) = tag_value else {
                    return self.invalid_input(
                        field_path,
                        ty_idx,
                        format!(
                            "non-existing union variant for $ = '{}'",
                            js_interpolation(tag_value)
                        ),
                    );
                };
                let Some(active_variant) = variants.iter().find(|variant| &variant.label == tag)
                else {
                    return self.invalid_input(
                        field_path,
                        ty_idx,
                        format!("non-existing union variant for $ = '{tag}'"),
                    );
                };
                let actual_value = if active_variant.has_value_field {
                    let Some(value) = value.field("value") else {
                        return self.invalid_input(
                            field_path,
                            ty_idx,
                            "expected {$,value} but field 'value' not provided",
                        );
                    };
                    value
                } else {
                    value
                };
                if active_variant.prefix_is_implicit && active_variant.prefix_len > 0 {
                    store_prefix(
                        builder,
                        active_variant.prefix_num,
                        i32::try_from(active_variant.prefix_len).map_err(|_| {
                            DynamicError::InvalidAbi("union prefix width exceeds i32".to_owned())
                        })?,
                    )?;
                }
                self.pack_type(
                    &format!("{field_path}#{tag}"),
                    active_variant.variant_ty_idx,
                    actual_value,
                    builder,
                    None,
                )
            }
            Ty::Void => Ok(()),
            Ty::Callable | Ty::Unknown | Ty::GenericT { .. } => Err(DynamicError::CannotPack {
                field_path: field_path.to_owned(),
                reason: format!(
                    "type '{}' is not serializable",
                    render_ty(&self.abi, ty_idx)
                ),
            }),
        }
    }

    fn unpack_type(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        slice: &mut CellSlice<'_>,
        union_label_ty_idx: Option<TyIdx>,
    ) -> Result<DynamicValue, DynamicError> {
        let ty = self.ty(ty_idx)?.clone();
        match ty {
            Ty::Int => Err(DynamicError::CannotUnpack {
                field_path: field_path.to_owned(),
                reason: "type 'int' is not serializable".to_owned(),
            }),
            Ty::IntN { n } => Ok(DynamicValue::Number(crate::cell::load_fixed_int(
                slice,
                bits(n)?,
                true,
            )?)),
            Ty::UintN { n } => Ok(DynamicValue::Number(crate::cell::load_fixed_int(
                slice,
                bits(n)?,
                false,
            )?)),
            Ty::VarintN { n } => Ok(DynamicValue::Number(crate::cell::load_var_int(
                slice,
                varint_len_bits(n)?,
                true,
            )?)),
            Ty::VaruintN { n } => Ok(DynamicValue::Number(crate::cell::load_var_int(
                slice,
                varint_len_bits(n)?,
                false,
            )?)),
            Ty::Coins => Ok(DynamicValue::Number(crate::cell::load_var_int(
                slice, 4, false,
            )?)),
            Ty::Bool => Ok(DynamicValue::Bool(slice.load_bit()?)),
            Ty::Cell => Ok(DynamicValue::Cell(slice.load_reference_cloned()?)),
            Ty::Builder => Err(DynamicError::CannotUnpack {
                field_path: field_path.to_owned(),
                reason: "type 'builder' is not serializable".to_owned(),
            }),
            Ty::Slice => Err(DynamicError::CannotUnpack {
                field_path: field_path.to_owned(),
                reason: "type 'slice' is not serializable".to_owned(),
            }),
            Ty::String => Ok(DynamicValue::String(crate::cell::load_string(slice)?)),
            Ty::Remaining => Ok(DynamicValue::Slice(crate::cell::load_remaining(slice)?)),
            Ty::Address => Ok(DynamicValue::Address(IntAddr::load_from(slice)?)),
            Ty::AddressOpt => match AnyAddr::load_from(slice)? {
                AnyAddr::None => Ok(DynamicValue::Null),
                AnyAddr::Std(address) => Ok(DynamicValue::Address(IntAddr::Std(address))),
                AnyAddr::Var(address) => Ok(DynamicValue::Address(IntAddr::Var(address))),
                AnyAddr::Ext(_) => Err(DynamicError::CannotUnpack {
                    field_path: field_path.to_owned(),
                    reason: "expected internal address or null for addressOpt".to_owned(),
                }),
            },
            Ty::AddressExt => match AnyAddr::load_from(slice)? {
                AnyAddr::Ext(address) => Ok(DynamicValue::ExtAddress(address)),
                _ => Err(DynamicError::CannotUnpack {
                    field_path: field_path.to_owned(),
                    reason: "expected external address for addressExt".to_owned(),
                }),
            },
            Ty::AddressAny => Ok(match AnyAddr::load_from(slice)? {
                AnyAddr::None => DynamicValue::AddressNone,
                AnyAddr::Std(address) => DynamicValue::Address(IntAddr::Std(address)),
                AnyAddr::Var(address) => DynamicValue::Address(IntAddr::Var(address)),
                AnyAddr::Ext(address) => DynamicValue::ExtAddress(address),
            }),
            Ty::BitsN { n } => Ok(DynamicValue::Bits(crate::cell::load_bits(slice, bits(n)?)?)),
            Ty::NullLiteral => Ok(DynamicValue::Null),
            Ty::Nullable { inner_ty_idx, .. } => {
                if slice.load_bit()? {
                    self.unpack_type(field_path, inner_ty_idx, slice, None)
                } else {
                    Ok(DynamicValue::Null)
                }
            }
            Ty::CellOf { inner_ty_idx } => {
                let cell = slice.load_reference_cloned()?;
                let mut inner_slice = cell.as_slice()?;
                let value = self.unpack_type(field_path, inner_ty_idx, &mut inner_slice, None)?;
                Ok(DynamicValue::reference(value))
            }
            Ty::ArrayOf { inner_ty_idx } => {
                let expected = usize::from(slice.load_u8()?);
                let mut head = load_maybe_ref(slice)?;
                let mut values = Vec::with_capacity(expected);
                while let Some(cell) = head {
                    let mut chunk = cell.as_slice()?;
                    head = load_maybe_ref(&mut chunk)?;
                    while chunk.size_bits() != 0 || chunk.size_refs() != 0 {
                        values.push(self.unpack_type(
                            &format!("{field_path}[ith]"),
                            inner_ty_idx,
                            &mut chunk,
                            None,
                        )?);
                    }
                }
                if values.len() != expected {
                    return Err(DynamicError::CannotUnpack {
                        field_path: field_path.to_owned(),
                        reason: format!(
                            "mismatch array binary data: expected {expected} elements, got {}",
                            values.len()
                        ),
                    });
                }
                Ok(DynamicValue::Array(values))
            }
            Ty::LispListOf { inner_ty_idx } => {
                let mut head = slice.load_reference_cloned()?;
                let mut values = Vec::new();
                while head.reference_count() != 0 {
                    let mut item = head.as_slice()?;
                    let tail = item.load_reference_cloned()?;
                    let value = self.unpack_type(
                        &format!("{field_path}[ith]"),
                        inner_ty_idx,
                        &mut item,
                        None,
                    )?;
                    crate::cell::ensure_empty(&item)?;
                    values.insert(0, value);
                    head = tail;
                }
                Ok(DynamicValue::Array(values))
            }
            Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => {
                let mut values = Vec::with_capacity(items_ty_idx.len());
                for (index, item_ty_idx) in items_ty_idx.into_iter().enumerate() {
                    values.push(self.unpack_type(
                        &format!("{field_path}[{index}]"),
                        item_ty_idx,
                        slice,
                        None,
                    )?);
                }
                Ok(DynamicValue::Array(values))
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                let root = Option::<Cell>::load_from(slice)?;
                self.unpack_map_root(field_path, key_ty_idx, value_ty_idx, root.as_ref())
            }
            Ty::EnumRef { enum_name } => {
                let ABIDeclaration::Enum {
                    encoded_as_ty_idx,
                    custom_pack_unpack,
                    ..
                } = self.declaration(&enum_name)?
                else {
                    return Err(DynamicError::InvalidAbi(format!(
                        "declaration '{enum_name}' is not an enum"
                    )));
                };
                if uses_custom_unpack(custom_pack_unpack.as_ref()) {
                    self.custom_unpack(&enum_name, field_path, slice)
                } else {
                    self.unpack_type(field_path, *encoded_as_ty_idx, slice, None)
                }
            }
            Ty::StructRef { struct_name, .. } => {
                let ABIDeclaration::Struct {
                    prefix,
                    custom_pack_unpack,
                    ..
                } = self.declaration(&struct_name)?
                else {
                    return Err(DynamicError::InvalidAbi(format!(
                        "declaration '{struct_name}' is not a struct"
                    )));
                };
                if uses_custom_unpack(custom_pack_unpack.as_ref()) {
                    return self.custom_unpack(&struct_name, field_path, slice);
                }
                if let Some(prefix) = prefix {
                    let prefix_len = u16::try_from(prefix.prefix_len).map_err(|_| {
                        DynamicError::InvalidAbi(format!(
                            "invalid serialization prefix width {}",
                            prefix.prefix_len
                        ))
                    })?;
                    if !matches_prefix(slice, prefix.prefix_num, usize::from(prefix_len))? {
                        return Err(DynamicError::CannotUnpack {
                            field_path: field_path.to_owned(),
                            reason: format!(
                                "incorrect prefix for '{struct_name}', expected {}",
                                crate::cell::format_prefix(prefix.prefix_num, prefix_len)
                            ),
                        });
                    }
                    slice.skip_first(prefix_len, 0)?;
                }
                let mut fields = vec![("$".to_owned(), DynamicValue::String(struct_name.clone()))];
                for field in self.struct_fields(ty_idx, false)? {
                    let nested_path = format!("{field_path}.{}", field.name);
                    let value = self
                        .unpack_type(&nested_path, field.ty_idx, slice, field.union_label_ty_idx)
                        .map_err(|error| match error {
                            DynamicError::CannotUnpack { .. } => error,
                            other => DynamicError::CannotUnpack {
                                field_path: nested_path,
                                reason: other.to_string(),
                            },
                        })?;
                    fields.push((field.name, value));
                }
                Ok(DynamicValue::Object(fields))
            }
            Ty::AliasRef { alias_name, .. } => {
                let ABIDeclaration::Alias {
                    custom_pack_unpack, ..
                } = self.declaration(&alias_name)?
                else {
                    return Err(DynamicError::InvalidAbi(format!(
                        "declaration '{alias_name}' is not an alias"
                    )));
                };
                if uses_custom_unpack(custom_pack_unpack.as_ref()) {
                    self.custom_unpack(&alias_name, field_path, slice)
                } else {
                    let target = self.alias_target(ty_idx)?;
                    self.unpack_type(field_path, target.ty_idx, slice, target.union_label_ty_idx)
                }
            }
            Ty::GenericT { name_t } => Err(DynamicError::CannotUnpack {
                field_path: field_path.to_owned(),
                reason: format!("unexpected genericT={name_t} at {field_path}"),
            }),
            Ty::Union { variants, .. } => {
                let variants = self.union_variants(&variants, union_label_ty_idx)?;
                let has_void = variants
                    .last()
                    .is_some_and(|variant| matches!(self.ty(variant.variant_ty_idx), Ok(Ty::Void)));
                if has_void && variants.len() == 2 {
                    if slice.size_bits() == 0 && slice.size_refs() == 0 {
                        return Ok(wrap_union_value(&variants[1], DynamicValue::Void));
                    }
                    let variant = &variants[0];
                    if variant.prefix_is_implicit {
                        slice.skip_first(prefix_bits(variant.prefix_len)?, 0)?;
                    }
                    let value = self.unpack_type(
                        &format!("{field_path}#{}", variant.label),
                        variant.variant_ty_idx,
                        slice,
                        None,
                    )?;
                    return Ok(wrap_union_value(variant, value));
                }

                let dispatch_len = variants.len() - usize::from(has_void);
                for variant in variants.iter().take(dispatch_len) {
                    if !matches_prefix(slice, variant.prefix_num, variant.prefix_len)? {
                        continue;
                    }
                    if variant.prefix_is_implicit {
                        slice.skip_first(prefix_bits(variant.prefix_len)?, 0)?;
                    }
                    let value = self.unpack_type(
                        &format!("{field_path}#{}", variant.label),
                        variant.variant_ty_idx,
                        slice,
                        None,
                    )?;
                    return Ok(wrap_union_value(variant, value));
                }
                if has_void && slice.size_bits() == 0 && slice.size_refs() == 0 {
                    return Ok(wrap_union_value(
                        variants.last().expect("void variant was checked"),
                        DynamicValue::Void,
                    ));
                }
                Err(DynamicError::CannotUnpack {
                    field_path: field_path.to_owned(),
                    reason: "none of union prefixes match".to_owned(),
                })
            }
            Ty::Void => Ok(DynamicValue::Void),
            Ty::Callable | Ty::Unknown => Err(DynamicError::CannotUnpack {
                field_path: field_path.to_owned(),
                reason: format!(
                    "type '{}' is not serializable",
                    render_ty(&self.abi, ty_idx)
                ),
            }),
        }
    }

    fn dictionary_key_bits(&self, ty_idx: TyIdx) -> Result<u16, DynamicError> {
        match self.ty(ty_idx)? {
            Ty::IntN { n } | Ty::UintN { n } => bits(*n),
            Ty::Address => Ok(267),
            _ => Err(DynamicError::InvalidAbi(format!(
                "map key type '{}' is not supported; expected intN, uintN, or address",
                render_ty(&self.abi, ty_idx)
            ))),
        }
    }

    fn build_map_root(
        &self,
        field_path: &str,
        map_ty_idx: TyIdx,
        value: &DynamicValue,
        key_ty_idx: TyIdx,
        value_ty_idx: TyIdx,
    ) -> Result<Option<Cell>, DynamicError> {
        let DynamicValue::Map(entries) = value else {
            return self.invalid_input(field_path, map_ty_idx, "not a map");
        };
        let key_bits = self.dictionary_key_bits(key_ty_idx)?;
        let mut root = None;
        for (key, value) in entries {
            let mut key_builder = CellBuilder::new();
            self.pack_type(field_path, key_ty_idx, key, &mut key_builder, None)?;
            if key_builder.size_bits() != key_bits || key_builder.size_refs() != 0 {
                return self.invalid_input(
                    field_path,
                    key_ty_idx,
                    format!("dictionary key must contain {key_bits} bits and no refs"),
                );
            }
            let key_cell = key_builder.build()?;
            let mut key_slice = key_cell.as_slice()?;
            let mut value_builder = CellBuilder::new();
            self.pack_type(field_path, value_ty_idx, value, &mut value_builder, None)?;
            dict::dict_insert(
                &mut root,
                &mut key_slice,
                key_bits,
                &value_builder,
                SetMode::Set,
                <Cell as CellFamily>::empty_context(),
            )?;
        }
        Ok(root)
    }

    fn unpack_map_root(
        &self,
        field_path: &str,
        key_ty_idx: TyIdx,
        value_ty_idx: TyIdx,
        root: Option<&Cell>,
    ) -> Result<DynamicValue, DynamicError> {
        let key_bits = self.dictionary_key_bits(key_ty_idx)?;
        let root = root.cloned();
        let mut entries = Vec::new();
        for entry in dict::RawIter::new(&root, key_bits) {
            let (key_data, mut value_slice) = entry?;
            let mut key_slice = key_data.as_data_slice();
            let key = self.unpack_type(field_path, key_ty_idx, &mut key_slice, None)?;
            crate::cell::ensure_empty(&key_slice)?;
            let value = self.unpack_type(field_path, value_ty_idx, &mut value_slice, None)?;
            crate::cell::ensure_empty(&value_slice)?;
            entries.push((key, value));
        }
        Ok(DynamicValue::Map(entries))
    }

    /// Pack a dynamic value into the TVM tuple representation used by get
    /// method arguments.
    pub fn make_tvm_tuple(
        &self,
        ty_idx: TyIdx,
        value: &DynamicValue,
    ) -> Result<Tuple, DynamicError> {
        Ok(Tuple(self.construct_stack_type(
            "value", ty_idx, value, false, None,
        )?))
    }

    /// Invoke a get method by ABI name and decode its result dynamically.
    pub async fn call_get_method<P: ContractProvider>(
        &self,
        provider: &P,
        address: &StdAddr,
        method_name: &str,
        arguments: &[DynamicValue],
    ) -> Result<DynamicValue, DynamicCallError<P::Error>> {
        let method =
            self.find_get_method(method_name)
                .ok_or_else(|| DynamicError::MethodNotFound {
                    contract: self.abi.contract_name.clone(),
                    method: method_name.to_owned(),
                })?;
        if method.parameters.len() != arguments.len() {
            return Err(DynamicError::InvalidArgumentCount {
                method: method_name.to_owned(),
                expected: method.parameters.len(),
                actual: arguments.len(),
            }
            .into());
        }

        let mut stack_input = Vec::new();
        for (parameter, value) in method.parameters.iter().zip(arguments) {
            stack_input.extend(self.construct_stack_type(
                &parameter.name,
                parameter.ty_idx,
                value,
                false,
                None,
            )?);
        }
        let method_id = method.tvm_method_id;
        let return_ty_idx = method.return_ty_idx;
        let output = provider
            .run_get_method(address, method_id, Tuple(stack_input))
            .await
            .map_err(DynamicCallError::Provider)?;
        let expected_width = calc_width_on_stack(&self.abi, return_ty_idx);
        let mut reader =
            StackReader::from_tuple(output, expected_width).map_err(DynamicError::from)?;
        let value = self.parse_stack_type("result", return_ty_idx, &mut reader, false, None)?;
        reader.ensure_empty().map_err(DynamicError::from)?;
        Ok(value)
    }

    fn make_stack_cell(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        value: &DynamicValue,
        union_label_ty_idx: Option<TyIdx>,
    ) -> Result<Cell, DynamicError> {
        let mut builder = CellBuilder::new();
        self.pack_type(field_path, ty_idx, value, &mut builder, union_label_ty_idx)?;
        Ok(builder.build()?)
    }

    fn construct_stack_type(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        value: &DynamicValue,
        tuple_if_wide: bool,
        union_label_ty_idx: Option<TyIdx>,
    ) -> Result<Vec<TupleItem>, DynamicError> {
        let width = calc_width_on_stack(&self.abi, ty_idx);
        if tuple_if_wide && width != 1 {
            return Ok(vec![TupleItem::Tuple(Tuple(self.construct_stack_type(
                field_path,
                ty_idx,
                value,
                false,
                union_label_ty_idx,
            )?))]);
        }

        let ty = self.ty(ty_idx)?.clone();
        match ty {
            Ty::Int
            | Ty::IntN { .. }
            | Ty::UintN { .. }
            | Ty::VarintN { .. }
            | Ty::VaruintN { .. }
            | Ty::Coins
            | Ty::EnumRef { .. } => Ok(vec![TupleItem::Int(
                self.number(field_path, ty_idx, value)?.clone(),
            )]),
            Ty::Bool => Ok(vec![TupleItem::Int(BigInt::from(
                if self.boolean(field_path, ty_idx, value)? {
                    -1
                } else {
                    0
                },
            ))]),
            Ty::Cell => {
                let DynamicValue::Cell(cell) = value else {
                    return self.invalid_input(field_path, ty_idx, "not a cell");
                };
                Ok(vec![TupleItem::Cell(cell.clone())])
            }
            Ty::Builder => {
                let DynamicValue::Builder(cell) = value else {
                    return self.invalid_input(field_path, ty_idx, "not a builder");
                };
                Ok(vec![TupleItem::Builder(cell.clone())])
            }
            Ty::Slice | Ty::Remaining => {
                let DynamicValue::Slice(slice) = value else {
                    return self.invalid_input(field_path, ty_idx, "not a slice");
                };
                Ok(vec![TupleItem::Slice(owned_slice_to_cell(slice)?)])
            }
            Ty::String => Ok(vec![TupleItem::Cell(crate::cell::string_to_cell(
                self.string(field_path, ty_idx, value)?,
            ))]),
            Ty::Address | Ty::AddressExt | Ty::AddressAny | Ty::BitsN { .. } => {
                Ok(vec![TupleItem::Slice(self.make_stack_cell(
                    field_path,
                    ty_idx,
                    value,
                    union_label_ty_idx,
                )?)])
            }
            Ty::AddressOpt => {
                if matches!(value, DynamicValue::Null | DynamicValue::AddressNone) {
                    Ok(vec![TupleItem::Null])
                } else {
                    Ok(vec![TupleItem::Slice(self.make_stack_cell(
                        field_path,
                        ty_idx,
                        value,
                        union_label_ty_idx,
                    )?)])
                }
            }
            Ty::NullLiteral => {
                if matches!(value, DynamicValue::Null) {
                    Ok(vec![TupleItem::Null])
                } else {
                    self.invalid_input(field_path, ty_idx, "not null")
                }
            }
            Ty::Callable => Err(DynamicError::CannotPack {
                field_path: field_path.to_owned(),
                reason: format!(
                    "type '{}' is not supported on the TVM stack",
                    render_ty(&self.abi, ty_idx)
                ),
            }),
            Ty::Void => Ok(Vec::new()),
            Ty::Unknown => {
                let DynamicValue::Unknown(item) = value else {
                    return self.invalid_input(field_path, ty_idx, "not a raw tuple item");
                };
                Ok(vec![item.clone()])
            }
            Ty::Nullable {
                inner_ty_idx,
                stack_type_id,
                stack_width,
            } => {
                if let Some(stack_type_id) = stack_type_id {
                    let stack_width = stack_width.ok_or_else(|| {
                        DynamicError::InvalidAbi(format!(
                            "wide nullable at type index {ty_idx} has no stack_width"
                        ))
                    })?;
                    if matches!(value, DynamicValue::Null) {
                        let mut result = vec![TupleItem::Null; stack_width.saturating_sub(1)];
                        result.push(TupleItem::Int(BigInt::from(0)));
                        Ok(result)
                    } else {
                        let mut result = self.construct_stack_type(
                            field_path,
                            inner_ty_idx,
                            value,
                            false,
                            None,
                        )?;
                        result.push(TupleItem::Int(BigInt::from(stack_type_id)));
                        Ok(result)
                    }
                } else if matches!(value, DynamicValue::Null) {
                    Ok(vec![TupleItem::Null])
                } else {
                    self.construct_stack_type(field_path, inner_ty_idx, value, false, None)
                }
            }
            Ty::CellOf { inner_ty_idx } => {
                let inner = self.property(field_path, ty_idx, value, "ref")?;
                Ok(vec![TupleItem::Cell(self.make_stack_cell(
                    field_path,
                    inner_ty_idx,
                    inner,
                    None,
                )?)])
            }
            Ty::ArrayOf { inner_ty_idx } => {
                let values = self.array(field_path, ty_idx, value, None)?;
                let mut items = Vec::with_capacity(values.len());
                for item in values {
                    let mut encoded =
                        self.construct_stack_type("ith", inner_ty_idx, item, true, None)?;
                    if encoded.len() != 1 {
                        return Err(DynamicError::InvalidAbi(format!(
                            "array item at type index {inner_ty_idx} did not occupy one tuple item"
                        )));
                    }
                    items.push(encoded.remove(0));
                }
                Ok(vec![TupleItem::Tuple(Tuple(items))])
            }
            Ty::LispListOf { inner_ty_idx } => {
                let values = self.array(field_path, ty_idx, value, None)?;
                let mut tail = TupleItem::Null;
                for item in values.iter().rev() {
                    let mut encoded =
                        self.construct_stack_type("head", inner_ty_idx, item, true, None)?;
                    if encoded.len() != 1 {
                        return Err(DynamicError::InvalidAbi(format!(
                            "lisp_list item at type index {inner_ty_idx} did not occupy one tuple item"
                        )));
                    }
                    tail = TupleItem::Tuple(Tuple(vec![encoded.remove(0), tail]));
                }
                Ok(vec![tail])
            }
            Ty::Tensor { items_ty_idx } => {
                let values = self.array(field_path, ty_idx, value, Some(items_ty_idx.len()))?;
                let mut result = Vec::new();
                for (index, (item_ty_idx, item)) in items_ty_idx.into_iter().zip(values).enumerate()
                {
                    result.extend(self.construct_stack_type(
                        &format!("{field_path}[{index}]"),
                        item_ty_idx,
                        item,
                        false,
                        None,
                    )?);
                }
                Ok(result)
            }
            Ty::ShapedTuple { items_ty_idx } => {
                let values = self.array(field_path, ty_idx, value, Some(items_ty_idx.len()))?;
                let mut result = Vec::with_capacity(items_ty_idx.len());
                for (index, (item_ty_idx, item)) in items_ty_idx.into_iter().zip(values).enumerate()
                {
                    let mut encoded = self.construct_stack_type(
                        &format!("{field_path}[{index}]"),
                        item_ty_idx,
                        item,
                        true,
                        None,
                    )?;
                    if encoded.len() != 1 {
                        return Err(DynamicError::InvalidAbi(format!(
                            "shaped tuple item at type index {item_ty_idx} did not occupy one tuple item"
                        )));
                    }
                    result.push(encoded.remove(0));
                }
                Ok(vec![TupleItem::Tuple(Tuple(result))])
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                let root =
                    self.build_map_root(field_path, ty_idx, value, key_ty_idx, value_ty_idx)?;
                Ok(vec![root.map_or(TupleItem::Null, TupleItem::Cell)])
            }
            Ty::StructRef { .. } => {
                self.object(field_path, ty_idx, value)?;
                let mut result = Vec::new();
                let undefined = DynamicValue::Void;
                for field in self.struct_fields(ty_idx, true)? {
                    let field_value = value.field(&field.name).unwrap_or(&undefined);
                    result.extend(self.construct_stack_type(
                        &format!("{field_path}.{}", field.name),
                        field.ty_idx,
                        field_value,
                        false,
                        field.union_label_ty_idx,
                    )?);
                }
                Ok(result)
            }
            Ty::AliasRef { .. } => {
                let target = self.alias_target(ty_idx)?;
                self.construct_stack_type(
                    field_path,
                    target.ty_idx,
                    value,
                    false,
                    target.union_label_ty_idx,
                )
            }
            Ty::Union {
                variants,
                stack_width,
            } => {
                let stack_width = stack_width.ok_or_else(|| {
                    DynamicError::InvalidAbi(format!(
                        "union at type index {ty_idx} has no stack_width"
                    ))
                })?;
                let variants = self.union_variants(&variants, union_label_ty_idx)?;
                if let Some(null_variant) = variants
                    .iter()
                    .find(|variant| matches!(self.ty(variant.variant_ty_idx), Ok(Ty::NullLiteral)))
                    && matches!(value, DynamicValue::Null)
                {
                    let mut result = vec![TupleItem::Null; stack_width.saturating_sub(1)];
                    result.push(TupleItem::Int(BigInt::from(
                        null_variant.stack_type_id.unwrap_or(0),
                    )));
                    return Ok(result);
                }
                let tag_value = self.property(field_path, ty_idx, value, "$")?;
                let DynamicValue::String(tag) = tag_value else {
                    return self.invalid_input(
                        field_path,
                        ty_idx,
                        format!(
                            "non-existing union variant for $ = '{}'",
                            js_interpolation(tag_value)
                        ),
                    );
                };
                let Some(active_variant) = variants.iter().find(|variant| &variant.label == tag)
                else {
                    return self.invalid_input(
                        field_path,
                        ty_idx,
                        format!("non-existing union variant for $ = '{tag}'"),
                    );
                };
                let variant_width = active_variant.stack_width.ok_or_else(|| {
                    DynamicError::InvalidAbi(format!("union variant '{tag}' has no stack_width"))
                })?;
                let type_id = active_variant.stack_type_id.ok_or_else(|| {
                    DynamicError::InvalidAbi(format!("union variant '{tag}' has no stack_type_id"))
                })?;
                let actual_value = if active_variant.has_value_field {
                    let Some(value) = value.field("value") else {
                        return self.invalid_input(
                            field_path,
                            ty_idx,
                            "expected {$,value} but field 'value' not provided",
                        );
                    };
                    value
                } else {
                    value
                };
                let padding = stack_width.checked_sub(variant_width + 1).ok_or_else(|| {
                    DynamicError::InvalidAbi(format!(
                        "union variant '{tag}' exceeds union stack width"
                    ))
                })?;
                let mut result = vec![TupleItem::Null; padding];
                result.extend(self.construct_stack_type(
                    &format!("{field_path}#{tag}"),
                    active_variant.variant_ty_idx,
                    actual_value,
                    false,
                    None,
                )?);
                result.push(TupleItem::Int(BigInt::from(type_id)));
                Ok(result)
            }
            Ty::GenericT { name_t } => Err(DynamicError::InvalidAbi(format!(
                "unexpected genericT={name_t} at {field_path}"
            ))),
        }
    }

    fn parse_stack_type(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        reader: &mut StackReader,
        untuple_if_wide: bool,
        union_label_ty_idx: Option<TyIdx>,
    ) -> Result<DynamicValue, DynamicError> {
        let width = calc_width_on_stack(&self.abi, ty_idx);
        if untuple_if_wide && width != 1 {
            let mut nested = reader.read_tuple(Some(width))?;
            let value =
                self.parse_stack_type(field_path, ty_idx, &mut nested, false, union_label_ty_idx)?;
            nested.ensure_empty()?;
            return Ok(value);
        }

        let ty = self.ty(ty_idx)?.clone();
        match ty {
            Ty::Int
            | Ty::IntN { .. }
            | Ty::UintN { .. }
            | Ty::VarintN { .. }
            | Ty::VaruintN { .. }
            | Ty::Coins
            | Ty::EnumRef { .. } => Ok(DynamicValue::Number(reader.read_int()?)),
            Ty::Bool => Ok(DynamicValue::Bool(reader.read_bool()?)),
            Ty::Cell => Ok(DynamicValue::Cell(reader.read_cell()?)),
            Ty::Builder => Ok(DynamicValue::Builder(reader.read_builder()?.build()?)),
            Ty::Slice | Ty::Remaining => Ok(DynamicValue::Slice(reader.read_owned_slice()?)),
            Ty::String => Ok(DynamicValue::String(reader.read_string()?)),
            Ty::Address | Ty::AddressExt | Ty::AddressAny | Ty::BitsN { .. } => {
                let cell = reader.read_cell()?;
                let mut slice = cell.as_slice()?;
                let value = self.unpack_type(field_path, ty_idx, &mut slice, None)?;
                crate::cell::ensure_empty(&slice)?;
                Ok(value)
            }
            Ty::AddressOpt => {
                if matches!(reader.peek(0)?, TupleItem::Null) {
                    reader.pop()?;
                    Ok(DynamicValue::Null)
                } else {
                    let cell = reader.read_cell()?;
                    let mut slice = cell.as_slice()?;
                    let value = self.unpack_type(field_path, ty_idx, &mut slice, None)?;
                    crate::cell::ensure_empty(&slice)?;
                    Ok(value)
                }
            }
            Ty::NullLiteral => match reader.pop()? {
                TupleItem::Null => Ok(DynamicValue::Null),
                _ => Err(DynamicError::CannotUnpack {
                    field_path: field_path.to_owned(),
                    reason: "not 'null' on a stack".to_owned(),
                }),
            },
            Ty::Callable => Err(DynamicError::CannotUnpack {
                field_path: field_path.to_owned(),
                reason: format!(
                    "type '{}' is not supported on the TVM stack",
                    render_ty(&self.abi, ty_idx)
                ),
            }),
            Ty::Void => Ok(DynamicValue::Void),
            Ty::Unknown => Ok(DynamicValue::Unknown(reader.pop()?)),
            Ty::Nullable {
                inner_ty_idx,
                stack_type_id,
                stack_width,
            } => {
                if stack_type_id.is_some() {
                    let stack_width = stack_width.ok_or_else(|| {
                        DynamicError::InvalidAbi(format!(
                            "wide nullable at type index {ty_idx} has no stack_width"
                        ))
                    })?;
                    let tag = reader.read_union_tag(stack_width)?;
                    if tag == BigInt::from(0) {
                        reader.skip(stack_width)?;
                        Ok(DynamicValue::Null)
                    } else {
                        let value =
                            self.parse_stack_type(field_path, inner_ty_idx, reader, false, None)?;
                        reader.finish_union_variant()?;
                        Ok(value)
                    }
                } else if matches!(reader.peek(0)?, TupleItem::Null) {
                    reader.pop()?;
                    Ok(DynamicValue::Null)
                } else {
                    self.parse_stack_type(field_path, inner_ty_idx, reader, false, None)
                }
            }
            Ty::CellOf { inner_ty_idx } => {
                let cell = reader.read_cell()?;
                let mut slice = cell.as_slice()?;
                let value = self.unpack_type(field_path, inner_ty_idx, &mut slice, None)?;
                Ok(DynamicValue::reference(value))
            }
            Ty::ArrayOf { inner_ty_idx } => {
                let mut items = reader.read_tuple(None)?;
                let nested = calc_width_on_stack(&self.abi, inner_ty_idx) != 1;
                let mut values = Vec::new();
                while items.remaining() != 0 {
                    values.push(self.parse_stack_type(
                        field_path,
                        inner_ty_idx,
                        &mut items,
                        nested,
                        None,
                    )?);
                }
                Ok(DynamicValue::Array(values))
            }
            Ty::LispListOf { inner_ty_idx } => {
                let mut current = reader.pop()?;
                let mut values = Vec::new();
                loop {
                    match current {
                        TupleItem::Null => break,
                        TupleItem::Tuple(tuple) if tuple.0.len() == 2 => {
                            let mut pair = tuple.0.into_iter();
                            let head = pair.next().expect("pair length was checked");
                            current = pair.next().expect("pair length was checked");
                            let mut head_reader = StackReader::new(vec![head]);
                            values.push(self.parse_stack_type(
                                field_path,
                                inner_ty_idx,
                                &mut head_reader,
                                true,
                                None,
                            )?);
                            head_reader.ensure_empty()?;
                        }
                        _ => {
                            return Err(DynamicError::CannotUnpack {
                                field_path: field_path.to_owned(),
                                reason: "malformed lisp_list on stack".to_owned(),
                            });
                        }
                    }
                }
                Ok(DynamicValue::Array(values))
            }
            Ty::Tensor { items_ty_idx } => {
                let mut values = Vec::with_capacity(items_ty_idx.len());
                for (index, item_ty_idx) in items_ty_idx.into_iter().enumerate() {
                    values.push(self.parse_stack_type(
                        &format!("{field_path}[{index}]"),
                        item_ty_idx,
                        reader,
                        false,
                        None,
                    )?);
                }
                Ok(DynamicValue::Array(values))
            }
            Ty::ShapedTuple { items_ty_idx } => {
                let mut nested = reader.read_tuple(Some(items_ty_idx.len()))?;
                let mut values = Vec::with_capacity(items_ty_idx.len());
                for (index, item_ty_idx) in items_ty_idx.into_iter().enumerate() {
                    values.push(self.parse_stack_type(
                        &format!("{field_path}[{index}]"),
                        item_ty_idx,
                        &mut nested,
                        true,
                        None,
                    )?);
                }
                nested.ensure_empty()?;
                Ok(DynamicValue::Array(values))
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => match reader.pop()? {
                TupleItem::Null => Ok(DynamicValue::Map(Vec::new())),
                TupleItem::Cell(root) | TupleItem::Slice(root) => {
                    self.unpack_map_root(field_path, key_ty_idx, value_ty_idx, Some(&root))
                }
                _ => Err(DynamicError::CannotUnpack {
                    field_path: field_path.to_owned(),
                    reason: "expected dictionary cell or null on TVM stack".to_owned(),
                }),
            },
            Ty::StructRef { struct_name, .. } => {
                let mut fields = vec![("$".to_owned(), DynamicValue::String(struct_name))];
                for field in self.struct_fields(ty_idx, true)? {
                    let value = self.parse_stack_type(
                        &format!("{field_path}.{}", field.name),
                        field.ty_idx,
                        reader,
                        false,
                        field.union_label_ty_idx,
                    )?;
                    fields.push((field.name, value));
                }
                Ok(DynamicValue::Object(fields))
            }
            Ty::AliasRef { .. } => {
                let target = self.alias_target(ty_idx)?;
                self.parse_stack_type(
                    field_path,
                    target.ty_idx,
                    reader,
                    false,
                    target.union_label_ty_idx,
                )
            }
            Ty::Union {
                variants,
                stack_width,
            } => {
                let stack_width = stack_width.ok_or_else(|| {
                    DynamicError::InvalidAbi(format!(
                        "union at type index {ty_idx} has no stack_width"
                    ))
                })?;
                let tag = reader.read_union_tag(stack_width)?;
                let variants = self.union_variants(&variants, union_label_ty_idx)?;
                let Some(variant) = variants.iter().find(|variant| {
                    variant
                        .stack_type_id
                        .is_some_and(|type_id| tag == BigInt::from(type_id))
                }) else {
                    return Err(DynamicError::CannotUnpack {
                        field_path: field_path.to_owned(),
                        reason: format!("unexpected UTag={tag}"),
                    });
                };
                let variant_width = variant.stack_width.ok_or_else(|| {
                    DynamicError::InvalidAbi(format!(
                        "union variant '{}' has no stack_width",
                        variant.label
                    ))
                })?;
                reader.prepare_union_variant(stack_width, variant_width)?;
                let value = self.parse_stack_type(
                    &format!("{field_path}#{}", variant.label),
                    variant.variant_ty_idx,
                    reader,
                    false,
                    None,
                )?;
                reader.finish_union_variant()?;
                Ok(wrap_union_value(variant, value))
            }
            Ty::GenericT { name_t } => Err(DynamicError::InvalidAbi(format!(
                "unexpected genericT={name_t} at {field_path}"
            ))),
        }
    }

    /// Render a TVM tuple according to an ABI type, following upstream's
    /// human-readable debug notation.
    pub fn debug_print_from_stack(
        &self,
        tuple: Tuple,
        ty_idx: TyIdx,
    ) -> Result<String, DynamicError> {
        let expected_width = calc_width_on_stack(&self.abi, ty_idx);
        let mut reader = StackReader::from_tuple(tuple, expected_width)?;
        let result = self.debug_format_stack("self", ty_idx, &mut reader, false, None)?;
        reader.ensure_empty()?;
        Ok(result)
    }

    fn debug_format_stack(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        reader: &mut StackReader,
        untuple_if_wide: bool,
        union_label_ty_idx: Option<TyIdx>,
    ) -> Result<String, DynamicError> {
        let width = calc_width_on_stack(&self.abi, ty_idx);
        if untuple_if_wide && width != 1 {
            let mut nested = reader.read_tuple(Some(width))?;
            let result = self.debug_format_stack(
                field_path,
                ty_idx,
                &mut nested,
                false,
                union_label_ty_idx,
            )?;
            nested.ensure_empty()?;
            return Ok(result);
        }

        let ty = self.ty(ty_idx)?.clone();
        match ty {
            Ty::Int
            | Ty::IntN { .. }
            | Ty::UintN { .. }
            | Ty::VarintN { .. }
            | Ty::VaruintN { .. }
            | Ty::Coins => Ok(reader.read_int()?.to_string()),
            Ty::Bool => Ok(reader.read_bool()?.to_string()),
            Ty::Cell => Ok(format!("cell{{{}}}", print_raw_cell(&reader.read_cell()?)?)),
            Ty::Builder => Ok(format!(
                "builder{{{}}}",
                print_raw_cell(&reader.read_builder()?.build()?)?
            )),
            Ty::Slice | Ty::BitsN { .. } | Ty::Remaining => Ok(format!(
                "slice{{{}}}",
                print_raw_cell(&reader.read_cell()?)?
            )),
            Ty::String => Ok(format!("\"{}\"", reader.read_string()?)),
            Ty::Address | Ty::AddressExt | Ty::AddressAny => {
                let cell = reader.read_cell()?;
                let mut slice = cell.as_slice()?;
                let address = self.unpack_type(field_path, ty_idx, &mut slice, None)?;
                crate::cell::ensure_empty(&slice)?;
                format_address(&address).ok_or_else(|| DynamicError::CannotUnpack {
                    field_path: field_path.to_owned(),
                    reason: "invalid address value".to_owned(),
                })
            }
            Ty::AddressOpt => {
                if matches!(reader.peek(0)?, TupleItem::Null) {
                    reader.pop()?;
                    Ok("null".to_owned())
                } else {
                    let cell = reader.read_cell()?;
                    let mut slice = cell.as_slice()?;
                    let address = self.unpack_type(field_path, ty_idx, &mut slice, None)?;
                    crate::cell::ensure_empty(&slice)?;
                    format_address(&address).ok_or_else(|| DynamicError::CannotUnpack {
                        field_path: field_path.to_owned(),
                        reason: "invalid optional address value".to_owned(),
                    })
                }
            }
            Ty::NullLiteral => match reader.pop()? {
                TupleItem::Null => Ok("null".to_owned()),
                _ => Err(DynamicError::CannotUnpack {
                    field_path: field_path.to_owned(),
                    reason: "not 'null' on a stack".to_owned(),
                }),
            },
            Ty::Callable => {
                reader.pop()?;
                Ok("continuation".to_owned())
            }
            Ty::Void => Ok("(void)".to_owned()),
            Ty::Unknown => Ok(debug_unknown(&reader.pop()?)),
            Ty::Nullable {
                inner_ty_idx,
                stack_type_id,
                stack_width,
            } => {
                if stack_type_id.is_some() {
                    let stack_width = stack_width.ok_or_else(|| {
                        DynamicError::InvalidAbi(format!(
                            "wide nullable at type index {ty_idx} has no stack_width"
                        ))
                    })?;
                    let tag = reader.read_union_tag(stack_width)?;
                    if tag == BigInt::from(0) {
                        reader.skip(stack_width)?;
                        Ok("null".to_owned())
                    } else {
                        let result =
                            self.debug_format_stack(field_path, inner_ty_idx, reader, false, None)?;
                        reader.finish_union_variant()?;
                        Ok(result)
                    }
                } else if matches!(reader.peek(0)?, TupleItem::Null) {
                    reader.pop()?;
                    Ok("null".to_owned())
                } else {
                    self.debug_format_stack(field_path, inner_ty_idx, reader, false, None)
                }
            }
            Ty::CellOf { inner_ty_idx } => {
                let cell = reader.read_cell()?;
                let mut slice = cell.as_slice()?;
                let value = self.unpack_type(field_path, inner_ty_idx, &mut slice, None)?;
                let inner = self.debug_format_value(field_path, inner_ty_idx, &value)?;
                Ok(format!("ref{{{inner}}}"))
            }
            Ty::ArrayOf { inner_ty_idx } => {
                let mut items = reader.read_tuple(None)?;
                let nested = calc_width_on_stack(&self.abi, inner_ty_idx) != 1;
                let mut values = Vec::new();
                while items.remaining() != 0 {
                    values.push(self.debug_format_stack(
                        field_path,
                        inner_ty_idx,
                        &mut items,
                        nested,
                        None,
                    )?);
                }
                Ok(format!("[{}]", values.join(", ")))
            }
            Ty::LispListOf { inner_ty_idx } => {
                let mut current = reader.pop()?;
                let mut values = Vec::new();
                loop {
                    match current {
                        TupleItem::Null => break,
                        TupleItem::Tuple(tuple) if tuple.0.len() == 2 => {
                            let mut pair = tuple.0.into_iter();
                            let head = pair.next().expect("pair length was checked");
                            current = pair.next().expect("pair length was checked");
                            let mut head_reader = StackReader::new(vec![head]);
                            values.push(self.debug_format_stack(
                                field_path,
                                inner_ty_idx,
                                &mut head_reader,
                                true,
                                None,
                            )?);
                            head_reader.ensure_empty()?;
                        }
                        _ => {
                            return Err(DynamicError::CannotUnpack {
                                field_path: field_path.to_owned(),
                                reason: "malformed lisp_list on stack".to_owned(),
                            });
                        }
                    }
                }
                Ok(format!("[{}]", values.join(", ")))
            }
            Ty::Tensor { items_ty_idx } => {
                let mut values = Vec::with_capacity(items_ty_idx.len());
                for (index, item_ty_idx) in items_ty_idx.into_iter().enumerate() {
                    values.push(self.debug_format_stack(
                        &format!("{field_path}[{index}]"),
                        item_ty_idx,
                        reader,
                        false,
                        None,
                    )?);
                }
                Ok(format!("({})", values.join(", ")))
            }
            Ty::ShapedTuple { items_ty_idx } => {
                let mut nested = reader.read_tuple(Some(items_ty_idx.len()))?;
                let mut values = Vec::with_capacity(items_ty_idx.len());
                for (index, item_ty_idx) in items_ty_idx.into_iter().enumerate() {
                    values.push(self.debug_format_stack(
                        &format!("{field_path}[{index}]"),
                        item_ty_idx,
                        &mut nested,
                        true,
                        None,
                    )?);
                }
                nested.ensure_empty()?;
                Ok(format!("[{}]", values.join(", ")))
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                let values = match reader.pop()? {
                    TupleItem::Null => DynamicValue::Map(Vec::new()),
                    TupleItem::Cell(root) | TupleItem::Slice(root) => {
                        self.unpack_map_root(field_path, key_ty_idx, value_ty_idx, Some(&root))?
                    }
                    _ => {
                        return Err(DynamicError::CannotUnpack {
                            field_path: field_path.to_owned(),
                            reason: "expected dictionary cell or null on TVM stack".to_owned(),
                        });
                    }
                };
                let DynamicValue::Map(entries) = values else {
                    unreachable!("unpack_map_root returns a map")
                };
                let mut parts = Vec::with_capacity(entries.len());
                for (key, value) in entries {
                    let key = self.debug_format_value(field_path, key_ty_idx, &key)?;
                    let value = self.debug_format_value(
                        &format!("{field_path}[{key}]"),
                        value_ty_idx,
                        &value,
                    )?;
                    parts.push(format!("{key}: {value}"));
                }
                Ok(format!("map{{{}}}", parts.join(", ")))
            }
            Ty::EnumRef { enum_name } => {
                let value = reader.read_int()?;
                let ABIDeclaration::Enum { members, .. } = self.declaration(&enum_name)? else {
                    return Err(DynamicError::InvalidAbi(format!(
                        "declaration '{enum_name}' is not an enum"
                    )));
                };
                let member = members.iter().find(|member| {
                    member
                        .value
                        .parse::<BigInt>()
                        .is_ok_and(|member_value| member_value == value)
                });
                Ok(member.map_or_else(
                    || format!("{enum_name}({value})"),
                    |member| format!("{enum_name}.{}", member.name),
                ))
            }
            Ty::StructRef { struct_name, .. } => {
                let fields = self.struct_fields(ty_idx, true)?;
                if fields.is_empty() {
                    return Ok(format!("{struct_name} {{}}"));
                }
                let mut parts = Vec::with_capacity(fields.len());
                for field in fields {
                    let value = self.debug_format_stack(
                        &format!("{field_path}.{}", field.name),
                        field.ty_idx,
                        reader,
                        false,
                        field.union_label_ty_idx,
                    )?;
                    parts.push(format!("{}: {value}", field.name));
                }
                Ok(format!("{struct_name} {{ {} }}", parts.join(", ")))
            }
            Ty::AliasRef { .. } => {
                let target = self.alias_target(ty_idx)?;
                self.debug_format_stack(
                    field_path,
                    target.ty_idx,
                    reader,
                    false,
                    target.union_label_ty_idx,
                )
            }
            Ty::Union {
                variants,
                stack_width,
            } => {
                let stack_width = stack_width.ok_or_else(|| {
                    DynamicError::InvalidAbi(format!(
                        "union at type index {ty_idx} has no stack_width"
                    ))
                })?;
                let tag = reader.read_union_tag(stack_width)?;
                let variants = self.union_variants(&variants, union_label_ty_idx)?;
                let Some(variant) = variants.iter().find(|variant| {
                    variant
                        .stack_type_id
                        .is_some_and(|type_id| tag == BigInt::from(type_id))
                }) else {
                    return Err(DynamicError::CannotUnpack {
                        field_path: field_path.to_owned(),
                        reason: format!("unexpected UTag={tag}"),
                    });
                };
                let variant_width = variant.stack_width.ok_or_else(|| {
                    DynamicError::InvalidAbi(format!(
                        "union variant '{}' has no stack_width",
                        variant.label
                    ))
                })?;
                reader.prepare_union_variant(stack_width, variant_width)?;
                let value = self.debug_format_stack(
                    &format!("{field_path}#{}", variant.label),
                    variant.variant_ty_idx,
                    reader,
                    false,
                    None,
                )?;
                reader.finish_union_variant()?;
                if variant.has_value_field {
                    Ok(format!("#{} {value}", variant.label))
                } else {
                    Ok(value)
                }
            }
            Ty::GenericT { name_t } => Err(DynamicError::InvalidAbi(format!(
                "unexpected genericT={name_t} at {field_path}"
            ))),
        }
    }

    fn debug_format_value(
        &self,
        field_path: &str,
        ty_idx: TyIdx,
        value: &DynamicValue,
    ) -> Result<String, DynamicError> {
        let tuple = self.construct_stack_type(field_path, ty_idx, value, false, None)?;
        let mut reader = StackReader::new(tuple);
        let result = self.debug_format_stack(field_path, ty_idx, &mut reader, false, None)?;
        reader.ensure_empty()?;
        Ok(result)
    }
}

fn declaration_name(declaration: &ABIDeclaration) -> &str {
    match declaration {
        ABIDeclaration::Struct { name, .. }
        | ABIDeclaration::Alias { name, .. }
        | ABIDeclaration::Enum { name, .. } => name,
    }
}

fn uses_custom_pack(custom: Option<&ABICustomPackUnpack>) -> bool {
    custom
        .and_then(|custom| custom.pack_to_builder)
        .unwrap_or(false)
}

fn uses_custom_unpack(custom: Option<&ABICustomPackUnpack>) -> bool {
    custom
        .and_then(|custom| custom.unpack_from_slice)
        .unwrap_or(false)
}

fn bits(bits: u32) -> Result<u16, DynamicError> {
    u16::try_from(bits)
        .map_err(|_| DynamicError::InvalidAbi(format!("bit width {bits} exceeds u16")))
}

fn prefix_bits(bits: usize) -> Result<u16, DynamicError> {
    u16::try_from(bits)
        .map_err(|_| DynamicError::InvalidAbi(format!("prefix width {bits} exceeds u16")))
}

fn varint_len_bits(n: u32) -> Result<u16, DynamicError> {
    if !n.is_power_of_two() {
        return Err(DynamicError::InvalidAbi(format!(
            "variadic integer size {n} is not a power of two"
        )));
    }
    bits(n.ilog2())
}

fn store_prefix(
    builder: &mut CellBuilder,
    prefix_num: u64,
    prefix_len: i32,
) -> Result<(), DynamicError> {
    let prefix_len = u16::try_from(prefix_len).map_err(|_| {
        DynamicError::InvalidAbi(format!("invalid serialization prefix width {prefix_len}"))
    })?;
    crate::cell::store_fixed_int(builder, &BigInt::from(prefix_num), prefix_len, false)?;
    Ok(())
}

fn matches_prefix(
    slice: &CellSlice<'_>,
    expected: u64,
    prefix_len: usize,
) -> Result<bool, DynamicError> {
    let prefix_len = prefix_bits(prefix_len)?;
    if !slice.has_remaining(prefix_len, 0) {
        return Ok(false);
    }
    Ok(slice.get_uint(0, prefix_len)? == expected)
}

fn store_maybe_ref(builder: &mut CellBuilder, value: Option<Cell>) -> Result<(), DynamicError> {
    builder.store_bit(value.is_some())?;
    if let Some(value) = value {
        builder.store_reference(value)?;
    }
    Ok(())
}

fn load_maybe_ref(slice: &mut CellSlice<'_>) -> Result<Option<Cell>, DynamicError> {
    if slice.load_bit()? {
        Ok(Some(slice.load_reference_cloned()?))
    } else {
        Ok(None)
    }
}

fn owned_slice_to_cell(slice: &OwnedSlice) -> Result<Cell, DynamicError> {
    let mut builder = CellBuilder::new();
    crate::cell::store_slice(&mut builder, slice)?;
    Ok(builder.build()?)
}

fn wrap_union_value(variant: &ResolvedUnionVariant, value: DynamicValue) -> DynamicValue {
    if variant.has_value_field {
        DynamicValue::union(variant.label.clone(), value)
    } else {
        value
    }
}

fn js_interpolation(value: &DynamicValue) -> String {
    match value {
        DynamicValue::Void => "undefined".to_owned(),
        DynamicValue::Null => "null".to_owned(),
        DynamicValue::Number(value) => value.to_string(),
        DynamicValue::Bool(value) => value.to_string(),
        DynamicValue::String(value) => value.clone(),
        DynamicValue::Array(values) => values
            .iter()
            .map(js_interpolation)
            .collect::<Vec<_>>()
            .join(","),
        DynamicValue::AddressNone => "none".to_owned(),
        DynamicValue::Cell(_)
        | DynamicValue::Builder(_)
        | DynamicValue::Slice(_)
        | DynamicValue::Bits(_)
        | DynamicValue::Address(_)
        | DynamicValue::ExtAddress(_)
        | DynamicValue::Map(_)
        | DynamicValue::Object(_)
        | DynamicValue::Unknown(_) => "[object Object]".to_owned(),
    }
}

fn format_address(value: &DynamicValue) -> Option<String> {
    match value {
        DynamicValue::Address(IntAddr::Std(address)) => {
            Some(address.display_base64_url(true).to_string())
        }
        DynamicValue::Address(IntAddr::Var(address)) => Some(format!(
            "{}:{}",
            address.workchain,
            Bitstring {
                bytes: &address.address,
                bit_len: address.address_len.into_inner(),
            }
        )),
        DynamicValue::ExtAddress(address) => Some(address.to_string()),
        DynamicValue::AddressNone => Some("addr_none".to_owned()),
        DynamicValue::Null => Some("null".to_owned()),
        _ => None,
    }
}

fn print_raw_cell(cell: &Cell) -> Result<String, DynamicError> {
    print_raw_dyn_cell(cell.as_ref())
}

fn print_raw_dyn_cell(cell: &DynCell) -> Result<String, DynamicError> {
    let slice = cell.as_slice()?;
    let mut result = format!("x{{{:X}}}", slice.display_data());
    for reference in cell.references() {
        let child = print_raw_dyn_cell(reference)?;
        for line in child.lines() {
            result.push('\n');
            result.push(' ');
            result.push_str(line);
        }
    }
    Ok(result)
}

fn debug_unknown(item: &TupleItem) -> String {
    match item {
        TupleItem::Int(value) => value.to_string(),
        TupleItem::Cell(cell) => format!(
            "cell{{{}}}",
            print_raw_cell(cell).unwrap_or_else(|_| "<invalid cell>".to_owned())
        ),
        TupleItem::Slice(cell) => format!(
            "slice{{{}}}",
            print_raw_cell(cell).unwrap_or_else(|_| "<invalid slice>".to_owned())
        ),
        TupleItem::Builder(cell) => format!(
            "builder{{{}}}",
            print_raw_cell(cell).unwrap_or_else(|_| "<invalid builder>".to_owned())
        ),
        TupleItem::Null => "null".to_owned(),
        TupleItem::Tuple(tuple) => format!(
            "({})",
            tuple
                .0
                .iter()
                .map(debug_unknown)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TupleItem::Nan => "NaN".to_owned(),
        TupleItem::Cont(_) => "<unknown>".to_owned(),
    }
}
