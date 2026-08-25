use crate::{CodegenError, generate_error};
use std::collections::HashSet;
use tolk_source_map::abi::{ABIDeclaration, ABIStructField, ContractABI};
use tolk_source_map::types_kernel::{Ty, TyIdx, UnionVariant, render_ty};

#[derive(Debug, Clone)]
pub(crate) struct ResolvedField {
    pub field: ABIStructField,
    pub u_label_ty_idx: Option<TyIdx>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AliasTarget {
    pub ty_idx: TyIdx,
    pub u_label_ty_idx: Option<TyIdx>,
}

#[derive(Debug, Clone)]
pub(crate) struct LabeledUnionVariant {
    pub variant_ty_idx: TyIdx,
    pub label: String,
    pub prefix_num: u64,
    pub prefix_len: usize,
    pub is_prefix_implicit: bool,
    pub stack_type_id: Option<usize>,
    pub stack_width: Option<usize>,
}

pub(crate) struct Symbols<'abi> {
    abi: &'abi ContractABI,
}

impl<'abi> Symbols<'abi> {
    pub(crate) const fn new(abi: &'abi ContractABI) -> Self {
        Self { abi }
    }

    pub(crate) fn ty(&self, ty_idx: TyIdx) -> Result<&'abi Ty, CodegenError> {
        self.abi
            .ty_by_idx(ty_idx)
            .ok_or_else(|| generate_error(format!("ABI references unknown type index {ty_idx}")))
    }

    pub(crate) fn declaration(&self, name: &str) -> Result<&'abi ABIDeclaration, CodegenError> {
        self.abi
            .declarations
            .iter()
            .find(|declaration| declaration.name() == name)
            .ok_or_else(|| generate_error(format!("ABI declaration `{name}` was not found")))
    }

    pub(crate) fn struct_fields(
        &self,
        ty_idx: TyIdx,
        is_for_stack: bool,
    ) -> Result<Vec<ResolvedField>, CodegenError> {
        let Ty::StructRef { struct_name, .. } = self.ty(ty_idx)? else {
            return Err(generate_error(format!(
                "expected StructRef at type index {ty_idx}"
            )));
        };
        let ABIDeclaration::Struct { fields, .. } = self.declaration(struct_name)? else {
            return Err(generate_error(format!(
                "declaration `{struct_name}` is not a struct"
            )));
        };

        let monomorphic = self
            .abi
            .struct_instantiations
            .iter()
            .find(|instantiation| instantiation.ty_idx == ty_idx);
        if let Some(monomorphic) = monomorphic
            && monomorphic.monomorphic_fields_ty_idx.len() != fields.len()
        {
            return Err(generate_error(format!(
                "struct instantiation `{struct_name}` has {} fields, expected {}",
                monomorphic.monomorphic_fields_ty_idx.len(),
                fields.len()
            )));
        }

        fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                let mut resolved = field.clone();
                let u_label_ty_idx =
                    if !is_for_stack && let Some(client_ty_idx) = field.client_ty_idx {
                        resolved.ty_idx = client_ty_idx;
                        None
                    } else if let Some(monomorphic) = monomorphic {
                        resolved.ty_idx = monomorphic.monomorphic_fields_ty_idx[index];
                        Some(field.ty_idx)
                    } else {
                        None
                    };
                Ok(ResolvedField {
                    field: resolved,
                    u_label_ty_idx,
                })
            })
            .collect()
    }

    pub(crate) fn alias_target(&self, ty_idx: TyIdx) -> Result<AliasTarget, CodegenError> {
        let Ty::AliasRef { alias_name, .. } = self.ty(ty_idx)? else {
            return Err(generate_error(format!(
                "expected AliasRef at type index {ty_idx}"
            )));
        };
        let ABIDeclaration::Alias { target_ty_idx, .. } = self.declaration(alias_name)? else {
            return Err(generate_error(format!(
                "declaration `{alias_name}` is not an alias"
            )));
        };
        if let Some(instantiation) = self
            .abi
            .alias_instantiations
            .iter()
            .find(|instantiation| instantiation.ty_idx == ty_idx)
        {
            return Ok(AliasTarget {
                ty_idx: instantiation.monomorphic_target_ty_idx,
                u_label_ty_idx: Some(*target_ty_idx),
            });
        }
        Ok(AliasTarget {
            ty_idx: *target_ty_idx,
            u_label_ty_idx: None,
        })
    }

    pub(crate) fn union_variants(
        &self,
        variants: &[UnionVariant],
        u_label_ty_idx: Option<TyIdx>,
    ) -> Result<Vec<LabeledUnionVariant>, CodegenError> {
        let generic_variants = u_label_ty_idx.and_then(|ty_idx| match self.ty(ty_idx).ok()? {
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

        let simple_labels = variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                self.simple_union_label(
                    generic_variants
                        .as_ref()
                        .map_or(variant.variant_ty_idx, |variants| variants[index]),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut unique = HashSet::new();
        let has_duplicates = simple_labels
            .iter()
            .any(|label| !unique.insert(label.clone()));

        variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                let label_ty_idx = generic_variants
                    .as_ref()
                    .map_or(variant.variant_ty_idx, |variants| variants[index]);
                let is_null = matches!(self.ty(label_ty_idx)?, Ty::NullLiteral);
                Ok(LabeledUnionVariant {
                    variant_ty_idx: variant.variant_ty_idx,
                    label: if is_null {
                        String::new()
                    } else if has_duplicates {
                        render_ty(self.abi, label_ty_idx)
                    } else {
                        simple_labels[index].clone()
                    },
                    prefix_num: variant.prefix_num,
                    prefix_len: variant.prefix_len,
                    is_prefix_implicit: variant.is_prefix_implicit.unwrap_or(false),
                    stack_type_id: variant.stack_type_id,
                    stack_width: variant.stack_width,
                })
            })
            .collect()
    }

    pub(crate) fn stack_width(&self, ty_idx: TyIdx) -> Result<usize, CodegenError> {
        match self.ty(ty_idx)? {
            Ty::Void => Ok(0),
            Ty::Tensor { items_ty_idx } => items_ty_idx
                .iter()
                .map(|ty_idx| self.stack_width(*ty_idx))
                .sum(),
            Ty::StructRef { .. } => self
                .struct_fields(ty_idx, true)?
                .iter()
                .map(|field| self.stack_width(field.field.ty_idx))
                .sum(),
            Ty::AliasRef { .. } => self.stack_width(self.alias_target(ty_idx)?.ty_idx),
            Ty::Nullable { stack_width, .. } => Ok(stack_width.unwrap_or(1)),
            Ty::Union { stack_width, .. } => stack_width.ok_or_else(|| {
                generate_error(format!(
                    "union at type index {ty_idx} has no concrete stack_width"
                ))
            }),
            Ty::GenericT { name_t } => Err(generate_error(format!(
                "unexpected unresolved generic `{name_t}` while calculating stack width"
            ))),
            _ => Ok(1),
        }
    }

    pub(crate) fn union_identity(&self, ty_idx: TyIdx, u_label_ty_idx: Option<TyIdx>) -> TyIdx {
        u_label_ty_idx
            .filter(|label_ty_idx| matches!(self.ty(*label_ty_idx), Ok(Ty::Union { .. })))
            .unwrap_or(ty_idx)
    }

    fn simple_union_label(&self, ty_idx: TyIdx) -> Result<String, CodegenError> {
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
            Ty::AliasRef { .. } => {
                return self.simple_union_label(self.alias_target(ty_idx)?.ty_idx);
            }
            Ty::GenericT { name_t } => name_t.clone(),
            Ty::Union { variants, .. } => variants
                .iter()
                .map(|variant| self.simple_union_label(variant.variant_ty_idx))
                .collect::<Result<Vec<_>, _>>()?
                .join("|"),
        })
    }
}

trait DeclarationName {
    fn name(&self) -> &str;
}

impl DeclarationName for ABIDeclaration {
    fn name(&self) -> &str {
        match self {
            Self::Struct { name, .. } | Self::Alias { name, .. } | Self::Enum { name, .. } => name,
        }
    }
}
