use crate::type_interner::TyId;
use std::sync::Arc;
use tolk_resolver::file_index::SymbolId;

/// Represents the data of a type. This is the main enum that holds all possible types in the language.
///
/// Types are interned in `TypeInterner` and referred to by `TyId`.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TyData {
    Struct {
        def: SymbolId,
        name: Arc<str>,
        base: Option<SymbolId>,
        /// Generic arguments if the struct is instantiated.
        args: Option<Vec<TyId>>,
    },
    Enum {
        def: SymbolId,
        name: Arc<str>,
    },
    TypeAlias {
        def: SymbolId,
        name: Arc<str>,
        inner_ty: TyId,
        /// Generic arguments if the alias is instantiated.
        args: Option<Vec<TyId>>,
    },
    /// A tensor type (e.g. `(int, int)`).
    Tensor(Vec<TyId>),
    /// A tuple type (e.g. `[int, int]`).
    Tuple(Vec<TyId>),
    /// An array type (e.g. `array<int>`).
    Array(TyId),
    /// A union type (e.g. `int | null`).
    Union(Vec<TyId>),
    Func {
        params: Vec<TyId>,
        return_ty: TyId,
    },
    /// A generic type parameter (e.g. `T`).
    TypeParameter {
        name: String,
        /// The default type if provided (as `T = int`).
        default_type: Option<TyId>,
    },
    /// `Wrapper<T>` when T is a generic (a struct is not ready to instantiate).
    /// `Wrapper<int>` is NOT here, it's an instantiated struct. Here is only when type arguments contain generics.
    /// Example: `type WrapperAlias<T> = Wrapper<T>`, then `Wrapper<T>` (underlying type of alias) is here.
    GenericTypeWithTs {
        inner_ty: TyId,
        types: Vec<TyId>,
    },
    Builtin {
        name: Arc<str>,
    },
    Int(IntTy),
    Bool {
        /// The value if known (e.g. `true`, `false`, or `None` for generic bool).
        value: Option<bool>,
    },
    Cell,
    Slice,
    Builder,
    Continuation,
    Address(AddressKind),
    MapKV {
        key: TyId,
        value: TyId,
    },
    Bits {
        size: usize,
    },
    Bytes {
        size: usize,
    },
    UntypedTuple, // tuple in C++
    Null,
    Void,
    Never,
    /// Type of function without explicit return type.
    Auto,
    Undefined,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AddressKind {
    Internal, // address
    Any,      // any_address
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum IntTy {
    Int,
    IntN { size: usize, unsigned: bool },
    VarIntN { size: usize, unsigned: bool },
    Coins,
}
