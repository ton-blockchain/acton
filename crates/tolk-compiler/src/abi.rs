use crate::source_map::SourceMap;
pub use crate::types_kernel::{Ty, UnionVariant};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

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
        cast_to: Ty,
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
    pub ty: Ty,

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

        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        overrides_client_type: bool,
    },

    #[serde(rename = "alias")]
    Alias {
        name: String,

        target_ty: Ty,

        #[serde(skip_serializing_if = "Option::is_none")]
        type_params: Option<Vec<String>>,

        #[serde(skip_serializing_if = "Option::is_none")]
        custom_pack_unpack: Option<ABICustomPackUnpack>,
    },

    #[serde(rename = "enum")]
    Enum {
        name: String,
        encoded_as: Ty,
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
    pub ty: Ty,
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
    pub return_ty: Ty,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIInternalMessage {
    pub body_ty: Ty,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIExternalMessage {
    pub body_ty: Ty,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIOutgoingMessage {
    pub body_ty: Ty,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ABIStorage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_ty: Option<Ty>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_at_deployment_ty: Option<Ty>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub err_code: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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

    pub compiler_name: String,
    pub compiler_version: String,
}

#[derive(Debug, Clone)]
pub struct ABIResolvedStruct {
    pub name: String,
    pub fields: Vec<ABIStructField>,
}

impl ContractABI {
    #[must_use]
    pub fn find_get_method_by_id(&self, id: i32) -> Option<&ABIGetMethod> {
        self.get_methods
            .iter()
            .find(|method| method.tvm_method_id == id)
    }

    #[must_use]
    pub fn find_message_name_by_opcode(&self, opcode: u32) -> Option<&str> {
        self.declarations.iter().find_map(|declaration| {
            let ABIDeclaration::Struct {
                name,
                type_params,
                prefix,
                ..
            } = declaration
            else {
                return None;
            };

            if type_params
                .as_ref()
                .is_some_and(|params| !params.is_empty())
            {
                return None;
            }

            let matches_opcode = prefix.as_ref().is_some_and(|prefix| {
                prefix.prefix_len == 32
                    && parse_abi_prefix_number(&prefix.prefix_str) == Some(opcode)
            });
            matches_opcode.then_some(name.as_str())
        })
    }

    #[must_use]
    pub fn find_message_name_by_opcode_with_symbols<'a>(
        symbols: &'a SourceMap,
        abi: Option<&'a Self>,
        opcode: u32,
    ) -> Option<&'a str> {
        symbols
            .find_message_name_by_opcode(opcode)
            .or_else(|| abi.and_then(|abi| abi.find_message_name_by_opcode(opcode)))
    }

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
        ty: &Ty,
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

fn parse_abi_prefix_number(prefix: &str) -> Option<u32> {
    let prefix = prefix.trim();
    if prefix.is_empty() {
        return None;
    }
    let parsed = if let Some(hex) = prefix
        .strip_prefix("0x")
        .or_else(|| prefix.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok()?
    } else {
        prefix.parse::<u64>().ok()?
    };
    u32::try_from(parsed).ok()
}

impl Ty {
    #[must_use]
    pub fn render_param_type(&self) -> String {
        match self {
            Ty::CellOf { inner } => inner.render_type(),
            _ => self.render_type(),
        }
    }

    #[must_use]
    pub fn render_type(&self) -> String {
        self.to_string()
    }

    #[must_use]
    pub const fn is_typed_cell(&self) -> bool {
        matches!(self, Ty::CellOf { .. })
    }

    #[must_use]
    pub fn typed_cell_payload_default_value(&self, abi: &ContractABI) -> Option<String> {
        match self {
            Ty::CellOf { inner } => Some(default_value_impl(
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
    ty: &Ty,
    visited_aliases: &mut HashSet<String>,
    seen_structs: &mut HashSet<String>,
    resolved: &mut Vec<ABIResolvedStruct>,
) -> anyhow::Result<()> {
    match ty {
        Ty::StructRef { struct_name, .. } => {
            let fields = find_struct_decl(abi, struct_name)
                .ok_or_else(|| anyhow!("Struct {struct_name} referenced by ABI was not found"))?;
            if seen_structs.insert(struct_name.clone()) {
                resolved.push(to_resolved_struct(struct_name, fields));
            }
            Ok(())
        }
        Ty::AliasRef { alias_name, .. } => {
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
        Ty::Union { variants, .. } => {
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
        Ty::Nullable { inner, .. } | Ty::CellOf { inner } | Ty::LispListOf { inner } => {
            collect_structs_from_type(abi, inner, visited_aliases, seen_structs, resolved)
        }
        _ => anyhow::bail!(
            "Unsupported ABI type {} while resolving contract wrapper types",
            ty.render_type()
        ),
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

fn find_alias_decl<'a>(abi: &'a ContractABI, target_name: &str) -> Option<&'a Ty> {
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
) -> Option<(Option<&'a [String]>, &'a Ty)> {
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
) -> Option<(&'a Ty, &'a [ABIEnumMember])> {
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

fn render_named_type(name: &str, type_args: Option<&[Ty]>) -> String {
    let type_args = type_args.unwrap_or(&[]);
    if type_args.is_empty() {
        name.to_owned()
    } else {
        format!(
            "{name}<{}>",
            type_args
                .iter()
                .map(Ty::render_type)
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn default_value_impl(
    abi: &ContractABI,
    ty: &Ty,
    bindings: &BTreeMap<String, Ty>,
    visited_defs: &mut HashSet<String>,
) -> String {
    match ty {
        Ty::Int
        | Ty::Coins
        | Ty::UintN { .. }
        | Ty::IntN { .. }
        | Ty::VaruintN { .. }
        | Ty::VarintN { .. } => "0".to_owned(),
        Ty::Bool => "false".to_owned(),
        Ty::Cell => "createEmptyCell()".to_owned(),
        Ty::Slice | Ty::Remaining => "createEmptySlice()".to_owned(),
        Ty::Builder => "beginCell()".to_owned(),
        Ty::Callable => "fun () {}".to_owned(),
        Ty::String => "\"\"".to_owned(),
        Ty::Void => "()".to_owned(),
        Ty::Address => "address(\"EQD__________________________________________0vo\")".to_owned(),
        Ty::AddressAny | Ty::AddressExt => "createAddressNone()".to_owned(),
        Ty::AddressOpt | Ty::NullLiteral | Ty::Nullable { .. } | Ty::Unknown => "null".to_owned(),
        Ty::BitsN { n } => format!("\"\" as bits{n}"),
        Ty::ArrayOf { .. } | Ty::LispListOf { .. } | Ty::MapKV { .. } => "[]".to_owned(),
        Ty::Tensor { items } => format!(
            "({})",
            items
                .iter()
                .map(|item| default_value_impl(abi, item, bindings, visited_defs))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Ty::ShapedTuple { items } => format!(
            "[{}]",
            items
                .iter()
                .map(|item| default_value_impl(abi, item, bindings, visited_defs))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Ty::GenericT { .. } => "{}".to_owned(),
        Ty::StructRef {
            struct_name,
            type_args,
        } => default_struct_value(abi, struct_name, type_args.as_deref(), visited_defs),
        Ty::EnumRef { enum_name } => default_enum_value(abi, enum_name),
        Ty::AliasRef {
            alias_name,
            type_args,
        } => default_alias_value(abi, alias_name, type_args.as_deref(), visited_defs),
        Ty::CellOf { inner } => format!(
            "{}.toCell()",
            default_value_impl(abi, inner, bindings, visited_defs)
        ),
        Ty::Union { variants, .. } => default_union_value(abi, variants, bindings, visited_defs),
    }
}

fn default_struct_value(
    abi: &ContractABI,
    struct_name: &str,
    type_args: Option<&[Ty]>,
    visited_defs: &mut HashSet<String>,
) -> String {
    let qualified_name = render_named_type(struct_name, type_args);
    let visit_key = format!("struct:{qualified_name}");
    if !visited_defs.insert(visit_key.clone()) {
        return "null".to_owned();
    }

    let result = find_struct_decl_full(abi, struct_name).map_or_else(
        || "null".to_owned(),
        |(type_params, fields)| {
            if type_args.is_some_and(|args| !args.is_empty())
                || type_params.is_some_and(|params| !params.is_empty())
            {
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
        },
    );

    visited_defs.remove(&visit_key);
    result
}

fn default_alias_value(
    abi: &ContractABI,
    alias_name: &str,
    type_args: Option<&[Ty]>,
    visited_defs: &mut HashSet<String>,
) -> String {
    let qualified_name = render_named_type(alias_name, type_args);
    let visit_key = format!("alias:{qualified_name}");
    if !visited_defs.insert(visit_key.clone()) {
        return "null".to_owned();
    }

    let result = find_alias_decl_full(abi, alias_name).map_or_else(
        || "null".to_owned(),
        |(type_params, target_ty)| {
            if type_args.is_some_and(|args| !args.is_empty())
                || type_params.is_some_and(|params| !params.is_empty())
            {
                return "{}".to_owned();
            }

            let target_default = default_value_impl(abi, target_ty, &BTreeMap::new(), visited_defs);
            if target_default == "null" {
                target_default
            } else {
                format!("({target_default} as {qualified_name})")
            }
        },
    );

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
    variants: &[UnionVariant],
    bindings: &BTreeMap<String, Ty>,
    visited_defs: &mut HashSet<String>,
) -> String {
    let Some(first_variant) = variants.first() else {
        return "null".to_owned();
    };

    if variants
        .iter()
        .any(|variant| matches!(variant.variant_ty, Ty::NullLiteral))
    {
        return "null".to_owned();
    }

    default_value_impl(abi, &first_variant.variant_ty, bindings, visited_defs)
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
        ABIStorage, ABIStructField, ABIThrownError, ABIThrownErrorKind, ContractABI, Ty,
        UnionVariant,
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
                description: String::new(),
            },
            get_methods: Vec::new(),
            thrown_errors: Vec::new(),
            compiler_name: "tolk".to_owned(),
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
                    ty: Ty::IntN { n: 32 },
                    default_value: None,
                    description: String::new(),
                }],
                custom_pack_unpack: None,
                overrides_client_type: false,
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
                overrides_client_type: false,
            },
            ABIDeclaration::Alias {
                name: "Incoming".to_owned(),
                target_ty: Ty::Union {
                    variants: vec![
                        UnionVariant {
                            variant_ty: Ty::StructRef {
                                struct_name: "MsgA".to_owned(),
                                type_args: None,
                            },
                            prefix_str: "0x1".to_owned(),
                            prefix_len: 32,
                            is_prefix_implicit: None,
                            stack_type_id: None,
                            stack_width: None,
                        },
                        UnionVariant {
                            variant_ty: Ty::StructRef {
                                struct_name: "MsgB".to_owned(),
                                type_args: None,
                            },
                            prefix_str: "0x2".to_owned(),
                            prefix_len: 32,
                            is_prefix_implicit: None,
                            stack_type_id: None,
                            stack_width: None,
                        },
                    ],
                    stack_width: None,
                },
                type_params: None,
                custom_pack_unpack: None,
            },
        ];
        abi.incoming_messages = vec![ABIInternalMessage {
            body_ty: Ty::AliasRef {
                alias_name: "Incoming".to_owned(),
                type_args: None,
            },
            description: String::new(),
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
                overrides_client_type: false,
            },
            ABIDeclaration::Struct {
                name: "DeploymentStorage".to_owned(),
                type_params: None,
                prefix: None,
                fields: vec![],
                custom_pack_unpack: None,
                overrides_client_type: false,
            },
        ];
        abi.storage = ABIStorage {
            storage_ty: Some(Ty::StructRef {
                struct_name: "Storage".to_owned(),
                type_args: None,
            }),
            storage_at_deployment_ty: Some(Ty::StructRef {
                struct_name: "DeploymentStorage".to_owned(),
                type_args: None,
            }),
            description: String::new(),
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
            target_ty: Ty::AliasRef {
                alias_name: "Incoming".to_owned(),
                type_args: None,
            },
            type_params: None,
            custom_pack_unpack: None,
        }];
        abi.incoming_messages = vec![ABIInternalMessage {
            body_ty: Ty::AliasRef {
                alias_name: "Incoming".to_owned(),
                type_args: None,
            },
            description: String::new(),
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
        let ty = Ty::CellOf {
            inner: Box::new(Ty::IntN { n: 32 }),
        };
        assert_eq!(ty.render_type(), "Cell<int32>");
        assert_eq!(ty.render_param_type(), "int32");
        assert!(ty.is_typed_cell());
        assert_eq!(
            ty.typed_cell_payload_default_value(&abi).as_deref(),
            Some("0")
        );
        assert_eq!(ty.default_value(&abi), "0.toCell()");

        let nullable_union = Ty::Union {
            variants: vec![
                UnionVariant {
                    variant_ty: Ty::NullLiteral,
                    prefix_str: String::new(),
                    prefix_len: 0,
                    is_prefix_implicit: None,
                    stack_type_id: None,
                    stack_width: None,
                },
                UnionVariant {
                    variant_ty: Ty::Callable,
                    prefix_str: String::new(),
                    prefix_len: 0,
                    is_prefix_implicit: None,
                    stack_type_id: None,
                    stack_width: None,
                },
            ],
            stack_width: None,
        };
        assert_eq!(nullable_union.render_type(), "null | continuation");
        assert_eq!(Ty::AddressAny.default_value(&abi), "createAddressNone()");
        assert_eq!(Ty::BitsN { n: 32 }.default_value(&abi), "\"\" as bits32");
        assert_eq!(Ty::Callable.default_value(&abi), "fun () {}");
        assert_eq!(
            Ty::Union {
                variants: vec![
                    UnionVariant {
                        variant_ty: Ty::IntN { n: 8 },
                        prefix_str: String::new(),
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: None,
                        stack_width: None,
                    },
                    UnionVariant {
                        variant_ty: Ty::Bool,
                        prefix_str: String::new(),
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: None,
                        stack_width: None,
                    },
                ],
                stack_width: None,
            }
            .default_value(&abi),
            "0"
        );
        assert_eq!(
            Ty::LispListOf {
                inner: Box::new(Ty::Bool),
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
                encoded_as: Ty::UintN { n: 2 },
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
                    ty: Ty::GenericT {
                        name_t: "T".to_owned(),
                    },
                    default_value: None,
                    description: String::new(),
                }],
                custom_pack_unpack: None,
                overrides_client_type: false,
            },
            ABIDeclaration::Alias {
                name: "UserId".to_owned(),
                target_ty: Ty::IntN { n: 32 },
                type_params: None,
                custom_pack_unpack: None,
            },
            ABIDeclaration::Alias {
                name: "MaybeBoxed".to_owned(),
                target_ty: Ty::StructRef {
                    struct_name: "Boxed".to_owned(),
                    type_args: Some(vec![Ty::Nullable {
                        inner: Box::new(Ty::GenericT {
                            name_t: "T".to_owned(),
                        }),
                        stack_type_id: None,
                        stack_width: None,
                    }]),
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
                        ty: Ty::AliasRef {
                            alias_name: "UserId".to_owned(),
                            type_args: None,
                        },
                        default_value: None,
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "color".to_owned(),
                        ty: Ty::EnumRef {
                            enum_name: "Color".to_owned(),
                        },
                        default_value: None,
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "maybeItem".to_owned(),
                        ty: Ty::AliasRef {
                            alias_name: "MaybeBoxed".to_owned(),
                            type_args: Some(vec![Ty::Bool]),
                        },
                        default_value: None,
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "nested".to_owned(),
                        ty: Ty::StructRef {
                            struct_name: "Boxed".to_owned(),
                            type_args: None,
                        },
                        default_value: Some(ABIConstValue::Object {
                            struct_name: "Boxed".to_owned(),
                            fields: vec![ABIConstValue::Null],
                        }),
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "opcode".to_owned(),
                        ty: Ty::UintN { n: 32 },
                        default_value: Some(ABIConstValue::CastTo {
                            inner: Box::new(ABIConstValue::Int { v: "7".to_owned() }),
                            cast_to: Ty::UintN { n: 32 },
                        }),
                        description: String::new(),
                    },
                ],
                custom_pack_unpack: None,
                overrides_client_type: false,
            },
        ];

        let storage_default = Ty::StructRef {
            struct_name: "Storage".to_owned(),
            type_args: None,
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
            target_ty: Ty::AliasRef {
                alias_name: "Loop".to_owned(),
                type_args: None,
            },
            type_params: None,
            custom_pack_unpack: None,
        }];

        let value = Ty::AliasRef {
            alias_name: "Loop".to_owned(),
            type_args: None,
        }
        .default_value(&abi);

        assert_eq!(value, "null");
    }
}
