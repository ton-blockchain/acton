use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(Box<T>),
    Many(Vec<T>),
}

/// =======================
/// Types: `TypePtr::as_abi_json()`
/// =======================
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ABIType {
    // ---- primitives ----
    #[serde(rename = "int")]
    Int,

    #[serde(rename = "bool")]
    Bool,

    #[serde(rename = "cell")]
    Cell,

    #[serde(rename = "slice")]
    Slice,

    #[serde(rename = "builder")]
    Builder,

    // TypeDataContinuation + TypeDataFunCallable
    #[serde(rename = "callable", alias = "continuation")]
    Callable,

    // TypeDataString
    #[serde(rename = "string")]
    String,

    // TypeDataCoins
    #[serde(rename = "coins")]
    Coins,

    // TypeDataVoid + TypeDataNever
    #[serde(rename = "void")]
    Void,

    // ---- addresses ----
    // TypeDataAddress::is_internal() ? address : addressAny
    #[serde(rename = "address")]
    Address,

    #[serde(rename = "addressAny")]
    AddressAny,

    // Special-case in TypeDataUnion for AddressAlias?
    #[serde(rename = "addressOpt")]
    AddressOpt,

    // ---- ints with width / variadic ----
    #[serde(rename = "uintN")]
    UintN { n: usize },

    #[serde(rename = "intN")]
    IntN { n: usize },

    #[serde(rename = "varuintN")]
    VarUintN { n: usize },

    #[serde(rename = "varintN")]
    VarIntN { n: usize },

    // TypeDataBitsN
    #[serde(rename = "bitsN")]
    BitsN { n: usize },

    // ---- composite / container ----
    // TypeDataArray
    #[serde(rename = "arrayOf")]
    ArrayOf { inner: Box<ABIType> },

    // TypeDataTensor
    #[serde(rename = "tensor")]
    Tensor { items: Vec<ABIType> },

    // TypeDataShapedTuple (TYPE)
    #[serde(rename = "shapedTuple")]
    ShapedTuple { items: Vec<ABIType> },

    // TypeDataNullLiteral (TYPE)
    #[serde(rename = "nullLiteral")]
    NullLiteral,

    // ---- generics ----
    // TypeDataGenericT
    #[serde(rename = "genericT")]
    GenericT { name_t: String },

    // ---- references to declarations ----
    // TypeDataStruct
    #[serde(rename = "StructRef")]
    StructRef {
        struct_name: String,

        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        type_args: Vec<ABIType>,
    },

    // TypeDataEnum
    #[serde(rename = "EnumRef")]
    EnumRef { enum_name: String },

    // TypeDataAlias / GenericTypeWithTs (alias_ref)
    #[serde(rename = "AliasRef")]
    AliasRef {
        alias_name: String,

        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        type_args: Vec<ABIType>,
    },

    // ---- special aliases / builtins ----
    // TypeDataAlias special-cases RemainingBitsAndRefs -> remaining
    #[serde(rename = "remaining")]
    Remaining,

    // TypeDataGenericTypeWithTs / TypeDataStruct special-case Cell / LispList
    #[serde(rename = "cellOf")]
    CellOf { inner: OneOrMany<ABIType> },

    #[serde(rename = "lispListOf")]
    LispListOf { inner: OneOrMany<ABIType> },

    // TypeDataUnion
    #[serde(rename = "union")]
    Union { variants: Vec<ABIUnionVariant> },

    // TypeDataUnion (or_null != null) -> nullable
    #[serde(rename = "nullable")]
    Nullable { inner: Box<ABIType> },

    // TypeDataMapKV
    #[serde(rename = "mapKV")]
    MapKV { k: Box<ABIType>, v: Box<ABIType> },

    // TypeDataUnknown
    #[serde(rename = "unknown")]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIUnionVariant {
    pub variant_ty: ABIType,
    pub prefix_str: String,
    pub prefix_len: i32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_prefix_implicit: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_type_id: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_width: Option<i32>,
}

/// =======================
/// Const values: `ConstValExpression` -> constants[].value
/// =======================
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ABIConstValue {
    #[serde(rename = "int")]
    Int { v: String },

    #[serde(rename = "bool")]
    Bool { v: bool },

    #[serde(rename = "slice")]
    Slice { hex: String },

    #[serde(rename = "string")]
    String { str: String },

    #[serde(rename = "address")]
    Address { addr: String },

    #[serde(rename = "tensor")]
    Tensor { items: Vec<ABIConstValue> },

    #[serde(rename = "shapedTuple")]
    ShapedTuple { items: Vec<ABIConstValue> },

    #[serde(rename = "object")]
    Object {
        struct_name: String,
        fields: Vec<ABIConstValue>,
    },

    #[serde(rename = "castTo")]
    CastTo {
        inner: Box<ABIConstValue>,
        cast_to: ABIType,
    },

    #[serde(rename = "null")]
    Null,
}

/// =======================
/// Declarations (`used_symbols` -> declarations[])
/// =======================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIOpcode {
    pub prefix_str: String,
    pub prefix_len: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABICustomPackUnpack {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pack_to_builder: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unpack_from_slice: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIStructField {
    pub name: String,
    pub ty: ABIType,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<ABIConstValue>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIEnumMember {
    pub name: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ABIDeclaration {
    #[serde(rename = "struct")]
    Struct {
        name: String,

        #[serde(skip_serializing_if = "Option::is_none")]
        type_params: Option<Vec<String>>,

        #[serde(skip_serializing_if = "Option::is_none")]
        prefix: Option<ABIOpcode>,

        fields: Vec<ABIStructField>,

        #[serde(skip_serializing_if = "Option::is_none")]
        custom_pack_unpack: Option<ABICustomPackUnpack>,
    },

    #[serde(rename = "alias")]
    Alias {
        name: String,

        target_ty: ABIType,

        #[serde(skip_serializing_if = "Option::is_none")]
        type_params: Option<Vec<String>>,

        #[serde(skip_serializing_if = "Option::is_none")]
        custom_pack_unpack: Option<ABICustomPackUnpack>,
    },

    #[serde(rename = "enum")]
    Enum {
        name: String,
        encoded_as: ABIType,
        members: Vec<ABIEnumMember>,
        #[serde(skip_serializing_if = "Option::is_none")]
        custom_pack_unpack: Option<ABICustomPackUnpack>,
    },
}

/// =======================
/// ABI messages / storage / getters / errors / constants
/// =======================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIFunctionParameter {
    pub name: String,
    pub ty: ABIType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<ABIConstValue>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIGetMethod {
    pub tvm_method_id: i32,
    pub name: String,
    pub parameters: Vec<ABIFunctionParameter>,
    pub return_ty: ABIType,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIInternalMessage {
    pub body_ty: ABIType,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimal_msg_value: Option<i64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_send_mode: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIExternalMessage {
    pub body_ty: ABIType,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIOutgoingMessage {
    pub body_ty: ABIType,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIStorage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_ty: Option<ABIType>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_at_deployment_ty: Option<ABIType>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ABIThrownErrorKind {
    PlainInt,
    Constant,
    EnumMember,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIThrownError {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<ABIThrownErrorKind>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub err_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIConstant {
    pub name: String,
    pub value: ABIConstValue,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractABI {
    pub abi_schema_version: String,

    pub contract_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,

    pub declarations: Vec<ABIDeclaration>,

    pub incoming_messages: Vec<ABIInternalMessage>,
    pub incoming_external: Vec<ABIExternalMessage>,
    pub outgoing_messages: Vec<ABIOutgoingMessage>,
    pub emitted_events: Vec<ABIOutgoingMessage>,

    pub storage: ABIStorage,

    pub get_methods: Vec<ABIGetMethod>,
    pub thrown_errors: Vec<ABIThrownError>,
    pub constants: Vec<ABIConstant>,

    pub compiler_name: String,
    pub compiler_version: String,
}

#[derive(Debug, Clone)]
pub struct ABIResolvedStruct {
    pub name: String,
    pub fields: Vec<ABIStructField>,
}

impl ContractABI {
    pub fn resolve_storage_struct(&self) -> anyhow::Result<Option<ABIResolvedStruct>> {
        let Some(storage_ty) = self
            .storage
            .storage_at_deployment_ty
            .as_ref()
            .or(self.storage.storage_ty.as_ref())
        else {
            return Ok(None);
        };

        Ok(Some(self.resolve_single_struct(storage_ty, "storage")?))
    }

    pub fn resolve_incoming_message_structs(&self) -> anyhow::Result<Vec<ABIResolvedStruct>> {
        let mut resolved = Vec::new();
        let mut seen_structs = HashSet::new();

        for message in &self.incoming_messages {
            collect_structs_from_type(
                self,
                &message.body_ty,
                &mut HashSet::new(),
                &mut seen_structs,
                &mut resolved,
            )?;
        }

        Ok(resolved)
    }

    pub fn resolve_single_struct(
        &self,
        ty: &ABIType,
        context: &str,
    ) -> anyhow::Result<ABIResolvedStruct> {
        let mut resolved = Vec::new();
        let mut seen_structs = HashSet::new();
        collect_structs_from_type(
            self,
            ty,
            &mut HashSet::new(),
            &mut seen_structs,
            &mut resolved,
        )?;

        let mut structs = resolved.into_iter();
        let Some(first) = structs.next() else {
            anyhow::bail!(
                "Failed to resolve {} type {} into a concrete struct",
                context,
                ty.render_type()
            );
        };

        if let Some(other) = structs.next() {
            anyhow::bail!(
                "{} type {} resolves to multiple structs ({}, {})",
                context,
                ty.render_type(),
                first.name,
                other.name
            );
        }

        Ok(first)
    }
}

impl ABIType {
    #[must_use]
    pub fn render_param_type(&self) -> String {
        if let ABIType::CellOf { inner } = self {
            render_one_or_many_type(inner)
        } else {
            self.render_type()
        }
    }

    pub fn render_type(&self) -> String {
        match self {
            ABIType::Int => "int".to_owned(),
            ABIType::Bool => "bool".to_owned(),
            ABIType::Cell => "cell".to_owned(),
            ABIType::Slice => "slice".to_owned(),
            ABIType::Builder => "builder".to_owned(),
            ABIType::Callable => "continuation".to_owned(),
            ABIType::String => "string".to_owned(),
            ABIType::Coins => "coins".to_owned(),
            ABIType::Void => "void".to_owned(),
            ABIType::Address => "address".to_owned(),
            ABIType::AddressAny => "any_address".to_owned(),
            ABIType::AddressOpt => "address?".to_owned(),
            ABIType::UintN { n } => format!("uint{n}"),
            ABIType::IntN { n } => format!("int{n}"),
            ABIType::VarUintN { n } => format!("varuint{n}"),
            ABIType::VarIntN { n } => format!("varint{n}"),
            ABIType::BitsN { n } => format!("bits{n}"),
            ABIType::ArrayOf { inner } => format!("array<{}>", inner.render_type()),
            ABIType::Tensor { items } => format!(
                "({})",
                items
                    .iter()
                    .map(ABIType::render_type)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ABIType::ShapedTuple { items } => format!(
                "[{}]",
                items
                    .iter()
                    .map(ABIType::render_type)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            ABIType::NullLiteral => "null".to_owned(),
            ABIType::GenericT { name_t } => name_t.clone(),
            ABIType::StructRef {
                struct_name,
                type_args,
            } => render_named_type(struct_name, type_args),
            ABIType::EnumRef { enum_name } => enum_name.clone(),
            ABIType::AliasRef {
                alias_name,
                type_args,
            } => render_named_type(alias_name, type_args),
            ABIType::Remaining => "RemainingBitsAndRefs".to_owned(),
            ABIType::CellOf { inner } => format!("Cell<{}>", render_one_or_many_type(inner)),
            ABIType::LispListOf { inner } => {
                format!("lisp_list<{}>", render_one_or_many_type(inner))
            }
            ABIType::Union { variants } => render_union_type(variants),
            ABIType::Nullable { inner } => render_nullable_type(inner),
            ABIType::MapKV { k, v } => {
                format!("map<{}, {}>", k.render_type(), v.render_type())
            }
            ABIType::Unknown => "unknown".to_owned(),
        }
    }

    #[must_use]
    pub const fn is_typed_cell(&self) -> bool {
        matches!(self, ABIType::CellOf { .. })
    }

    #[must_use]
    pub fn typed_cell_payload_default_value(&self, abi: &ContractABI) -> Option<String> {
        match self {
            ABIType::CellOf { inner } => Some(default_value_for_one_or_many(
                abi,
                inner,
                &BTreeMap::new(),
                &mut HashSet::new(),
            )),
            _ => None,
        }
    }

    #[must_use]
    pub fn default_value(&self, abi: &ContractABI) -> String {
        default_value_impl(abi, self, &BTreeMap::new(), &mut HashSet::new())
    }
}

fn collect_structs_from_type(
    abi: &ContractABI,
    ty: &ABIType,
    visited_aliases: &mut HashSet<String>,
    seen_structs: &mut HashSet<String>,
    resolved: &mut Vec<ABIResolvedStruct>,
) -> anyhow::Result<()> {
    match ty {
        ABIType::StructRef { struct_name, .. } => {
            let fields = find_struct_decl(abi, struct_name)
                .ok_or_else(|| anyhow!("Struct {struct_name} referenced by ABI was not found"))?;
            if seen_structs.insert(struct_name.clone()) {
                resolved.push(to_resolved_struct(struct_name, fields));
            }
            Ok(())
        }
        ABIType::AliasRef { alias_name, .. } => {
            if !visited_aliases.insert(alias_name.clone()) {
                anyhow::bail!("Cyclic ABI alias reference detected for {alias_name}");
            }

            let result = find_alias_decl(abi, alias_name)
                .ok_or_else(|| anyhow!("Alias {alias_name} referenced by ABI was not found"))
                .and_then(|target_ty| {
                    collect_structs_from_type(
                        abi,
                        target_ty,
                        visited_aliases,
                        seen_structs,
                        resolved,
                    )
                });

            visited_aliases.remove(alias_name);
            result
        }
        ABIType::Union { variants } => {
            for variant in variants {
                collect_structs_from_type(
                    abi,
                    &variant.variant_ty,
                    visited_aliases,
                    seen_structs,
                    resolved,
                )?;
            }
            Ok(())
        }
        ABIType::Nullable { inner } => {
            collect_structs_from_type(abi, inner, visited_aliases, seen_structs, resolved)
        }
        ABIType::CellOf { inner } | ABIType::LispListOf { inner } => {
            collect_structs_from_one_or_many(abi, inner, visited_aliases, seen_structs, resolved)
        }
        _ => anyhow::bail!(
            "Unsupported ABI type {} while resolving contract wrapper types",
            ty.render_type()
        ),
    }
}

fn collect_structs_from_one_or_many(
    abi: &ContractABI,
    types: &OneOrMany<ABIType>,
    visited_aliases: &mut HashSet<String>,
    seen_structs: &mut HashSet<String>,
    resolved: &mut Vec<ABIResolvedStruct>,
) -> anyhow::Result<()> {
    match types {
        OneOrMany::One(inner) => {
            collect_structs_from_type(abi, inner, visited_aliases, seen_structs, resolved)
        }
        OneOrMany::Many(items) => {
            for item in items {
                collect_structs_from_type(abi, item, visited_aliases, seen_structs, resolved)?;
            }
            Ok(())
        }
    }
}

fn find_struct_decl<'a>(abi: &'a ContractABI, target_name: &str) -> Option<&'a [ABIStructField]> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Struct { name, fields, .. } if name == target_name => {
            Some(fields.as_slice())
        }
        _ => None,
    })
}

fn find_alias_decl<'a>(abi: &'a ContractABI, target_name: &str) -> Option<&'a ABIType> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Alias {
            name, target_ty, ..
        } if name == target_name => Some(target_ty),
        _ => None,
    })
}

fn to_resolved_struct(name: &str, fields: &[ABIStructField]) -> ABIResolvedStruct {
    ABIResolvedStruct {
        name: name.to_owned(),
        fields: fields.to_vec(),
    }
}

fn find_struct_decl_full<'a>(
    abi: &'a ContractABI,
    target_name: &str,
) -> Option<(Option<&'a [String]>, &'a [ABIStructField])> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Struct {
            name,
            type_params,
            fields,
            ..
        } if name == target_name => Some((type_params.as_deref(), fields.as_slice())),
        _ => None,
    })
}

fn find_alias_decl_full<'a>(
    abi: &'a ContractABI,
    target_name: &str,
) -> Option<(Option<&'a [String]>, &'a ABIType)> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Alias {
            name,
            target_ty,
            type_params,
            ..
        } if name == target_name => Some((type_params.as_deref(), target_ty)),
        _ => None,
    })
}

fn find_enum_decl<'a>(
    abi: &'a ContractABI,
    target_name: &str,
) -> Option<(&'a ABIType, &'a [ABIEnumMember])> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Enum {
            name,
            encoded_as,
            members,
            ..
        } if name == target_name => Some((encoded_as, members.as_slice())),
        _ => None,
    })
}

fn render_named_type(name: &str, type_args: &[ABIType]) -> String {
    if type_args.is_empty() {
        name.to_owned()
    } else {
        format!(
            "{name}<{}>",
            type_args
                .iter()
                .map(ABIType::render_type)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn render_one_or_many_type(types: &OneOrMany<ABIType>) -> String {
    match types {
        OneOrMany::One(inner) => inner.render_type(),
        OneOrMany::Many(items) if items.len() == 1 => items[0].render_type(),
        OneOrMany::Many(items) => format!(
            "({})",
            items
                .iter()
                .map(ABIType::render_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render_union_type(variants: &[ABIUnionVariant]) -> String {
    let null_variants = variants
        .iter()
        .filter(|variant| matches!(variant.variant_ty, ABIType::NullLiteral))
        .count();
    let non_null_variants = variants
        .iter()
        .filter(|variant| !matches!(variant.variant_ty, ABIType::NullLiteral))
        .collect::<Vec<_>>();

    if null_variants == 1 && non_null_variants.len() == 1 {
        return render_nullable_type(&non_null_variants[0].variant_ty);
    }

    variants
        .iter()
        .map(|variant| variant.variant_ty.render_type())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn render_nullable_type(inner: &ABIType) -> String {
    let inner_text = inner.render_type();
    if matches!(inner, ABIType::Callable | ABIType::Union { .. }) {
        format!("({inner_text})?")
    } else {
        format!("{inner_text}?")
    }
}

fn default_value_impl(
    abi: &ContractABI,
    ty: &ABIType,
    bindings: &BTreeMap<String, ABIType>,
    visited_defs: &mut HashSet<String>,
) -> String {
    match ty {
        ABIType::Int
        | ABIType::Coins
        | ABIType::UintN { .. }
        | ABIType::IntN { .. }
        | ABIType::VarUintN { .. }
        | ABIType::VarIntN { .. } => "0".to_owned(),
        ABIType::Bool => "false".to_owned(),
        ABIType::Cell => "createEmptyCell()".to_owned(),
        ABIType::Slice | ABIType::Remaining => "createEmptySlice()".to_owned(),
        ABIType::Builder => "beginCell()".to_owned(),
        ABIType::Callable => "fun () {}".to_owned(),
        ABIType::String => "\"\"".to_owned(),
        ABIType::Void => "()".to_owned(),
        ABIType::Address => {
            "address(\"EQD__________________________________________0vo\")".to_owned()
        }
        ABIType::AddressAny => "createAddressNone()".to_owned(),
        ABIType::AddressOpt | ABIType::NullLiteral | ABIType::Nullable { .. } => "null".to_owned(),
        ABIType::BitsN { n } => format!("\"\" as bits{n}"),
        ABIType::ArrayOf { .. } | ABIType::LispListOf { .. } | ABIType::MapKV { .. } => {
            "[]".to_owned()
        }
        ABIType::Tensor { items } => format!(
            "({})",
            items
                .iter()
                .map(|item| default_value_impl(abi, item, bindings, visited_defs))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ABIType::ShapedTuple { items } => format!(
            "[{}]",
            items
                .iter()
                .map(|item| default_value_impl(abi, item, bindings, visited_defs))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ABIType::GenericT { .. } => "{}".to_owned(),
        ABIType::StructRef {
            struct_name,
            type_args,
        } => default_struct_value(abi, struct_name, type_args, visited_defs),
        ABIType::EnumRef { enum_name } => default_enum_value(abi, enum_name),
        ABIType::AliasRef {
            alias_name,
            type_args,
        } => default_alias_value(abi, alias_name, type_args, visited_defs),
        ABIType::CellOf { inner } => format!(
            "{}.toCell()",
            default_value_for_one_or_many(abi, inner, bindings, visited_defs)
        ),
        ABIType::Union { variants } => default_union_value(abi, variants, bindings, visited_defs),
        ABIType::Unknown => "null".to_owned(),
    }
}

fn default_struct_value(
    abi: &ContractABI,
    struct_name: &str,
    type_args: &[ABIType],
    visited_defs: &mut HashSet<String>,
) -> String {
    let qualified_name = render_named_type(struct_name, type_args);
    let visit_key = format!("struct:{qualified_name}");
    if !visited_defs.insert(visit_key.clone()) {
        return "null".to_owned();
    }

    let result = find_struct_decl_full(abi, struct_name)
        .map(|(type_params, fields)| {
            if !type_args.is_empty() || type_params.is_some_and(|params| !params.is_empty()) {
                return "{}".to_owned();
            }

            let rendered_fields = fields
                .iter()
                .map(|field| {
                    let value = match &field.default_value {
                        Some(default_value) => render_const_value(abi, default_value),
                        None => default_value_impl(abi, &field.ty, &BTreeMap::new(), visited_defs),
                    };
                    format!("{}: {}", field.name, value)
                })
                .collect::<Vec<_>>();

            if rendered_fields.is_empty() {
                format!("{qualified_name} {{}}")
            } else {
                format!("{qualified_name} {{ {} }}", rendered_fields.join(", "))
            }
        })
        .unwrap_or_else(|| "null".to_owned());

    visited_defs.remove(&visit_key);
    result
}

fn default_alias_value(
    abi: &ContractABI,
    alias_name: &str,
    type_args: &[ABIType],
    visited_defs: &mut HashSet<String>,
) -> String {
    let qualified_name = render_named_type(alias_name, type_args);
    let visit_key = format!("alias:{qualified_name}");
    if !visited_defs.insert(visit_key.clone()) {
        return "null".to_owned();
    }

    let result = find_alias_decl_full(abi, alias_name)
        .map(|(type_params, target_ty)| {
            if !type_args.is_empty() || type_params.is_some_and(|params| !params.is_empty()) {
                return "{}".to_owned();
            }

            let target_default = default_value_impl(abi, target_ty, &BTreeMap::new(), visited_defs);
            if target_default == "null" {
                target_default
            } else {
                format!("({target_default} as {qualified_name})")
            }
        })
        .unwrap_or_else(|| "null".to_owned());

    visited_defs.remove(&visit_key);
    result
}

fn default_enum_value(abi: &ContractABI, enum_name: &str) -> String {
    find_enum_decl(abi, enum_name)
        .and_then(|(_, members)| members.first())
        .map_or_else(
            || "null".to_owned(),
            |member| format!("{enum_name}.{}", member.name),
        )
}

fn default_union_value(
    abi: &ContractABI,
    variants: &[ABIUnionVariant],
    bindings: &BTreeMap<String, ABIType>,
    visited_defs: &mut HashSet<String>,
) -> String {
    let Some(first_variant) = variants.first() else {
        return "null".to_owned();
    };

    if variants
        .iter()
        .any(|variant| matches!(variant.variant_ty, ABIType::NullLiteral))
    {
        return "null".to_owned();
    }

    default_value_impl(abi, &first_variant.variant_ty, bindings, visited_defs)
}

fn default_value_for_one_or_many(
    abi: &ContractABI,
    types: &OneOrMany<ABIType>,
    bindings: &BTreeMap<String, ABIType>,
    visited_defs: &mut HashSet<String>,
) -> String {
    match types {
        OneOrMany::One(inner) => default_value_impl(abi, inner, bindings, visited_defs),
        OneOrMany::Many(items) if items.len() == 1 => {
            default_value_impl(abi, &items[0], bindings, visited_defs)
        }
        OneOrMany::Many(items) => format!(
            "({})",
            items
                .iter()
                .map(|item| default_value_impl(abi, item, bindings, visited_defs))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn render_const_value(abi: &ContractABI, value: &ABIConstValue) -> String {
    match value {
        ABIConstValue::Int { v } => v.clone(),
        ABIConstValue::Bool { v } => v.to_string(),
        ABIConstValue::Slice { hex } => serde_json::to_string(hex).map_or_else(
            |_| "null".to_owned(),
            |hex| format!("stringHexToSlice({hex})"),
        ),
        ABIConstValue::String { str } => {
            serde_json::to_string(str).unwrap_or_else(|_| "\"\"".to_owned())
        }
        ABIConstValue::Address { addr } => serde_json::to_string(addr)
            .map_or_else(|_| "null".to_owned(), |addr| format!("address({addr})")),
        ABIConstValue::Tensor { items } => format!(
            "({})",
            items
                .iter()
                .map(|item| render_const_value(abi, item))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ABIConstValue::ShapedTuple { items } => format!(
            "[{}]",
            items
                .iter()
                .map(|item| render_const_value(abi, item))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ABIConstValue::Object {
            struct_name,
            fields,
        } => render_const_object_value(abi, struct_name, fields),
        ABIConstValue::CastTo { inner, cast_to } => format!(
            "({} as {})",
            render_const_value(abi, inner),
            cast_to.render_type()
        ),
        ABIConstValue::Null => "null".to_owned(),
    }
}

fn render_const_object_value(
    abi: &ContractABI,
    struct_name: &str,
    fields: &[ABIConstValue],
) -> String {
    let Some(struct_fields) = find_struct_decl(abi, struct_name) else {
        return format!("{struct_name} {{}}");
    };

    let rendered_fields = struct_fields
        .iter()
        .zip(fields)
        .map(|(field, value)| format!("{}: {}", field.name, render_const_value(abi, value)))
        .collect::<Vec<_>>();

    if rendered_fields.is_empty() {
        format!("{struct_name} {{}}")
    } else {
        format!("{struct_name} {{ {} }}", rendered_fields.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ABIConstValue, ABIDeclaration, ABIEnumMember, ABIGetMethod, ABIInternalMessage, ABIOpcode,
        ABIStorage, ABIStructField, ABIThrownError, ABIThrownErrorKind, ABIType, ABIUnionVariant,
        ContractABI, OneOrMany,
    };

    fn empty_abi() -> ContractABI {
        ContractABI {
            abi_schema_version: "1.0".to_owned(),
            contract_name: "Test".to_owned(),
            author: String::new(),
            version: String::new(),
            description: String::new(),
            declarations: Vec::new(),
            incoming_messages: Vec::new(),
            incoming_external: Vec::new(),
            outgoing_messages: Vec::new(),
            emitted_events: Vec::new(),
            storage: ABIStorage {
                storage_ty: None,
                storage_at_deployment_ty: None,
            },
            get_methods: Vec::new(),
            thrown_errors: Vec::new(),
            constants: Vec::new(),
            compiler_name: "tolkc".to_owned(),
            compiler_version: "test".to_owned(),
        }
    }

    #[test]
    fn thrown_error_deserializes_abi_1_3_format() {
        let error: ABIThrownError = serde_json::from_str(
            r#"{"kind":"enum_member","name":"Errors.NotEnoughTon","err_code":57}"#,
        )
        .expect("failed to deserialize ABI 1.3 thrown error");

        assert_eq!(error.kind, Some(ABIThrownErrorKind::EnumMember));
        assert_eq!(error.name, "Errors.NotEnoughTon");
        assert_eq!(error.err_code, 57);
    }

    #[test]
    fn thrown_error_deserializes_without_kind() {
        let error: ABIThrownError =
            serde_json::from_str(r#"{"name":"ERR_NOT_ENOUGH_TON","err_code":57}"#)
                .expect("failed to deserialize thrown error without kind");

        assert_eq!(error.kind, None);
        assert_eq!(error.name, "ERR_NOT_ENOUGH_TON");
        assert_eq!(error.err_code, 57);
    }

    #[test]
    fn get_method_parameter_deserializes_default_value() {
        let method: ABIGetMethod = serde_json::from_str(
            r#"{
                "tvm_method_id": 1,
                "name": "foo",
                "parameters": [{
                    "name": "arg",
                    "ty": {"kind":"int"},
                    "default_value": {"kind":"int","v":"10"}
                }],
                "return_ty": {"kind":"int"}
            }"#,
        )
        .expect("failed to deserialize get method with parameter default value");

        assert_eq!(method.parameters.len(), 1);
        assert!(matches!(
            method.parameters[0].default_value,
            Some(ABIConstValue::Int { ref v }) if v == "10"
        ));
    }

    #[test]
    fn resolves_incoming_messages_through_alias_union_in_order() {
        let mut abi = empty_abi();
        abi.declarations = vec![
            ABIDeclaration::Struct {
                name: "MsgA".to_owned(),
                type_params: None,
                prefix: Some(ABIOpcode {
                    prefix_str: "0x1".to_owned(),
                    prefix_len: 32,
                }),
                fields: vec![ABIStructField {
                    name: "value".to_owned(),
                    ty: ABIType::IntN { n: 32 },
                    default_value: None,
                    description: String::new(),
                }],
                custom_pack_unpack: None,
            },
            ABIDeclaration::Struct {
                name: "MsgB".to_owned(),
                type_params: None,
                prefix: Some(ABIOpcode {
                    prefix_str: "0x2".to_owned(),
                    prefix_len: 32,
                }),
                fields: vec![],
                custom_pack_unpack: None,
            },
            ABIDeclaration::Alias {
                name: "Incoming".to_owned(),
                target_ty: ABIType::Union {
                    variants: vec![
                        ABIUnionVariant {
                            variant_ty: ABIType::StructRef {
                                struct_name: "MsgA".to_owned(),
                                type_args: Vec::new(),
                            },
                            prefix_str: "0x1".to_owned(),
                            prefix_len: 32,
                            is_prefix_implicit: None,
                            stack_type_id: None,
                            stack_width: None,
                        },
                        ABIUnionVariant {
                            variant_ty: ABIType::StructRef {
                                struct_name: "MsgB".to_owned(),
                                type_args: Vec::new(),
                            },
                            prefix_str: "0x2".to_owned(),
                            prefix_len: 32,
                            is_prefix_implicit: None,
                            stack_type_id: None,
                            stack_width: None,
                        },
                    ],
                },
                type_params: None,
                custom_pack_unpack: None,
            },
        ];
        abi.incoming_messages = vec![ABIInternalMessage {
            body_ty: ABIType::AliasRef {
                alias_name: "Incoming".to_owned(),
                type_args: Vec::new(),
            },
            description: String::new(),
            minimal_msg_value: None,
            preferred_send_mode: None,
        }];

        let resolved = abi
            .resolve_incoming_message_structs()
            .expect("should resolve incoming messages");

        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].name, "MsgA");
        assert_eq!(resolved[1].name, "MsgB");
    }

    #[test]
    fn resolve_storage_struct_prefers_deployment_type() {
        let mut abi = empty_abi();
        abi.declarations = vec![
            ABIDeclaration::Struct {
                name: "Storage".to_owned(),
                type_params: None,
                prefix: None,
                fields: vec![],
                custom_pack_unpack: None,
            },
            ABIDeclaration::Struct {
                name: "DeploymentStorage".to_owned(),
                type_params: None,
                prefix: None,
                fields: vec![],
                custom_pack_unpack: None,
            },
        ];
        abi.storage = ABIStorage {
            storage_ty: Some(ABIType::StructRef {
                struct_name: "Storage".to_owned(),
                type_args: Vec::new(),
            }),
            storage_at_deployment_ty: Some(ABIType::StructRef {
                struct_name: "DeploymentStorage".to_owned(),
                type_args: Vec::new(),
            }),
        };

        let resolved = abi
            .resolve_storage_struct()
            .expect("should resolve storage")
            .expect("storage should exist");

        assert_eq!(resolved.name, "DeploymentStorage");
    }

    #[test]
    fn detects_cyclic_aliases_while_resolving_messages() {
        let mut abi = empty_abi();
        abi.declarations = vec![ABIDeclaration::Alias {
            name: "Incoming".to_owned(),
            target_ty: ABIType::AliasRef {
                alias_name: "Incoming".to_owned(),
                type_args: Vec::new(),
            },
            type_params: None,
            custom_pack_unpack: None,
        }];
        abi.incoming_messages = vec![ABIInternalMessage {
            body_ty: ABIType::AliasRef {
                alias_name: "Incoming".to_owned(),
                type_args: Vec::new(),
            },
            description: String::new(),
            minimal_msg_value: None,
            preferred_send_mode: None,
        }];

        let error = abi
            .resolve_incoming_message_structs()
            .expect_err("cyclic alias should fail");

        assert!(
            error
                .to_string()
                .contains("Cyclic ABI alias reference detected")
        );
    }

    #[test]
    fn abi_type_helpers_render_and_default_values() {
        let abi = empty_abi();
        let ty = ABIType::CellOf {
            inner: OneOrMany::One(Box::new(ABIType::IntN { n: 32 })),
        };
        assert_eq!(ty.render_type(), "Cell<int32>");
        assert_eq!(ty.render_param_type(), "int32");
        assert!(ty.is_typed_cell());
        assert_eq!(
            ty.typed_cell_payload_default_value(&abi).as_deref(),
            Some("0")
        );
        assert_eq!(ty.default_value(&abi), "0.toCell()");

        let nullable_union = ABIType::Union {
            variants: vec![
                ABIUnionVariant {
                    variant_ty: ABIType::NullLiteral,
                    prefix_str: String::new(),
                    prefix_len: 0,
                    is_prefix_implicit: None,
                    stack_type_id: None,
                    stack_width: None,
                },
                ABIUnionVariant {
                    variant_ty: ABIType::Callable,
                    prefix_str: String::new(),
                    prefix_len: 0,
                    is_prefix_implicit: None,
                    stack_type_id: None,
                    stack_width: None,
                },
            ],
        };
        assert_eq!(nullable_union.render_type(), "(continuation)?");
        assert_eq!(
            ABIType::AddressAny.default_value(&abi),
            "createAddressNone()"
        );
        assert_eq!(
            ABIType::BitsN { n: 32 }.default_value(&abi),
            "\"\" as bits32"
        );
        assert_eq!(ABIType::Callable.default_value(&abi), "fun () {}");
        assert_eq!(
            ABIType::Union {
                variants: vec![
                    ABIUnionVariant {
                        variant_ty: ABIType::IntN { n: 8 },
                        prefix_str: String::new(),
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: None,
                        stack_width: None,
                    },
                    ABIUnionVariant {
                        variant_ty: ABIType::Bool,
                        prefix_str: String::new(),
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: None,
                        stack_width: None,
                    },
                ],
            }
            .default_value(&abi),
            "0"
        );
        assert_eq!(
            ABIType::LispListOf {
                inner: OneOrMany::One(Box::new(ABIType::Bool)),
            }
            .default_value(&abi),
            "[]"
        );
    }

    #[test]
    fn defaults_resolve_structs_aliases_enums_and_field_defaults() {
        let mut abi = empty_abi();
        abi.declarations = vec![
            ABIDeclaration::Enum {
                name: "Color".to_owned(),
                encoded_as: ABIType::UintN { n: 2 },
                members: vec![
                    ABIEnumMember {
                        name: "Red".to_owned(),
                        value: "0".to_owned(),
                        description: String::new(),
                    },
                    ABIEnumMember {
                        name: "Blue".to_owned(),
                        value: "1".to_owned(),
                        description: String::new(),
                    },
                ],
                custom_pack_unpack: None,
            },
            ABIDeclaration::Struct {
                name: "Boxed".to_owned(),
                type_params: Some(vec!["T".to_owned()]),
                prefix: None,
                fields: vec![ABIStructField {
                    name: "item".to_owned(),
                    ty: ABIType::GenericT {
                        name_t: "T".to_owned(),
                    },
                    default_value: None,
                    description: String::new(),
                }],
                custom_pack_unpack: None,
            },
            ABIDeclaration::Alias {
                name: "UserId".to_owned(),
                target_ty: ABIType::IntN { n: 32 },
                type_params: None,
                custom_pack_unpack: None,
            },
            ABIDeclaration::Alias {
                name: "MaybeBoxed".to_owned(),
                target_ty: ABIType::StructRef {
                    struct_name: "Boxed".to_owned(),
                    type_args: vec![ABIType::Nullable {
                        inner: Box::new(ABIType::GenericT {
                            name_t: "T".to_owned(),
                        }),
                    }],
                },
                type_params: Some(vec!["T".to_owned()]),
                custom_pack_unpack: None,
            },
            ABIDeclaration::Struct {
                name: "Storage".to_owned(),
                type_params: None,
                prefix: None,
                fields: vec![
                    ABIStructField {
                        name: "owner".to_owned(),
                        ty: ABIType::AliasRef {
                            alias_name: "UserId".to_owned(),
                            type_args: Vec::new(),
                        },
                        default_value: None,
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "color".to_owned(),
                        ty: ABIType::EnumRef {
                            enum_name: "Color".to_owned(),
                        },
                        default_value: None,
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "maybeItem".to_owned(),
                        ty: ABIType::AliasRef {
                            alias_name: "MaybeBoxed".to_owned(),
                            type_args: vec![ABIType::Bool],
                        },
                        default_value: None,
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "nested".to_owned(),
                        ty: ABIType::StructRef {
                            struct_name: "Boxed".to_owned(),
                            type_args: Vec::new(),
                        },
                        default_value: Some(ABIConstValue::Object {
                            struct_name: "Boxed".to_owned(),
                            fields: vec![ABIConstValue::Null],
                        }),
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "opcode".to_owned(),
                        ty: ABIType::UintN { n: 32 },
                        default_value: Some(ABIConstValue::CastTo {
                            inner: Box::new(ABIConstValue::Int { v: "7".to_owned() }),
                            cast_to: ABIType::UintN { n: 32 },
                        }),
                        description: String::new(),
                    },
                ],
                custom_pack_unpack: None,
            },
        ];

        let storage_default = ABIType::StructRef {
            struct_name: "Storage".to_owned(),
            type_args: Vec::new(),
        }
        .default_value(&abi);

        assert_eq!(
            storage_default,
            "Storage { owner: (0 as UserId), color: Color.Red, maybeItem: {}, nested: Boxed { item: null }, opcode: (7 as uint32) }"
        );
    }

    #[test]
    fn default_value_falls_back_to_null_for_recursive_aliases() {
        let mut abi = empty_abi();
        abi.declarations = vec![ABIDeclaration::Alias {
            name: "Loop".to_owned(),
            target_ty: ABIType::AliasRef {
                alias_name: "Loop".to_owned(),
                type_args: Vec::new(),
            },
            type_params: None,
            custom_pack_unpack: None,
        }];

        let value = ABIType::AliasRef {
            alias_name: "Loop".to_owned(),
            type_args: Vec::new(),
        }
        .default_value(&abi);

        assert_eq!(value, "null");
    }
}
