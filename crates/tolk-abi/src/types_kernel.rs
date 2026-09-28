use serde::{Deserialize, Serialize};

/// ABI and symbol-types JSON contain a flat `unique_types` table.
/// Other entities reference table entries by `TyIdx`.
pub type TyIdx = usize;

/// ABI type that fully reflects the type system in Tolk.
/// Mirrors TypeScript implementation `abi-types.ts`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum Ty {
    // primitives
    #[serde(rename = "int")]
    Int,
    #[serde(rename = "intN")]
    IntN { n: u32 },
    #[serde(rename = "uintN")]
    UintN { n: u32 },
    #[serde(rename = "varintN")]
    VarintN { n: u32 },
    #[serde(rename = "varuintN")]
    VaruintN { n: u32 },
    #[serde(rename = "coins")]
    Coins,
    #[serde(rename = "bool")]
    Bool,
    #[serde(rename = "cell")]
    Cell,
    #[serde(rename = "builder")]
    Builder,
    #[serde(rename = "slice")]
    Slice,
    #[serde(rename = "string")]
    String,
    #[serde(rename = "remaining")]
    Remaining,
    #[serde(rename = "address")]
    Address,
    #[serde(rename = "addressOpt")]
    AddressOpt,
    #[serde(rename = "addressExt")]
    AddressExt,
    #[serde(rename = "addressAny")]
    AddressAny,
    #[serde(rename = "bitsN")]
    BitsN { n: u32 },
    #[serde(rename = "nullLiteral")]
    NullLiteral,
    #[serde(rename = "callable")]
    Callable,
    #[serde(rename = "void")]
    Void,
    #[serde(rename = "unknown")]
    Unknown,

    // compound types
    #[serde(rename = "nullable")]
    Nullable {
        inner_ty_idx: TyIdx,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stack_type_id: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stack_width: Option<usize>,
    },
    #[serde(rename = "cellOf")]
    CellOf { inner_ty_idx: TyIdx },
    #[serde(rename = "arrayOf")]
    ArrayOf { inner_ty_idx: TyIdx },
    #[serde(rename = "lispListOf")]
    LispListOf { inner_ty_idx: TyIdx },
    #[serde(rename = "tensor")]
    Tensor { items_ty_idx: Vec<TyIdx> },
    #[serde(rename = "shapedTuple")]
    ShapedTuple { items_ty_idx: Vec<TyIdx> },
    #[serde(rename = "mapKV")]
    MapKV {
        key_ty_idx: TyIdx,
        value_ty_idx: TyIdx,
    },

    // references to user-defined types
    #[serde(rename = "EnumRef")]
    EnumRef { enum_name: String },
    #[serde(rename = "StructRef")]
    StructRef {
        struct_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        type_args_ty_idx: Option<Vec<TyIdx>>,
    },
    #[serde(rename = "AliasRef")]
    AliasRef {
        alias_name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        type_args_ty_idx: Option<Vec<TyIdx>>,
    },
    #[serde(rename = "genericT")]
    GenericT { name_t: String },
    #[serde(rename = "union")]
    Union {
        variants: Vec<UnionVariant>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stack_width: Option<usize>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UnionVariant {
    pub variant_ty_idx: TyIdx,
    pub prefix_num: u64,
    pub prefix_len: usize,
    pub is_prefix_implicit: Option<bool>,
    pub stack_type_id: Option<usize>,
    pub stack_width: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct StructInstantiation {
    pub ty_idx: TyIdx,
    pub struct_name: String,
    pub monomorphic_fields_ty_idx: Vec<TyIdx>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct AliasInstantiation {
    pub ty_idx: TyIdx,
    pub alias_name: String,
    pub monomorphic_target_ty_idx: TyIdx,
}

pub trait TyResolver {
    fn ty_by_idx(&self, ty_idx: TyIdx) -> Option<&Ty>;
    fn struct_field_ty_indices(&self, ty_idx: TyIdx) -> Option<Vec<TyIdx>>;
    fn alias_target_ty_idx(&self, ty_idx: TyIdx) -> Option<TyIdx>;
}

impl Ty {
    #[must_use]
    pub const fn is_typed_cell(&self) -> bool {
        matches!(self, Ty::CellOf { .. })
    }
}

#[must_use]
pub fn render_ty<R: TyResolver + ?Sized>(symbols: &R, ty_idx: TyIdx) -> String {
    let Some(ty) = symbols.ty_by_idx(ty_idx) else {
        return format!("ty#{ty_idx}");
    };

    match ty {
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
        Ty::Callable => "continuation".to_owned(),
        Ty::Void => "void".to_owned(),
        Ty::Unknown => "unknown".to_owned(),
        Ty::Nullable { inner_ty_idx, .. } => format!("{}?", render_ty(symbols, *inner_ty_idx)),
        Ty::CellOf { inner_ty_idx } => format!("Cell<{}>", render_ty(symbols, *inner_ty_idx)),
        Ty::ArrayOf { inner_ty_idx } => format!("array<{}>", render_ty(symbols, *inner_ty_idx)),
        Ty::LispListOf { inner_ty_idx } => {
            format!("lisp_list<{}>", render_ty(symbols, *inner_ty_idx))
        }
        Ty::Tensor { items_ty_idx } => format!(
            "({})",
            items_ty_idx
                .iter()
                .map(|&idx| render_ty(symbols, idx))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Ty::ShapedTuple { items_ty_idx } => format!(
            "[{}]",
            items_ty_idx
                .iter()
                .map(|&idx| render_ty(symbols, idx))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Ty::MapKV {
            key_ty_idx,
            value_ty_idx,
        } => format!(
            "map<{}, {}>",
            render_ty(symbols, *key_ty_idx),
            render_ty(symbols, *value_ty_idx)
        ),
        Ty::EnumRef { enum_name } => enum_name.clone(),
        Ty::StructRef {
            struct_name,
            type_args_ty_idx,
        } => render_named(symbols, struct_name, type_args_ty_idx.as_deref()),
        Ty::AliasRef {
            alias_name,
            type_args_ty_idx,
        } => render_named(symbols, alias_name, type_args_ty_idx.as_deref()),
        Ty::GenericT { name_t } => name_t.clone(),
        Ty::Union { variants, .. } => variants
            .iter()
            .map(|variant| render_ty(symbols, variant.variant_ty_idx))
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

#[must_use]
pub fn render_param_ty<R: TyResolver + ?Sized>(symbols: &R, ty_idx: TyIdx) -> String {
    match symbols.ty_by_idx(ty_idx) {
        Some(Ty::CellOf { inner_ty_idx }) => render_ty(symbols, *inner_ty_idx),
        _ => render_ty(symbols, ty_idx),
    }
}

fn render_named<R: TyResolver + ?Sized>(
    symbols: &R,
    name: &str,
    type_args_ty_idx: Option<&[TyIdx]>,
) -> String {
    let Some(type_args_ty_idx) = type_args_ty_idx else {
        return name.to_owned();
    };
    if type_args_ty_idx.is_empty() {
        return name.to_owned();
    }
    format!(
        "{name}<{}>",
        type_args_ty_idx
            .iter()
            .map(|&idx| render_ty(symbols, idx))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Calculate how many stack slots a type occupies.
#[must_use]
pub fn calc_width_on_stack<R: TyResolver + ?Sized>(symbols: &R, ty_idx: TyIdx) -> usize {
    let Some(ty) = symbols.ty_by_idx(ty_idx) else {
        return 1;
    };

    match ty {
        Ty::Void => {
            // void is like "unit", equal to an empty tensor
            0
        }
        Ty::Tensor { items_ty_idx } => {
            // a tensor is a sum of its elements
            items_ty_idx
                .iter()
                .map(|ty_idx| calc_width_on_stack(symbols, *ty_idx))
                .sum()
        }
        Ty::StructRef { .. } => {
            // a struct is a named tensor: fields one by one;
            // if a struct is generic `Wrapper<T>`, we have type_args T=xxx, and replace T in each field
            // (it works unless `T` is used in unions)
            symbols
                .struct_field_ty_indices(ty_idx)
                .unwrap_or_default()
                .iter()
                .map(|&field_ty_idx| calc_width_on_stack(symbols, field_ty_idx))
                .sum()
        }
        Ty::AliasRef { .. } => {
            // an alias is the same as its underlying (target) type;
            // if an alias is generic `Maybe<T>`, we have typeArgs T=xxx, and replace T in its target
            symbols
                .alias_target_ty_idx(ty_idx)
                .map_or(1, |target_ty_idx| {
                    calc_width_on_stack(symbols, target_ty_idx)
                })
        }
        Ty::Nullable { stack_width, .. } => {
            // for primitive nullables (common case), like `int?` and `address?`, it's 1 (TVM value or NULL);
            // for non-primitive nullables, the compiler inserts stack_width and stack_type_id
            stack_width.unwrap_or(1)
        }
        Ty::Union { stack_width, .. } => {
            // for union types, the compiler always inserts stack_width for simplicity (and stack_type_id for each variant)
            stack_width.unwrap_or(1)
        }
        Ty::GenericT { name_t } => {
            panic!("unexpected genericT={name_t} in calc_width_on_stack")
        }

        _ => {
            // almost all types are TVM primitives that occupy 1 stack slot:
            // - intN is TVM INT
            // - array<T> is TVM TUPLE
            // - map<K, V> is TVM DICT or NULL
            // etc.
            1
        }
    }
}
