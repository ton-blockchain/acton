use std::collections::{HashMap, HashSet};

use num_bigint::{BigInt, Sign};
use tolk_resolver::{SymbolId, SymbolKind};
use tolk_syntax::{AstNode, TopLevel};
use tolk_ty::{AddressKind, IntTy, TyData, TyId, TypeInterner};

use crate::constant_evaluator::{ConstantEvaluationContext, ConstantEvaluator, ConstantValue};

const UNBOUNDED_BITS: u32 = 9_999;

/// Supplies the semantic data needed to estimate a Tolk value's serialized size.
pub trait SerializationSizeContext: ConstantEvaluationContext {
    fn type_interner(&self) -> &TypeInterner;

    fn type_of_symbol(&self, symbol_id: SymbolId) -> Option<TyId>;

    fn method_receiver_type(&self, symbol_id: SymbolId) -> Option<TyId>;
}

/// Minimum and maximum number of bits and references used by a serialized value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SerializationSize {
    pub valid: bool,
    pub min_bits: u32,
    pub max_bits: u32,
    pub min_refs: u32,
    pub max_refs: u32,
}

impl SerializationSize {
    #[must_use]
    pub const fn exact(bits: u32) -> Self {
        Self::range(bits, bits, 0, 0)
    }

    #[must_use]
    pub const fn range(min_bits: u32, max_bits: u32, min_refs: u32, max_refs: u32) -> Self {
        Self {
            valid: true,
            min_bits,
            max_bits,
            min_refs,
            max_refs,
        }
    }

    #[must_use]
    pub const fn invalid() -> Self {
        Self {
            valid: false,
            min_bits: 0,
            max_bits: 0,
            min_refs: 0,
            max_refs: 0,
        }
    }

    #[must_use]
    pub const fn unpredictable() -> Self {
        Self::range(0, UNBOUNDED_BITS, 0, 4)
    }

    #[must_use]
    pub fn presentation(self) -> String {
        if !self.valid {
            return "unknown or invalid size".to_owned();
        }

        if self.min_bits == self.max_bits && self.min_refs == self.max_refs {
            if self.min_refs == 0 {
                return format!("{} bits", self.min_bits);
            }

            return format!("{} bits, {} refs", self.min_bits, self.min_refs);
        }

        format!(
            "{}, {}",
            format_range(self.min_bits, self.max_bits, "bit"),
            format_range(self.min_refs, self.max_refs, "ref"),
        )
    }

    fn sum(self, other: Self) -> Self {
        if !self.valid || !other.valid {
            return Self::invalid();
        }

        Self::range(
            self.min_bits.saturating_add(other.min_bits),
            UNBOUNDED_BITS.min(self.max_bits.saturating_add(other.max_bits)),
            self.min_refs.saturating_add(other.min_refs),
            self.max_refs.saturating_add(other.max_refs),
        )
    }

    fn minmax(self, other: Self) -> Self {
        if !self.valid || !other.valid {
            return Self::invalid();
        }

        Self::range(
            self.min_bits.min(other.min_bits),
            self.max_bits.max(other.max_bits),
            self.min_refs.min(other.min_refs),
            self.max_refs.max(other.max_refs),
        )
    }
}

/// Estimates the serialized size of a Tolk type using resolver and inference metadata.
#[must_use]
pub fn estimate_serialization_size(
    context: &dyn SerializationSizeContext,
    ty: TyId,
) -> SerializationSize {
    Estimator {
        context,
        visiting: HashMap::new(),
        cell_ref_depth: 0,
    }
    .estimate(ty, true, &HashMap::new())
}

struct Estimator<'a> {
    context: &'a dyn SerializationSizeContext,
    visiting: HashMap<TyId, u32>,
    cell_ref_depth: u32,
}

impl Estimator<'_> {
    fn estimate(
        &mut self,
        ty: TyId,
        include_struct_prefix: bool,
        substitutions: &HashMap<String, TyId>,
    ) -> SerializationSize {
        if let Some(previous_depth) = self.visiting.get(&ty) {
            return if self.cell_ref_depth > *previous_depth {
                SerializationSize::exact(0)
            } else {
                SerializationSize::invalid()
            };
        }
        self.visiting.insert(ty, self.cell_ref_depth);

        let substitution = match self.context.type_interner().data(ty) {
            TyData::TypeParameter { name, .. } => substitutions.get(name).copied(),
            _ => None,
        };
        let mut size = if let Some(substitution) = substitution {
            self.estimate(substitution, include_struct_prefix, substitutions)
        } else if self.has_custom_serializer(ty) {
            SerializationSize::unpredictable()
        } else {
            self.calculate(ty, include_struct_prefix, substitutions)
        };
        self.visiting.remove(&ty);
        if size.valid && size.max_bits >= UNBOUNDED_BITS {
            size.max_bits = UNBOUNDED_BITS.max(size.min_bits);
        }
        size
    }

    fn calculate(
        &mut self,
        ty: TyId,
        include_struct_prefix: bool,
        substitutions: &HashMap<String, TyId>,
    ) -> SerializationSize {
        match self.context.type_interner().data(ty).clone() {
            TyData::Struct {
                def, base, args, ..
            } => {
                let def = base.unwrap_or(def);
                if self.is_typed_cell(def) {
                    let Some(inner) = args.as_deref().and_then(|args| args.first()).copied() else {
                        return SerializationSize::invalid();
                    };
                    let inner = self.resolve_substitution(inner, substitutions);
                    let previous_depth = self.cell_ref_depth;
                    self.cell_ref_depth = self.cell_ref_depth.saturating_add(1);
                    let inner_is_valid = self.estimate(inner, true, substitutions).valid;
                    self.cell_ref_depth = previous_depth;
                    if inner_is_valid {
                        SerializationSize::range(0, 0, 1, 1)
                    } else {
                        SerializationSize::invalid()
                    }
                } else {
                    self.struct_size(def, args.as_deref(), include_struct_prefix, substitutions)
                }
            }
            TyData::Enum { def, .. } => self.enum_size(def),
            TyData::TypeAlias {
                def,
                inner_ty,
                args,
                ..
            } => {
                let substitutions = self.substitutions(def, args.as_deref(), substitutions);
                self.estimate(inner_ty, include_struct_prefix, &substitutions)
            }
            TyData::Tensor(elements) | TyData::Tuple(elements) => elements
                .into_iter()
                .fold(SerializationSize::exact(0), |size, element| {
                    size.sum(self.estimate(element, true, substitutions))
                }),
            TyData::Array(element) => {
                let element_size = self.estimate(element, true, substitutions);
                if !element_size.valid
                    || (element_size.max_bits == 0 && element_size.max_refs == 0)
                    || ((element_size.max_refs >= 4 || element_size.min_bits >= 1_022)
                        && element_size.max_bits < UNBOUNDED_BITS)
                {
                    SerializationSize::invalid()
                } else {
                    SerializationSize::range(9, 9, 0, 1)
                }
            }
            TyData::TypeParameter { .. } | TyData::Slice | TyData::Builder => {
                SerializationSize::unpredictable()
            }
            TyData::Union(elements) => self.union_size(&elements, substitutions),
            TyData::GenericTypeWithTs { inner_ty, types } => {
                let def = type_definition(self.context.type_interner(), inner_ty);
                let substitutions = def.map_or_else(
                    || substitutions.clone(),
                    |def| self.substitutions(def, Some(&types), substitutions),
                );
                self.estimate(inner_ty, include_struct_prefix, &substitutions)
            }
            TyData::Builtin { name } => match name.as_ref() {
                "address" => SerializationSize::exact(267),
                "any_address" => SerializationSize::range(2, 522, 0, 0),
                "cell" | "string" => SerializationSize::range(0, 0, 1, 1),
                "builder" | "slice" => SerializationSize::unpredictable(),
                _ => SerializationSize::invalid(),
            },
            TyData::Int(IntTy::IntN { size, .. }) | TyData::Bits { size } => {
                SerializationSize::exact(size as u32)
            }
            TyData::Int(IntTy::VarIntN { size: 32, .. }) => SerializationSize::range(5, 253, 0, 0),
            TyData::Int(IntTy::VarIntN { .. } | IntTy::Coins) => {
                SerializationSize::range(4, 124, 0, 0)
            }
            TyData::Bool { .. } => SerializationSize::exact(1),
            TyData::Cell => SerializationSize::range(0, 0, 1, 1),
            TyData::Address(AddressKind::Internal) => SerializationSize::exact(267),
            TyData::Address(AddressKind::Any) => SerializationSize::range(2, 522, 0, 0),
            TyData::MapKV { .. } => SerializationSize::range(1, 1, 0, 1),
            TyData::Bytes { size } => SerializationSize::exact((size * 8) as u32),
            TyData::Void => SerializationSize::exact(0),
            TyData::Int(IntTy::Int)
            | TyData::UntypedTuple
            | TyData::Func { .. }
            | TyData::Continuation
            | TyData::Null
            | TyData::Never
            | TyData::Auto
            | TyData::Undefined
            | TyData::Unknown => SerializationSize::invalid(),
        }
    }

    fn struct_size(
        &mut self,
        def: SymbolId,
        args: Option<&[TyId]>,
        include_prefix: bool,
        inherited: &HashMap<String, TyId>,
    ) -> SerializationSize {
        let Some(symbol) = self.context.project_index().resolve_symbol(def) else {
            return SerializationSize::invalid();
        };
        let SymbolKind::Struct { fields, .. } = &symbol.kind else {
            return SerializationSize::invalid();
        };

        let substitutions = self.substitutions(def, args, inherited);
        let mut size = if include_prefix {
            self.struct_prefix_size(def)
                .map_or_else(|| SerializationSize::exact(0), SerializationSize::exact)
        } else {
            SerializationSize::exact(0)
        };

        for field in fields {
            let Some(field_ty) = self.context.type_of_symbol(field.id) else {
                return SerializationSize::invalid();
            };
            size = size.sum(self.estimate(field_ty, true, &substitutions));
        }

        size
    }

    fn enum_size(&self, def: SymbolId) -> SerializationSize {
        let Some(file) = self.context.file_db().get_by_id(def.file_id) else {
            return SerializationSize::invalid();
        };
        let Some(TopLevel::Enum(enumeration)) = file.find_syntax_declaration(def) else {
            return SerializationSize::invalid();
        };
        if let Some(backed_type) = enumeration.backed_type() {
            return primitive_size(backed_type.text(file.source().source.as_ref()));
        }

        let mut evaluator = ConstantEvaluator::new(self.context);
        let Some(values) = evaluator.evaluate_enum_values(def) else {
            return SerializationSize::invalid();
        };
        let values = values
            .values()
            .map(|value| match value {
                ConstantValue::Int(value) => Some(value),
                ConstantValue::Bool(_)
                | ConstantValue::String(_)
                | ConstantValue::Overflow
                | ConstantValue::Unknown => None,
            })
            .collect::<Option<Vec<_>>>();
        let Some(values) = values.filter(|values| !values.is_empty()) else {
            return SerializationSize::invalid();
        };

        for bits in 1..=256 {
            if values.iter().all(|value| fits_unsigned(value, bits))
                || values.iter().all(|value| fits_signed(value, bits))
            {
                return SerializationSize::exact(bits);
            }
        }

        SerializationSize::invalid()
    }

    fn union_size(
        &mut self,
        elements: &[TyId],
        substitutions: &HashMap<String, TyId>,
    ) -> SerializationSize {
        if elements.is_empty() {
            return SerializationSize::invalid();
        }

        let null = elements.iter().position(|ty| self.is_null(*ty));
        if elements.len() == 2
            && let Some(null_index) = null
        {
            let value = elements[usize::from(null_index == 0)];
            if self.is_void(value) {
                return SerializationSize::invalid();
            }
            let value_size = self.estimate(value, true, substitutions);
            if !value_size.valid {
                return SerializationSize::invalid();
            }
            if self.is_internal_address(value) && !self.has_custom_serializer(value) {
                return SerializationSize::range(2, 267, 0, 0);
            }
            return SerializationSize::range(
                1,
                value_size.max_bits.saturating_add(1),
                0,
                value_size.max_refs,
            );
        }

        let void = elements.iter().position(|ty| self.is_void(*ty));
        if void.is_some_and(|index| index + 1 != elements.len()) {
            return SerializationSize::invalid();
        }

        let prefixes = elements
            .iter()
            .map(|ty| self.type_prefix(*ty))
            .collect::<Vec<_>>();
        let non_void = elements.len() - usize::from(void.is_some());
        let prefixed = elements
            .iter()
            .zip(&prefixes)
            .filter(|(ty, prefix)| !self.is_void(**ty) && prefix.is_some())
            .count();

        if prefixed > 0 && prefixed != non_void {
            return SerializationSize::invalid();
        }

        if prefixed == non_void && prefixed > 0 {
            let mut seen = HashSet::new();
            let mut result = SerializationSize::invalid();
            for (ty, prefix) in elements.iter().zip(&prefixes) {
                let variant = if self.is_void(*ty) {
                    SerializationSize::exact(0)
                } else {
                    let Some(prefix) = prefix else {
                        return SerializationSize::invalid();
                    };
                    if !seen.insert(prefix.clone()) {
                        return SerializationSize::invalid();
                    }
                    SerializationSize::exact(prefix.len() as u32).sum(self.estimate(
                        *ty,
                        false,
                        substitutions,
                    ))
                };
                if !variant.valid {
                    return SerializationSize::invalid();
                }
                result = if result.valid {
                    result.minmax(variant)
                } else {
                    variant
                };
            }
            return result;
        }

        let tagged_variants =
            elements.len() - usize::from(null.is_some()) - usize::from(void.is_some());
        let tree_bits = if tagged_variants <= 1 {
            0
        } else {
            usize::BITS - (tagged_variants - 1).leading_zeros()
        };
        let mut result = SerializationSize::invalid();

        for ty in elements {
            let (variant, prefix) = if self.is_void(*ty) {
                (SerializationSize::exact(0), 0)
            } else if self.is_null(*ty) {
                (SerializationSize::exact(0), 1)
            } else {
                (
                    self.estimate(*ty, true, substitutions),
                    tree_bits + u32::from(null.is_some()),
                )
            };
            if !variant.valid {
                return SerializationSize::invalid();
            }
            if void.is_some()
                && !self.is_void(*ty)
                && prefix == 0
                && variant.min_bits == 0
                && variant.min_refs == 0
            {
                return SerializationSize::invalid();
            }
            let prefix = if self.is_null(*ty) { 1 } else { prefix };
            let variant = SerializationSize::exact(prefix).sum(variant);
            result = if result.valid {
                result.minmax(variant)
            } else {
                variant
            };
        }

        result
    }

    fn has_custom_serializer(&self, ty: TyId) -> bool {
        let Some(def) = type_definition(self.context.type_interner(), ty) else {
            return false;
        };
        self.context
            .project_index()
            .methods_by_name()
            .get("packToBuilder")
            .into_iter()
            .flatten()
            .any(|method| {
                self.context
                    .method_receiver_type(*method)
                    .and_then(|receiver| type_definition(self.context.type_interner(), receiver))
                    == Some(def)
            })
    }

    fn is_typed_cell(&self, def: SymbolId) -> bool {
        self.context
            .project_index()
            .resolve_symbol(def)
            .is_some_and(|symbol| {
                symbol.name.as_ref() == "Cell"
                    && matches!(
                        &symbol.kind,
                        SymbolKind::Struct {
                            fields,
                            type_parameters,
                            ..
                        } if fields.len() == 1
                            && fields[0].name.as_ref() == "tvmCell"
                            && type_parameters.len() == 1
                    )
            })
    }

    fn resolve_substitution(&self, ty: TyId, substitutions: &HashMap<String, TyId>) -> TyId {
        match self.context.type_interner().data(ty) {
            TyData::TypeParameter { name, .. } => substitutions.get(name).copied().unwrap_or(ty),
            _ => ty,
        }
    }

    fn is_null(&self, ty: TyId) -> bool {
        matches!(
            self.context
                .type_interner()
                .data(self.context.type_interner().unwrap_alias(ty)),
            TyData::Null
        )
    }

    fn is_void(&self, ty: TyId) -> bool {
        matches!(
            self.context
                .type_interner()
                .data(self.context.type_interner().unwrap_alias(ty)),
            TyData::Void
        )
    }

    fn is_internal_address(&self, ty: TyId) -> bool {
        match self
            .context
            .type_interner()
            .data(self.context.type_interner().unwrap_alias(ty))
        {
            TyData::Address(AddressKind::Internal) => true,
            TyData::Builtin { name } => name.as_ref() == "address",
            _ => false,
        }
    }

    fn substitutions(
        &self,
        def: SymbolId,
        args: Option<&[TyId]>,
        inherited: &HashMap<String, TyId>,
    ) -> HashMap<String, TyId> {
        let mut result = inherited.clone();
        let Some(args) = args else {
            return result;
        };
        let Some(symbol) = self.context.project_index().resolve_symbol(def) else {
            return result;
        };
        let (SymbolKind::Struct {
            type_parameters: parameters,
            ..
        }
        | SymbolKind::TypeAlias {
            type_parameters: parameters,
            ..
        }) = &symbol.kind
        else {
            return result;
        };

        for (parameter, ty) in parameters.iter().zip(args) {
            let ty = self.resolve_substitution(*ty, inherited);
            if matches!(
                self.context.type_interner().data(ty),
                TyData::TypeParameter { name, .. } if name == parameter.name.as_ref()
            ) {
                continue;
            }
            result.insert(parameter.name.to_string(), ty);
        }
        result
    }

    fn type_prefix(&self, ty: TyId) -> Option<String> {
        let ty = self.context.type_interner().unwrap_alias(ty);
        let TyData::Struct { def, base, .. } = self.context.type_interner().data(ty) else {
            return None;
        };
        self.struct_prefix(base.unwrap_or(*def))
    }

    fn struct_prefix_size(&self, def: SymbolId) -> Option<u32> {
        u32::try_from(self.struct_prefix(def)?.len()).ok()
    }

    fn struct_prefix(&self, def: SymbolId) -> Option<String> {
        let file = self.context.file_db().get_by_id(def.file_id)?;
        let TopLevel::Struct(structure) = file.find_syntax_declaration(def)? else {
            return None;
        };
        let prefix = structure.pack_prefix()?;
        prefix_bits(prefix.text(file.source().source.as_ref()))
    }
}

fn type_definition(interner: &TypeInterner, ty: TyId) -> Option<SymbolId> {
    match interner.data(ty) {
        TyData::Struct { def, base, .. } => Some(base.unwrap_or(*def)),
        TyData::TypeAlias { def, .. } | TyData::Enum { def, .. } => Some(*def),
        TyData::GenericTypeWithTs { inner_ty, .. } => type_definition(interner, *inner_ty),
        _ => None,
    }
}

fn prefix_bits(text: &str) -> Option<String> {
    let value = text.replace('_', "");
    if let Some(hex) = value.strip_prefix("0x") {
        let mut result = String::with_capacity(hex.len().checked_mul(4)?);
        for digit in hex.chars() {
            let digit = digit.to_digit(16)?;
            for shift in (0..4).rev() {
                result.push(if digit & (1 << shift) == 0 { '0' } else { '1' });
            }
        }
        return Some(result);
    }
    if let Some(binary) = value.strip_prefix("0b") {
        binary
            .chars()
            .all(|digit| matches!(digit, '0' | '1'))
            .then(|| binary.to_owned())
    } else {
        let value = value.parse::<u128>().ok()?;
        Some(format!("{value:b}"))
    }
}

fn primitive_size(text: &str) -> SerializationSize {
    let text = text.trim();
    if text == "coins" {
        return SerializationSize::range(4, 124, 0, 0);
    }
    let (prefix, text) = if let Some(text) = text.strip_prefix("varuint") {
        ("varuint", text)
    } else if let Some(text) = text.strip_prefix("varint") {
        ("varint", text)
    } else if let Some(text) = text.strip_prefix("uint") {
        ("uint", text)
    } else if let Some(text) = text.strip_prefix("int") {
        ("int", text)
    } else {
        return SerializationSize::invalid();
    };
    let Ok(bits) = text.parse::<u32>() else {
        return SerializationSize::invalid();
    };
    match prefix {
        "uint" | "int" => SerializationSize::exact(bits),
        "varuint" | "varint" if bits == 32 => SerializationSize::range(5, 253, 0, 0),
        "varuint" | "varint" => SerializationSize::range(4, 124, 0, 0),
        _ => SerializationSize::invalid(),
    }
}

fn fits_unsigned(value: &BigInt, bits: u32) -> bool {
    value.sign() != Sign::Minus && value.bits() <= u64::from(bits)
}

fn fits_signed(value: &BigInt, bits: u32) -> bool {
    let boundary = BigInt::from(1_u8) << (bits - 1);
    value >= &-boundary.clone() && value < &boundary
}

fn format_range(first: u32, second: u32, unit: &str) -> String {
    if first == second {
        let suffix = if first == 1 { "" } else { "s" };
        return format!("{first} {unit}{suffix}");
    }

    format!("{first}..{second} {unit}s")
}
