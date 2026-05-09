use crate::source_map::SourceMap;
pub use crate::types_kernel::{AliasInstantiation, StructInstantiation, Ty, TyIdx, UnionVariant};
use crate::types_kernel::{TyResolver, render_param_ty, render_ty};
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
        cast_to_ty_idx: TyIdx,
    },

    #[serde(rename = "null")]
    Null,
}

/// =======================
/// ABI declarations exported by the compiler.
/// =======================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIOpcode {
    pub prefix_num: u64,
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
    pub ty_idx: TyIdx,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_ty_idx: Option<TyIdx>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<ABIConstValue>,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

impl ABIStructField {
    #[must_use]
    pub fn client_or_declared_ty_idx(&self) -> TyIdx {
        self.client_ty_idx.unwrap_or(self.ty_idx)
    }
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

        ty_idx: TyIdx,

        #[serde(skip_serializing_if = "Option::is_none")]
        type_params: Option<Vec<String>>,

        #[serde(skip_serializing_if = "Option::is_none")]
        prefix: Option<ABIOpcode>,

        fields: Vec<ABIStructField>,

        #[serde(skip_serializing_if = "Option::is_none")]
        custom_pack_unpack: Option<ABICustomPackUnpack>,

        #[serde(default, skip_serializing_if = "String::is_empty")]
        description: String,
    },

    #[serde(rename = "alias")]
    Alias {
        name: String,

        ty_idx: TyIdx,

        target_ty_idx: TyIdx,

        #[serde(skip_serializing_if = "Option::is_none")]
        type_params: Option<Vec<String>>,

        #[serde(skip_serializing_if = "Option::is_none")]
        custom_pack_unpack: Option<ABICustomPackUnpack>,

        #[serde(default, skip_serializing_if = "String::is_empty")]
        description: String,
    },

    #[serde(rename = "enum")]
    Enum {
        name: String,
        ty_idx: TyIdx,
        encoded_as_ty_idx: TyIdx,
        members: Vec<ABIEnumMember>,

        #[serde(skip_serializing_if = "Option::is_none")]
        custom_pack_unpack: Option<ABICustomPackUnpack>,

        #[serde(default, skip_serializing_if = "String::is_empty")]
        description: String,
    },
}

/// =======================
/// ABI messages / storage / getters / errors / constants
/// =======================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIFunctionParameter {
    pub name: String,
    pub ty_idx: TyIdx,
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
    pub return_ty_idx: TyIdx,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIInternalMessage {
    pub body_ty_idx: TyIdx,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIExternalMessage {
    pub body_ty_idx: TyIdx,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIOutgoingMessage {
    pub body_ty_idx: TyIdx,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ABIStorage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_ty_idx: Option<TyIdx>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_at_deployment_ty_idx: Option<TyIdx>,
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

    pub unique_types: Vec<Ty>,
    pub struct_instantiations: Vec<StructInstantiation>,
    pub alias_instantiations: Vec<AliasInstantiation>,
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
    pub prefix: Option<ABIOpcode>,
    pub fields: Vec<ABIStructField>,
}

impl ContractABI {
    #[must_use]
    pub fn ty_by_idx(&self, ty_idx: TyIdx) -> Option<&Ty> {
        self.unique_types.get(ty_idx)
    }

    #[must_use]
    pub fn render_type(&self, ty_idx: TyIdx) -> String {
        render_ty(self, ty_idx)
    }

    #[must_use]
    pub fn render_param_type(&self, ty_idx: TyIdx) -> String {
        render_param_ty(self, ty_idx)
    }

    #[must_use]
    pub fn is_typed_cell(&self, ty_idx: TyIdx) -> bool {
        self.ty_by_idx(ty_idx).is_some_and(Ty::is_typed_cell)
    }

    #[must_use]
    pub fn typed_cell_payload_default_value(&self, ty_idx: TyIdx) -> Option<String> {
        let Ty::CellOf { inner_ty_idx } = self.ty_by_idx(ty_idx)? else {
            return None;
        };
        Some(default_value_impl(
            self,
            *inner_ty_idx,
            &BTreeMap::new(),
            &mut HashSet::new(),
        ))
    }

    #[must_use]
    pub fn default_value(&self, ty_idx: TyIdx) -> String {
        default_value_impl(self, ty_idx, &BTreeMap::new(), &mut HashSet::new())
    }

    pub fn struct_fields_of(&self, ty_idx: TyIdx) -> anyhow::Result<Vec<ABIStructField>> {
        let ty = self
            .ty_by_idx(ty_idx)
            .ok_or_else(|| anyhow!("ABI ty_idx {ty_idx} was not found"))?;
        let Ty::StructRef { struct_name, .. } = ty else {
            anyhow::bail!(
                "expected StructRef at ty_idx={ty_idx}, got {}",
                self.render_type(ty_idx)
            );
        };

        if let Some(inst) = self
            .struct_instantiations
            .iter()
            .find(|inst| inst.ty_idx == ty_idx)
        {
            let (_, fields) = find_struct_decl(self, struct_name)
                .ok_or_else(|| anyhow!("Struct {struct_name} referenced by ABI was not found"))?;
            if fields.len() != inst.monomorphic_fields_ty_idx.len() {
                anyhow::bail!(
                    "struct instantiation `{}` has {} monomorphic fields, expected {}",
                    inst.struct_name,
                    inst.monomorphic_fields_ty_idx.len(),
                    fields.len()
                );
            }
            return Ok(fields
                .iter()
                .zip(&inst.monomorphic_fields_ty_idx)
                .map(|(field, &field_ty_idx)| ABIStructField {
                    ty_idx: field_ty_idx,
                    ..field.clone()
                })
                .collect());
        }

        let (_, fields) = find_struct_decl(self, struct_name)
            .ok_or_else(|| anyhow!("Struct {struct_name} referenced by ABI was not found"))?;
        Ok(fields.to_vec())
    }

    pub fn alias_target_of(&self, ty_idx: TyIdx) -> anyhow::Result<TyIdx> {
        let ty = self
            .ty_by_idx(ty_idx)
            .ok_or_else(|| anyhow!("ABI ty_idx {ty_idx} was not found"))?;
        let Ty::AliasRef { alias_name, .. } = ty else {
            anyhow::bail!(
                "expected AliasRef at ty_idx={ty_idx}, got {}",
                self.render_type(ty_idx)
            );
        };

        if let Some(inst) = self
            .alias_instantiations
            .iter()
            .find(|inst| inst.ty_idx == ty_idx)
        {
            return Ok(inst.monomorphic_target_ty_idx);
        }

        find_alias_decl(self, alias_name)
            .ok_or_else(|| anyhow!("Alias {alias_name} referenced by ABI was not found"))
    }

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
                prefix.prefix_len == 32 && prefix.prefix_num == u64::from(opcode)
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
        let Some(storage_ty_idx) = self
            .storage
            .storage_at_deployment_ty_idx
            .or(self.storage.storage_ty_idx)
        else {
            return Ok(None);
        };

        Ok(Some(self.resolve_single_struct(storage_ty_idx, "storage")?))
    }

    pub fn resolve_incoming_message_structs(&self) -> anyhow::Result<Vec<ABIResolvedStruct>> {
        let mut resolved = Vec::new();
        let mut seen_structs = HashSet::new();

        for message in &self.incoming_messages {
            collect_structs_from_type(
                self,
                message.body_ty_idx,
                &mut HashSet::new(),
                &mut seen_structs,
                &mut resolved,
            )?;
        }

        Ok(resolved)
    }

    pub fn resolve_incoming_external_message_structs(
        &self,
    ) -> anyhow::Result<Vec<ABIResolvedStruct>> {
        let mut resolved = Vec::new();
        let mut seen_structs = HashSet::new();

        for message in &self.incoming_external {
            collect_structs_from_type(
                self,
                message.body_ty_idx,
                &mut HashSet::new(),
                &mut seen_structs,
                &mut resolved,
            )?;
        }

        Ok(resolved)
    }

    pub fn resolve_single_struct(
        &self,
        ty_idx: TyIdx,
        context: &str,
    ) -> anyhow::Result<ABIResolvedStruct> {
        let mut resolved = Vec::new();
        let mut seen_structs = HashSet::new();
        collect_structs_from_type(
            self,
            ty_idx,
            &mut HashSet::new(),
            &mut seen_structs,
            &mut resolved,
        )?;

        let mut structs = resolved.into_iter();
        let Some(first) = structs.next() else {
            anyhow::bail!(
                "Failed to resolve {} type {} into a concrete struct",
                context,
                self.render_type(ty_idx)
            );
        };

        if let Some(other) = structs.next() {
            anyhow::bail!(
                "{} type {} resolves to multiple structs ({}, {})",
                context,
                self.render_type(ty_idx),
                first.name,
                other.name
            );
        }

        Ok(first)
    }
}

impl TyResolver for ContractABI {
    fn ty_by_idx(&self, ty_idx: TyIdx) -> Option<&Ty> {
        self.unique_types.get(ty_idx)
    }

    fn struct_field_ty_indices(&self, ty_idx: TyIdx) -> Option<Vec<TyIdx>> {
        self.struct_fields_of(ty_idx)
            .ok()
            .map(|fields| fields.into_iter().map(|field| field.ty_idx).collect())
    }

    fn alias_target_ty_idx(&self, ty_idx: TyIdx) -> Option<TyIdx> {
        self.alias_target_of(ty_idx).ok()
    }
}

fn collect_structs_from_type(
    abi: &ContractABI,
    ty_idx: TyIdx,
    visited_aliases: &mut HashSet<TyIdx>,
    seen_structs: &mut HashSet<String>,
    resolved: &mut Vec<ABIResolvedStruct>,
) -> anyhow::Result<()> {
    let ty = abi
        .ty_by_idx(ty_idx)
        .ok_or_else(|| anyhow!("ABI ty_idx {ty_idx} was not found"))?;
    match ty {
        Ty::StructRef { struct_name, .. } => {
            let (prefix, _) = find_struct_decl(abi, struct_name)
                .ok_or_else(|| anyhow!("Struct {struct_name} referenced by ABI was not found"))?;
            let fields = abi.struct_fields_of(ty_idx)?;
            if seen_structs.insert(struct_name.clone()) {
                resolved.push(to_resolved_struct(struct_name, prefix, &fields));
            }
            Ok(())
        }
        Ty::AliasRef { alias_name, .. } => {
            if !visited_aliases.insert(ty_idx) {
                anyhow::bail!("Cyclic ABI alias reference detected for {alias_name}");
            }

            let result = abi.alias_target_of(ty_idx).and_then(|target_ty_idx| {
                collect_structs_from_type(
                    abi,
                    target_ty_idx,
                    visited_aliases,
                    seen_structs,
                    resolved,
                )
            });

            visited_aliases.remove(&ty_idx);
            result
        }
        Ty::Union { variants, .. } => {
            for variant in variants {
                collect_structs_from_type(
                    abi,
                    variant.variant_ty_idx,
                    visited_aliases,
                    seen_structs,
                    resolved,
                )?;
            }
            Ok(())
        }
        Ty::Nullable { inner_ty_idx, .. }
        | Ty::CellOf { inner_ty_idx }
        | Ty::LispListOf { inner_ty_idx } => {
            collect_structs_from_type(abi, *inner_ty_idx, visited_aliases, seen_structs, resolved)
        }
        // Primitive bodies (slice, cell, etc.) yield no structs — silently skip.
        // Callers that require a struct (e.g. `resolve_single_struct`) bail when `resolved`
        // ends up empty.
        _ => Ok(()),
    }
}

fn find_struct_decl<'a>(
    abi: &'a ContractABI,
    target_name: &str,
) -> Option<(Option<&'a ABIOpcode>, &'a [ABIStructField])> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Struct {
            name,
            prefix,
            fields,
            ..
        } if name == target_name => Some((prefix.as_ref(), fields.as_slice())),
        _ => None,
    })
}

fn find_alias_decl(abi: &ContractABI, target_name: &str) -> Option<TyIdx> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Alias {
            name,
            target_ty_idx,
            ..
        } if name == target_name => Some(*target_ty_idx),
        _ => None,
    })
}

fn to_resolved_struct(
    name: &str,
    prefix: Option<&ABIOpcode>,
    fields: &[ABIStructField],
) -> ABIResolvedStruct {
    ABIResolvedStruct {
        name: name.to_owned(),
        prefix: prefix.cloned(),
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
) -> Option<(Option<&'a [String]>, TyIdx)> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Alias {
            name,
            target_ty_idx,
            type_params,
            ..
        } if name == target_name => Some((type_params.as_deref(), *target_ty_idx)),
        _ => None,
    })
}

fn find_enum_decl<'a>(
    abi: &'a ContractABI,
    target_name: &str,
) -> Option<(TyIdx, &'a [ABIEnumMember])> {
    abi.declarations.iter().find_map(|decl| match decl {
        ABIDeclaration::Enum {
            name,
            encoded_as_ty_idx,
            members,
            ..
        } if name == target_name => Some((*encoded_as_ty_idx, members.as_slice())),
        _ => None,
    })
}

fn default_value_impl(
    abi: &ContractABI,
    ty_idx: TyIdx,
    bindings: &BTreeMap<String, Ty>,
    visited_defs: &mut HashSet<String>,
) -> String {
    let Some(ty) = abi.ty_by_idx(ty_idx) else {
        return "null".to_owned();
    };
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
        Ty::Tensor { items_ty_idx } => format!(
            "({})",
            items_ty_idx
                .iter()
                .map(|&item_ty_idx| default_value_impl(abi, item_ty_idx, bindings, visited_defs))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Ty::ShapedTuple { items_ty_idx } => format!(
            "[{}]",
            items_ty_idx
                .iter()
                .map(|&item_ty_idx| default_value_impl(abi, item_ty_idx, bindings, visited_defs))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Ty::GenericT { .. } => "{}".to_owned(),
        Ty::StructRef { struct_name, .. } => {
            default_struct_value(abi, ty_idx, struct_name, visited_defs)
        }
        Ty::EnumRef { enum_name } => default_enum_value(abi, enum_name),
        Ty::AliasRef { alias_name, .. } => {
            default_alias_value(abi, ty_idx, alias_name, visited_defs)
        }
        Ty::CellOf { inner_ty_idx } => format!(
            "{}.toCell()",
            default_value_impl(abi, *inner_ty_idx, bindings, visited_defs)
        ),
        Ty::Union { variants, .. } => default_union_value(abi, variants, bindings, visited_defs),
    }
}

fn default_struct_value(
    abi: &ContractABI,
    ty_idx: TyIdx,
    struct_name: &str,
    visited_defs: &mut HashSet<String>,
) -> String {
    let qualified_name = abi.render_type(ty_idx);
    let visit_key = format!("struct:{qualified_name}");
    if !visited_defs.insert(visit_key.clone()) {
        return "null".to_owned();
    }

    let result = find_struct_decl_full(abi, struct_name).map_or_else(
        || "null".to_owned(),
        |(type_params, fields)| {
            let fields = abi
                .struct_fields_of(ty_idx)
                .unwrap_or_else(|_| fields.to_vec());
            if type_params.is_some_and(|params| !params.is_empty())
                && abi
                    .struct_instantiations
                    .iter()
                    .all(|inst| inst.ty_idx != ty_idx)
            {
                return "{}".to_owned();
            }

            let rendered_fields = fields
                .iter()
                .map(|field| {
                    let value = match &field.default_value {
                        Some(default_value) => render_const_value(abi, default_value),
                        None => {
                            default_value_impl(abi, field.ty_idx, &BTreeMap::new(), visited_defs)
                        }
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
    ty_idx: TyIdx,
    alias_name: &str,
    visited_defs: &mut HashSet<String>,
) -> String {
    let qualified_name = abi.render_type(ty_idx);
    let visit_key = format!("alias:{qualified_name}");
    if !visited_defs.insert(visit_key.clone()) {
        return "null".to_owned();
    }

    let result = find_alias_decl_full(abi, alias_name).map_or_else(
        || "null".to_owned(),
        |(type_params, target_ty_idx)| {
            let target_ty_idx = abi.alias_target_of(ty_idx).unwrap_or(target_ty_idx);
            if type_params.is_some_and(|params| !params.is_empty())
                && abi
                    .alias_instantiations
                    .iter()
                    .all(|inst| inst.ty_idx != ty_idx)
            {
                return "{}".to_owned();
            }

            let target_default =
                default_value_impl(abi, target_ty_idx, &BTreeMap::new(), visited_defs);
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
        .any(|variant| matches!(abi.ty_by_idx(variant.variant_ty_idx), Some(Ty::NullLiteral)))
    {
        return "null".to_owned();
    }

    default_value_impl(abi, first_variant.variant_ty_idx, bindings, visited_defs)
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
        ABIConstValue::CastTo {
            inner,
            cast_to_ty_idx,
        } => format!(
            "({} as {})",
            render_const_value(abi, inner),
            abi.render_type(*cast_to_ty_idx)
        ),
        ABIConstValue::Null => "null".to_owned(),
    }
}

fn render_const_object_value(
    abi: &ContractABI,
    struct_name: &str,
    fields: &[ABIConstValue],
) -> String {
    let Some((_, struct_fields)) = find_struct_decl(abi, struct_name) else {
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
        ABIStorage, ABIStructField, ABIThrownError, ABIThrownErrorKind, AliasInstantiation,
        ContractABI, StructInstantiation, Ty, TyIdx, UnionVariant,
    };

    fn empty_abi() -> ContractABI {
        ContractABI {
            abi_schema_version: "1.0".to_owned(),
            contract_name: "Test".to_owned(),
            author: String::new(),
            version: String::new(),
            description: String::new(),
            unique_types: Vec::new(),
            struct_instantiations: Vec::new(),
            alias_instantiations: Vec::new(),
            declarations: Vec::new(),
            incoming_messages: Vec::new(),
            incoming_external: Vec::new(),
            outgoing_messages: Vec::new(),
            emitted_events: Vec::new(),
            storage: ABIStorage {
                storage_ty_idx: None,
                storage_at_deployment_ty_idx: None,
            },
            get_methods: Vec::new(),
            thrown_errors: Vec::new(),
            compiler_name: "tolk".to_owned(),
            compiler_version: "test".to_owned(),
        }
    }

    fn add_ty(abi: &mut ContractABI, ty: Ty) -> TyIdx {
        if let Some(idx) = abi.unique_types.iter().position(|existing| existing == &ty) {
            return idx;
        }
        let idx = abi.unique_types.len();
        abi.unique_types.push(ty);
        idx
    }

    fn struct_ref(abi: &mut ContractABI, struct_name: &str) -> TyIdx {
        add_ty(
            abi,
            Ty::StructRef {
                struct_name: struct_name.to_owned(),
                type_args_ty_idx: None,
            },
        )
    }

    fn alias_ref(abi: &mut ContractABI, alias_name: &str) -> TyIdx {
        add_ty(
            abi,
            Ty::AliasRef {
                alias_name: alias_name.to_owned(),
                type_args_ty_idx: None,
            },
        )
    }

    fn enum_ref(abi: &mut ContractABI, enum_name: &str) -> TyIdx {
        add_ty(
            abi,
            Ty::EnumRef {
                enum_name: enum_name.to_owned(),
            },
        )
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
                    "ty_idx": 1,
                    "default_value": {"kind":"int","v":"10"}
                }],
                "return_ty_idx": 1
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
        let msg_a_ty_idx = struct_ref(&mut abi, "MsgA");
        let msg_b_ty_idx = struct_ref(&mut abi, "MsgB");
        let value_ty_idx = add_ty(&mut abi, Ty::IntN { n: 32 });
        let incoming_alias_ty_idx = alias_ref(&mut abi, "Incoming");
        let incoming_union_ty_idx = add_ty(
            &mut abi,
            Ty::Union {
                variants: vec![
                    UnionVariant {
                        variant_ty_idx: msg_a_ty_idx,
                        prefix_num: 0x1,
                        prefix_len: 32,
                        is_prefix_implicit: None,
                        stack_type_id: None,
                        stack_width: None,
                    },
                    UnionVariant {
                        variant_ty_idx: msg_b_ty_idx,
                        prefix_num: 0x2,
                        prefix_len: 32,
                        is_prefix_implicit: None,
                        stack_type_id: None,
                        stack_width: None,
                    },
                ],
                stack_width: None,
            },
        );
        abi.declarations = vec![
            ABIDeclaration::Struct {
                name: "MsgA".to_owned(),
                ty_idx: msg_a_ty_idx,
                type_params: None,
                prefix: Some(ABIOpcode {
                    prefix_num: 0x1,
                    prefix_len: 32,
                }),
                fields: vec![ABIStructField {
                    name: "value".to_owned(),
                    ty_idx: value_ty_idx,
                    client_ty_idx: None,
                    default_value: None,
                    description: String::new(),
                }],
                custom_pack_unpack: None,
                description: String::new(),
            },
            ABIDeclaration::Struct {
                name: "MsgB".to_owned(),
                ty_idx: msg_b_ty_idx,
                type_params: None,
                prefix: Some(ABIOpcode {
                    prefix_num: 0x2,
                    prefix_len: 32,
                }),
                fields: vec![],
                custom_pack_unpack: None,
                description: String::new(),
            },
            ABIDeclaration::Alias {
                name: "Incoming".to_owned(),
                ty_idx: incoming_alias_ty_idx,
                target_ty_idx: incoming_union_ty_idx,
                type_params: None,
                custom_pack_unpack: None,
                description: String::new(),
            },
        ];
        abi.incoming_messages = vec![ABIInternalMessage {
            body_ty_idx: incoming_alias_ty_idx,
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
        let storage_ty_idx = struct_ref(&mut abi, "Storage");
        let deployment_ty_idx = struct_ref(&mut abi, "DeploymentStorage");
        abi.declarations = vec![
            ABIDeclaration::Struct {
                name: "Storage".to_owned(),
                ty_idx: storage_ty_idx,
                type_params: None,
                prefix: None,
                fields: vec![],
                custom_pack_unpack: None,
                description: String::new(),
            },
            ABIDeclaration::Struct {
                name: "DeploymentStorage".to_owned(),
                ty_idx: deployment_ty_idx,
                type_params: None,
                prefix: None,
                fields: vec![],
                custom_pack_unpack: None,
                description: String::new(),
            },
        ];
        abi.storage = ABIStorage {
            storage_ty_idx: Some(storage_ty_idx),
            storage_at_deployment_ty_idx: Some(deployment_ty_idx),
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
        let incoming_alias_ty_idx = alias_ref(&mut abi, "Incoming");
        abi.declarations = vec![ABIDeclaration::Alias {
            name: "Incoming".to_owned(),
            ty_idx: incoming_alias_ty_idx,
            target_ty_idx: incoming_alias_ty_idx,
            type_params: None,
            custom_pack_unpack: None,
            description: String::new(),
        }];
        abi.incoming_messages = vec![ABIInternalMessage {
            body_ty_idx: incoming_alias_ty_idx,
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
        let mut abi = empty_abi();
        let int32_ty_idx = add_ty(&mut abi, Ty::IntN { n: 32 });
        let cell_ty_idx = add_ty(
            &mut abi,
            Ty::CellOf {
                inner_ty_idx: int32_ty_idx,
            },
        );
        assert_eq!(abi.render_type(cell_ty_idx), "Cell<int32>");
        assert_eq!(abi.render_param_type(cell_ty_idx), "int32");
        assert!(abi.is_typed_cell(cell_ty_idx));
        assert_eq!(
            abi.typed_cell_payload_default_value(cell_ty_idx).as_deref(),
            Some("0")
        );
        assert_eq!(abi.default_value(cell_ty_idx), "0.toCell()");

        let null_ty_idx = add_ty(&mut abi, Ty::NullLiteral);
        let callable_ty_idx = add_ty(&mut abi, Ty::Callable);
        let nullable_union_ty_idx = add_ty(
            &mut abi,
            Ty::Union {
                variants: vec![
                    UnionVariant {
                        variant_ty_idx: null_ty_idx,
                        prefix_num: 0,
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: None,
                        stack_width: None,
                    },
                    UnionVariant {
                        variant_ty_idx: callable_ty_idx,
                        prefix_num: 0,
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: None,
                        stack_width: None,
                    },
                ],
                stack_width: None,
            },
        );
        assert_eq!(
            abi.render_type(nullable_union_ty_idx),
            "null | continuation"
        );
        let address_any_ty_idx = add_ty(&mut abi, Ty::AddressAny);
        let bits32_ty_idx = add_ty(&mut abi, Ty::BitsN { n: 32 });
        assert_eq!(abi.default_value(address_any_ty_idx), "createAddressNone()");
        assert_eq!(abi.default_value(bits32_ty_idx), "\"\" as bits32");
        assert_eq!(abi.default_value(callable_ty_idx), "fun () {}");
        let int8_ty_idx = add_ty(&mut abi, Ty::IntN { n: 8 });
        let bool_ty_idx = add_ty(&mut abi, Ty::Bool);
        let union_ty_idx = add_ty(
            &mut abi,
            Ty::Union {
                variants: vec![
                    UnionVariant {
                        variant_ty_idx: int8_ty_idx,
                        prefix_num: 0,
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: None,
                        stack_width: None,
                    },
                    UnionVariant {
                        variant_ty_idx: bool_ty_idx,
                        prefix_num: 0,
                        prefix_len: 0,
                        is_prefix_implicit: None,
                        stack_type_id: None,
                        stack_width: None,
                    },
                ],
                stack_width: None,
            },
        );
        assert_eq!(abi.default_value(union_ty_idx), "0");
        let list_ty_idx = add_ty(
            &mut abi,
            Ty::LispListOf {
                inner_ty_idx: bool_ty_idx,
            },
        );
        assert_eq!(abi.default_value(list_ty_idx), "[]");
    }

    #[test]
    fn defaults_resolve_structs_aliases_enums_and_field_defaults() {
        let mut abi = empty_abi();
        let color_ty_idx = enum_ref(&mut abi, "Color");
        let color_encoded_ty_idx = add_ty(&mut abi, Ty::UintN { n: 2 });
        let generic_ty_idx = add_ty(
            &mut abi,
            Ty::GenericT {
                name_t: "T".to_owned(),
            },
        );
        let boxed_generic_ty_idx = add_ty(
            &mut abi,
            Ty::StructRef {
                struct_name: "Boxed".to_owned(),
                type_args_ty_idx: Some(vec![generic_ty_idx]),
            },
        );
        let boxed_plain_ty_idx = struct_ref(&mut abi, "Boxed");
        let user_id_ty_idx = alias_ref(&mut abi, "UserId");
        let int32_ty_idx = add_ty(&mut abi, Ty::IntN { n: 32 });
        let uint32_ty_idx = add_ty(&mut abi, Ty::UintN { n: 32 });
        let bool_ty_idx = add_ty(&mut abi, Ty::Bool);
        let nullable_generic_ty_idx = add_ty(
            &mut abi,
            Ty::Nullable {
                inner_ty_idx: generic_ty_idx,
                stack_type_id: None,
                stack_width: None,
            },
        );
        let nullable_bool_ty_idx = add_ty(
            &mut abi,
            Ty::Nullable {
                inner_ty_idx: bool_ty_idx,
                stack_type_id: None,
                stack_width: None,
            },
        );
        let boxed_nullable_generic_ty_idx = add_ty(
            &mut abi,
            Ty::StructRef {
                struct_name: "Boxed".to_owned(),
                type_args_ty_idx: Some(vec![nullable_generic_ty_idx]),
            },
        );
        let boxed_nullable_bool_ty_idx = add_ty(
            &mut abi,
            Ty::StructRef {
                struct_name: "Boxed".to_owned(),
                type_args_ty_idx: Some(vec![nullable_bool_ty_idx]),
            },
        );
        let maybe_boxed_generic_ty_idx = add_ty(
            &mut abi,
            Ty::AliasRef {
                alias_name: "MaybeBoxed".to_owned(),
                type_args_ty_idx: Some(vec![generic_ty_idx]),
            },
        );
        let maybe_boxed_bool_ty_idx = add_ty(
            &mut abi,
            Ty::AliasRef {
                alias_name: "MaybeBoxed".to_owned(),
                type_args_ty_idx: Some(vec![bool_ty_idx]),
            },
        );
        let storage_ty_idx = struct_ref(&mut abi, "Storage");
        abi.struct_instantiations.push(StructInstantiation {
            ty_idx: boxed_nullable_bool_ty_idx,
            struct_name: "Boxed".to_owned(),
            monomorphic_fields_ty_idx: vec![nullable_bool_ty_idx],
        });
        abi.alias_instantiations.push(AliasInstantiation {
            ty_idx: maybe_boxed_bool_ty_idx,
            alias_name: "MaybeBoxed".to_owned(),
            monomorphic_target_ty_idx: boxed_nullable_bool_ty_idx,
        });
        abi.declarations = vec![
            ABIDeclaration::Enum {
                name: "Color".to_owned(),
                ty_idx: color_ty_idx,
                encoded_as_ty_idx: color_encoded_ty_idx,
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
                description: String::new(),
            },
            ABIDeclaration::Struct {
                name: "Boxed".to_owned(),
                ty_idx: boxed_generic_ty_idx,
                type_params: Some(vec!["T".to_owned()]),
                prefix: None,
                fields: vec![ABIStructField {
                    name: "item".to_owned(),
                    ty_idx: generic_ty_idx,
                    client_ty_idx: None,
                    default_value: None,
                    description: String::new(),
                }],
                custom_pack_unpack: None,
                description: String::new(),
            },
            ABIDeclaration::Alias {
                name: "UserId".to_owned(),
                ty_idx: user_id_ty_idx,
                target_ty_idx: int32_ty_idx,
                type_params: None,
                custom_pack_unpack: None,
                description: String::new(),
            },
            ABIDeclaration::Alias {
                name: "MaybeBoxed".to_owned(),
                ty_idx: maybe_boxed_generic_ty_idx,
                target_ty_idx: boxed_nullable_generic_ty_idx,
                type_params: Some(vec!["T".to_owned()]),
                custom_pack_unpack: None,
                description: String::new(),
            },
            ABIDeclaration::Struct {
                name: "Storage".to_owned(),
                ty_idx: storage_ty_idx,
                type_params: None,
                prefix: None,
                fields: vec![
                    ABIStructField {
                        name: "owner".to_owned(),
                        ty_idx: user_id_ty_idx,
                        client_ty_idx: None,
                        default_value: None,
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "color".to_owned(),
                        ty_idx: color_ty_idx,
                        client_ty_idx: None,
                        default_value: None,
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "maybeItem".to_owned(),
                        ty_idx: maybe_boxed_bool_ty_idx,
                        client_ty_idx: None,
                        default_value: None,
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "nested".to_owned(),
                        ty_idx: boxed_plain_ty_idx,
                        client_ty_idx: None,
                        default_value: Some(ABIConstValue::Object {
                            struct_name: "Boxed".to_owned(),
                            fields: vec![ABIConstValue::Null],
                        }),
                        description: String::new(),
                    },
                    ABIStructField {
                        name: "opcode".to_owned(),
                        ty_idx: uint32_ty_idx,
                        client_ty_idx: None,
                        default_value: Some(ABIConstValue::CastTo {
                            inner: Box::new(ABIConstValue::Int { v: "7".to_owned() }),
                            cast_to_ty_idx: uint32_ty_idx,
                        }),
                        description: String::new(),
                    },
                ],
                custom_pack_unpack: None,
                description: String::new(),
            },
        ];

        let storage_default = abi.default_value(storage_ty_idx);

        assert_eq!(
            storage_default,
            "Storage { owner: (0 as UserId), color: Color.Red, maybeItem: (Boxed<bool?> { item: null } as MaybeBoxed<bool>), nested: Boxed { item: null }, opcode: (7 as uint32) }"
        );
    }

    #[test]
    fn default_value_falls_back_to_null_for_recursive_aliases() {
        let mut abi = empty_abi();
        let loop_ty_idx = alias_ref(&mut abi, "Loop");
        abi.declarations = vec![ABIDeclaration::Alias {
            name: "Loop".to_owned(),
            ty_idx: loop_ty_idx,
            target_ty_idx: loop_ty_idx,
            type_params: None,
            custom_pack_unpack: None,
            description: String::new(),
        }];

        let value = abi.default_value(loop_ty_idx);

        assert_eq!(value, "null");
    }
}
