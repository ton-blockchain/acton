use crate::source_map::SourceMap;
use serde::{Deserialize, Serialize};
use std::fmt;

/// ABI type that fully reflects the type system in Tolk.
/// Mirrors TypeScript implementation `abi-types.ts`.
#[derive(Serialize, Deserialize, Debug, Clone)]
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
        inner: Box<Ty>,
        stack_type_id: Option<usize>,
        stack_width: Option<usize>,
    },
    #[serde(rename = "cellOf")]
    CellOf { inner: Box<Ty> },
    #[serde(rename = "arrayOf")]
    ArrayOf { inner: Box<Ty> },
    #[serde(rename = "lispListOf")]
    LispListOf { inner: Box<Ty> },
    #[serde(rename = "tensor")]
    Tensor { items: Vec<Ty> },
    #[serde(rename = "shapedTuple")]
    ShapedTuple { items: Vec<Ty> },
    #[serde(rename = "mapKV")]
    MapKV { k: Box<Ty>, v: Box<Ty> },

    // references to user-defined types
    #[serde(rename = "EnumRef")]
    EnumRef { enum_name: String },
    #[serde(rename = "StructRef")]
    StructRef {
        struct_name: String,
        type_args: Option<Vec<Ty>>,
    },
    #[serde(rename = "AliasRef")]
    AliasRef {
        alias_name: String,
        type_args: Option<Vec<Ty>>,
    },
    #[serde(rename = "genericT")]
    GenericT { name_t: String },
    #[serde(rename = "union")]
    Union {
        variants: Vec<UnionVariant>,
        stack_width: Option<usize>,
    },
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Int => write!(f, "int"),
            Ty::IntN { n } => write!(f, "int{n}"),
            Ty::UintN { n } => write!(f, "uint{n}"),
            Ty::VarintN { n } => write!(f, "varint{n}"),
            Ty::VaruintN { n } => write!(f, "varuint{n}"),
            Ty::Coins => write!(f, "coins"),
            Ty::Bool => write!(f, "bool"),
            Ty::Cell => write!(f, "cell"),
            Ty::Builder => write!(f, "builder"),
            Ty::Slice => write!(f, "slice"),
            Ty::String => write!(f, "string"),
            Ty::Remaining => write!(f, "RemainingBitsAndRefs"),
            Ty::Address => write!(f, "address"),
            Ty::AddressOpt => write!(f, "address?"),
            Ty::AddressExt => write!(f, "ext_address"),
            Ty::AddressAny => write!(f, "any_address"),
            Ty::BitsN { n } => write!(f, "bits{n}"),
            Ty::NullLiteral => write!(f, "null"),
            Ty::Callable => write!(f, "continuation"),
            Ty::Void => write!(f, "void"),
            Ty::Unknown => write!(f, "unknown"),
            Ty::Nullable { inner, .. } => write!(f, "{inner}?"),
            Ty::CellOf { inner } => write!(f, "Cell<{inner}>"),
            Ty::ArrayOf { inner } => write!(f, "array<{inner}>"),
            Ty::LispListOf { inner } => write!(f, "lisp_list<{inner}>"),
            Ty::Tensor { items } => {
                write!(f, "(")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, ")")
            }
            Ty::ShapedTuple { items } => {
                write!(f, "[")?;
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{item}")?;
                }
                write!(f, "]")
            }
            Ty::MapKV { k, v } => write!(f, "map<{k}, {v}>"),
            Ty::EnumRef { enum_name } => write!(f, "{enum_name}"),
            Ty::StructRef {
                struct_name,
                type_args,
            } => {
                write!(f, "{struct_name}")?;
                if let Some(type_args) = type_args {
                    write!(f, "<")?;
                    for (i, item) in type_args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{item}")?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            }
            Ty::AliasRef {
                alias_name,
                type_args,
            } => {
                write!(f, "{alias_name}")?;
                if let Some(type_args) = type_args {
                    write!(f, "<")?;
                    for (i, item) in type_args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{item}")?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            }
            Ty::GenericT { name_t } => write!(f, "{name_t}"),
            Ty::Union { variants, .. } => {
                for (i, variant) in variants.iter().enumerate() {
                    let variant_ty = &variant.variant_ty;
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{variant_ty}")?;
                }
                Ok(())
            }
        }
    }
}

/// `UnionVariant` exists for every `T_i` in a union type `T1 | T2 | ...`.
///
/// For binary serialization, a union should have a prefix tree,
/// which is either defined explicitly with struct prefixes: `struct (0x12345678) CounterIncrement`,
/// or auto-generated (implicit), e.g. `int8 | int16 | int32` is serialized as '00'+int8 / '01'+int16 / '10'+int32.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UnionVariant {
    pub variant_ty: Ty,
    pub prefix_str: String,
    pub prefix_len: usize,
    pub is_prefix_implicit: Option<bool>,
    pub stack_type_id: Option<usize>,
    pub stack_width: Option<usize>,
}

/// Calculate how many stack slots a type occupies.
#[must_use]
pub fn calc_width_on_stack(symbols: &SourceMap, ty: &Ty) -> usize {
    match ty {
        Ty::Void => {
            // void is like "unit", equal to an empty tensor
            0
        }
        Ty::Tensor { items } => {
            // a tensor is a sum of its elements
            items
                .iter()
                .map(|item| calc_width_on_stack(symbols, item))
                .sum()
        }
        Ty::StructRef {
            struct_name,
            type_args,
        } => {
            // a struct is a named tensor: fields one by one;
            // if a struct is generic `Wrapper<T>`, we have type_args T=xxx, and replace T in each field
            // (it works unless `T` is used in unions)
            let struct_ref = symbols.get_struct(struct_name);
            struct_ref
                .fields
                .iter()
                .map(|f| match type_args {
                    Some(type_args) => {
                        let f_ty = instantiate_generics(
                            &f.ty,
                            struct_ref.type_params.as_deref().unwrap_or(&[]),
                            type_args,
                        );
                        calc_width_on_stack(symbols, &f_ty)
                    }
                    None => calc_width_on_stack(symbols, &f.ty),
                })
                .sum()
        }
        Ty::AliasRef {
            alias_name,
            type_args,
        } => {
            // an alias is the same as its underlying (target) type;
            // if an alias is generic `Maybe<T>`, we have typeArgs T=xxx, and replace T in its target
            let alias_ref = symbols.get_alias(alias_name);
            match type_args {
                Some(type_args) => {
                    let target_ty = instantiate_generics(
                        &alias_ref.target_ty,
                        alias_ref.type_params.as_deref().unwrap_or(&[]),
                        type_args,
                    );
                    calc_width_on_stack(symbols, &target_ty)
                }
                None => calc_width_on_stack(symbols, &alias_ref.target_ty),
            }
        }
        Ty::Nullable { stack_width, .. } => {
            // for primitive nullables (common case), like `int?` and `address?`, it's 1 (TVM value or NULL);
            // for non-primitive nullables, the compiler inserts stackWidth and stackTypeId
            stack_width.unwrap_or(1)
        }
        Ty::Union { stack_width, .. } => {
            // for union types, the compiler always inserts stackWidth for simplicity (and stackTypeId for each variant)
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

/// Replace all generic Ts (typeParams) with instantiation (typeArgs) recursively.
/// Example: `(int, T, Wrapper<T?>)` and T=coins → `(int, coins, Wrapper<coins?>)`
#[must_use]
pub fn instantiate_generics(ty: &Ty, type_params: &[String], type_args: &[Ty]) -> Ty {
    match ty {
        Ty::Nullable {
            inner,
            stack_type_id,
            stack_width,
        } => Ty::Nullable {
            inner: Box::new(instantiate_generics(inner, type_params, type_args)),
            stack_type_id: *stack_type_id,
            stack_width: *stack_width,
        },
        Ty::CellOf { inner } => Ty::CellOf {
            inner: Box::new(instantiate_generics(inner, type_params, type_args)),
        },
        Ty::ArrayOf { inner } => Ty::ArrayOf {
            inner: Box::new(instantiate_generics(inner, type_params, type_args)),
        },
        Ty::LispListOf { inner } => Ty::LispListOf {
            inner: Box::new(instantiate_generics(inner, type_params, type_args)),
        },
        Ty::Tensor { items } => Ty::Tensor {
            items: items
                .iter()
                .map(|i| instantiate_generics(i, type_params, type_args))
                .collect(),
        },
        Ty::ShapedTuple { items } => Ty::ShapedTuple {
            items: items
                .iter()
                .map(|i| instantiate_generics(i, type_params, type_args))
                .collect(),
        },
        Ty::MapKV { k, v } => Ty::MapKV {
            k: Box::new(instantiate_generics(k, type_params, type_args)),
            v: Box::new(instantiate_generics(v, type_params, type_args)),
        },
        Ty::StructRef {
            struct_name,
            type_args: ta,
        } => Ty::StructRef {
            struct_name: struct_name.clone(),
            type_args: ta.as_ref().map(|tas| {
                tas.iter()
                    .map(|t| instantiate_generics(t, type_params, type_args))
                    .collect()
            }),
        },
        Ty::AliasRef {
            alias_name,
            type_args: ta,
        } => Ty::AliasRef {
            alias_name: alias_name.clone(),
            type_args: ta.as_ref().map(|tas| {
                tas.iter()
                    .map(|t| instantiate_generics(t, type_params, type_args))
                    .collect()
            }),
        },
        Ty::Union {
            variants,
            stack_width,
        } => Ty::Union {
            variants: variants
                .iter()
                .map(|v| UnionVariant {
                    variant_ty: instantiate_generics(&v.variant_ty, type_params, type_args),
                    ..v.clone()
                })
                .collect(),
            stack_width: *stack_width,
        },
        Ty::GenericT { name_t } => {
            let idx = type_params.iter().position(|p| p == name_t).unwrap_or(100);

            type_args
                .get(idx)
                .unwrap_or_else(|| {
                    panic!("inconsistent generics: could not find type argument for {name_t}")
                })
                .clone()
        }
        _ => ty.clone(),
    }
}
