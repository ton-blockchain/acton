use std::collections::{HashMap, HashSet};

use tolk_resolver::{FileDb, ProjectIndex, SymbolId, SymbolKind};
use tolk_syntax::{AstNode, TopLevel};
use tolk_ty::{IntTy, TyData, TyId, TypeInterner};

const UNBOUNDED_BITS: u32 = 9_999;

/// Supplies the semantic data needed to estimate a Tolk value's serialized size.
pub trait SerializationSizeContext {
    fn file_db(&self) -> &FileDb;

    fn project_index(&self) -> &ProjectIndex;

    fn type_interner(&self) -> &TypeInterner;

    fn type_of_symbol(&self, symbol_id: SymbolId) -> Option<TyId>;
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
        visiting: HashSet::new(),
    }
    .estimate(ty, true, &HashMap::new())
}

struct Estimator<'a> {
    context: &'a dyn SerializationSizeContext,
    visiting: HashSet<TyId>,
}

impl Estimator<'_> {
    fn estimate(
        &mut self,
        ty: TyId,
        include_struct_prefix: bool,
        substitutions: &HashMap<String, TyId>,
    ) -> SerializationSize {
        if !self.visiting.insert(ty) {
            return SerializationSize::unpredictable();
        }

        let substitution = match self.context.type_interner().data(ty) {
            TyData::TypeParameter { name, .. } => substitutions.get(name).copied(),
            _ => None,
        };
        let size = if let Some(substitution) = substitution {
            self.estimate(substitution, include_struct_prefix, substitutions)
        } else {
            self.calculate(ty, include_struct_prefix, substitutions)
        };
        self.visiting.remove(&ty);
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
            } => self.struct_size(base.unwrap_or(def), args.as_deref(), include_struct_prefix),
            // Enum serialization depends on its declaration and is not represented in TyData yet.
            TyData::Enum { .. } => SerializationSize::invalid(),
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
            TyData::Array(_)
            | TyData::UntypedTuple
            | TyData::Func { .. }
            | TyData::TypeParameter { .. }
            | TyData::Slice
            | TyData::Builder
            | TyData::Continuation => SerializationSize::unpredictable(),
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
                "address" | "any_address" => SerializationSize::range(2, 267, 0, 0),
                "cell" => SerializationSize::range(0, 0, 1, 1),
                "builder" | "slice" => SerializationSize::unpredictable(),
                _ => SerializationSize::invalid(),
            },
            TyData::Int(IntTy::Int) => SerializationSize::exact(257),
            TyData::Int(IntTy::IntN { size, .. }) | TyData::Bits { size } => {
                SerializationSize::exact(size as u32)
            }
            TyData::Int(IntTy::VarIntN { size: 32, .. }) => SerializationSize::range(5, 253, 0, 0),
            TyData::Int(IntTy::VarIntN { .. } | IntTy::Coins) => {
                SerializationSize::range(4, 124, 0, 0)
            }
            TyData::Bool { .. } => SerializationSize::exact(1),
            TyData::Cell => SerializationSize::range(0, 0, 1, 1),
            TyData::Address(_) => SerializationSize::range(2, 267, 0, 0),
            TyData::MapKV { .. } => SerializationSize::range(0, 1, 0, 1),
            TyData::Bytes { size } => SerializationSize::exact((size * 8) as u32),
            TyData::Null | TyData::Never => SerializationSize::exact(0),
            TyData::Void | TyData::Auto | TyData::Undefined | TyData::Unknown => {
                SerializationSize::invalid()
            }
        }
    }

    fn struct_size(
        &mut self,
        def: SymbolId,
        args: Option<&[TyId]>,
        include_prefix: bool,
    ) -> SerializationSize {
        let Some(symbol) = self.context.project_index().resolve_symbol(def) else {
            return SerializationSize::invalid();
        };
        let SymbolKind::Struct { fields, .. } = &symbol.kind else {
            return SerializationSize::invalid();
        };

        let substitutions = self.substitutions(def, args, &HashMap::new());
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

    fn union_size(
        &mut self,
        elements: &[TyId],
        substitutions: &HashMap<String, TyId>,
    ) -> SerializationSize {
        if elements.is_empty() {
            return SerializationSize::invalid();
        }

        let null = elements
            .iter()
            .position(|ty| matches!(self.context.type_interner().data(*ty), TyData::Null));
        if elements.len() == 2
            && let Some(null_index) = null
        {
            let value = elements[usize::from(null_index == 0)];
            let value_size = self.estimate(value, true, substitutions);
            return SerializationSize::exact(1).sum(SerializationSize::range(
                value_size.min_bits,
                value_size.max_bits,
                0,
                value_size.max_refs,
            ));
        }

        let prefix_sizes = elements
            .iter()
            .map(|ty| self.type_prefix_size(*ty))
            .collect::<Vec<_>>();
        let prefixed = prefix_sizes
            .iter()
            .filter(|prefix| prefix.is_some())
            .count();

        if prefixed > 0 && prefixed != elements.len() {
            return SerializationSize::invalid();
        }

        if prefixed == elements.len() {
            let mut variants = self.estimate(elements[0], false, substitutions);
            let mut prefixes = SerializationSize::exact(prefix_sizes[0].unwrap_or(0));

            for (ty, prefix) in elements.iter().zip(prefix_sizes.iter()).skip(1) {
                variants = variants.minmax(self.estimate(*ty, false, substitutions));
                prefixes = prefixes.minmax(SerializationSize::exact(prefix.unwrap_or(0)));
            }

            return prefixes.sum(variants);
        }

        let without_null = elements.len() - usize::from(null.is_some());
        let tree_bits = if without_null <= 1 {
            0
        } else {
            usize::BITS - (without_null - 1).leading_zeros()
        };
        let mut result = SerializationSize::invalid();

        for ty in elements {
            let variant = self.estimate(*ty, true, substitutions);
            let prefix = if matches!(self.context.type_interner().data(*ty), TyData::Null) {
                1
            } else {
                tree_bits + u32::from(null.is_some())
            };
            let variant = SerializationSize::exact(prefix).sum(variant);
            result = if result.valid {
                result.minmax(variant)
            } else {
                variant
            };
        }

        result
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
            result.insert(parameter.name.to_string(), *ty);
        }
        result
    }

    fn type_prefix_size(&self, ty: TyId) -> Option<u32> {
        let ty = self.context.type_interner().unwrap_alias(ty);
        let TyData::Struct { def, base, .. } = self.context.type_interner().data(ty) else {
            return None;
        };
        self.struct_prefix_size(base.unwrap_or(*def))
    }

    fn struct_prefix_size(&self, def: SymbolId) -> Option<u32> {
        let file = self.context.file_db().get_by_id(def.file_id)?;
        let TopLevel::Struct(structure) = file.find_syntax_declaration(def)? else {
            return None;
        };
        let prefix = structure.pack_prefix()?;
        prefix_width(prefix.text(file.source().source.as_ref()))
    }
}

fn type_definition(interner: &TypeInterner, ty: TyId) -> Option<SymbolId> {
    match interner.data(ty) {
        TyData::Struct { def, base, .. } => Some(base.unwrap_or(*def)),
        TyData::TypeAlias { def, .. } => Some(*def),
        _ => None,
    }
}

fn prefix_width(text: &str) -> Option<u32> {
    let value = text.replace('_', "");
    if let Some(hex) = value.strip_prefix("0x") {
        return u32::try_from(hex.len().checked_mul(4)?).ok();
    }
    if let Some(binary) = value.strip_prefix("0b") {
        return u32::try_from(binary.len()).ok();
    }

    let value = value.parse::<u128>().ok()?;
    Some((u128::BITS - value.leading_zeros()).max(1))
}

fn format_range(first: u32, second: u32, unit: &str) -> String {
    if first == second {
        let suffix = if first == 1 { "" } else { "s" };
        return format!("{first} {unit}{suffix}");
    }

    format!("{first}..{second} {unit}s")
}
