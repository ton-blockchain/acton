use crate::type_formatter::TypeFormatter;
use crate::type_substitutor::TypeSubstitutor;
use crate::types::*;
use rustc_hash::FxHashMap;
use std::sync::Arc;
use tolk_resolver::file_index::SymbolId;

/// A lightweight identifier for an interned type.
///
/// This is a handle that can be used to retrieve the actual type data from `TypeInterner`.
/// `TyId`s are cheap to copy and can be used as keys in maps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct TyId(u32);

/// Helper struct for displaying types using an interner.
///
/// This struct implements `std::fmt::Display` by delegating to `TypeFormatter`.
pub struct TyDisplay<'a> {
    id: TyId,
    interner: &'a TypeInterner,
}

impl std::fmt::Display for TyDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.interner.format(self.id))
    }
}

/// A type interner that stores all type definitions and ensures uniqueness.
///
/// Interning types allows for fast equality checks (pointer comparison of `TyId`)
/// and memory efficiency by deduplicating identical types.
#[derive(Debug, Clone)]
pub struct TypeInterner {
    arena: Vec<TyData>,           // TyId -> TyData
    map: FxHashMap<TyData, TyId>, // TyData -> TyId

    pub ty_undefined: TyId,
    pub ty_unknown: TyId,
    pub ty_auto: TyId, // special type for omitted return type of functions
    pub ty_int: TyId,
    pub ty_coins: TyId,
    pub ty_bool: TyId,
    pub ty_never: TyId,
    pub ty_void: TyId,
    pub ty_null: TyId,
    pub ty_untyped_tuple: TyId,
    pub ty_cell: TyId,
    pub ty_slice: TyId,
    pub ty_string: TyId,
    pub ty_builder: TyId,
    pub ty_continuation: TyId,
    pub ty_address_internal: TyId,
    pub ty_address_any: TyId,
}

impl Default for TypeInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeInterner {
    /// Creates a new `TypeInterner` with all builtin types pre-interned.
    #[must_use]
    pub fn new() -> Self {
        let mut this = Self {
            arena: Vec::new(),
            map: FxHashMap::default(),
            ty_undefined: TyId(0),
            ty_unknown: TyId(0),
            ty_int: TyId(0),
            ty_coins: TyId(0),
            ty_bool: TyId(0),
            ty_never: TyId(0),
            ty_void: TyId(0),
            ty_null: TyId(0),
            ty_untyped_tuple: TyId(0),
            ty_cell: TyId(0),
            ty_slice: TyId(0),
            ty_string: TyId(0),
            ty_builder: TyId(0),
            ty_continuation: TyId(0),
            ty_address_internal: TyId(0),
            ty_address_any: TyId(0),
            ty_auto: TyId(0),
        };

        // ty_undefined always go first for easier debugging (TyId = 0)
        this.ty_undefined = this.intern(TyData::Undefined);
        this.ty_unknown = this.intern(TyData::Unknown);
        this.ty_auto = this.intern(TyData::Auto);
        this.ty_int = this.intern(TyData::Int(IntTy::Int));
        this.ty_coins = this.intern(TyData::Int(IntTy::Coins));
        this.ty_bool = this.intern(TyData::Bool { value: None });
        this.ty_never = this.intern(TyData::Never);
        this.ty_void = this.intern(TyData::Void);
        this.ty_null = this.intern(TyData::Null);
        this.ty_untyped_tuple = this.intern(TyData::UntypedTuple);
        this.ty_cell = this.intern(TyData::Cell);
        this.ty_slice = this.intern(TyData::Slice);
        this.ty_string = this.builtin("string".into());
        this.ty_builder = this.intern(TyData::Builder);
        this.ty_continuation = this.intern(TyData::Continuation);
        this.ty_address_internal = this.intern(TyData::Address(AddressKind::Internal));
        this.ty_address_any = this.intern(TyData::Address(AddressKind::Any));

        this
    }

    /// Interns a type definition and returns its ID.
    /// If the type already exists, returns the existing ID.
    pub fn intern(&mut self, ty: TyData) -> TyId {
        if let Some(&id) = self.map.get(&ty) {
            return id;
        }
        let id = TyId(self.arena.len() as u32);
        self.arena.push(ty.clone());
        self.map.insert(ty, id);
        id
    }

    /// Returns a string representation of the type.
    #[must_use]
    pub fn format(&self, id: TyId) -> String {
        TypeFormatter::new(self).format(id)
    }

    /// Returns a helper object that implements `std::fmt::Display` for the given type ID.
    #[must_use]
    pub const fn display(&self, id: TyId) -> TyDisplay<'_> {
        TyDisplay { id, interner: self }
    }

    /// Retrieves the type data for a given ID.
    #[inline]
    #[must_use]
    pub fn data(&self, id: TyId) -> &TyData {
        &self.arena[id.0 as usize]
    }

    /// Creates a fixed-size integer type.
    pub fn int_n(&mut self, size: usize, unsigned: bool) -> TyId {
        self.intern(TyData::Int(IntTy::IntN { size, unsigned }))
    }

    /// Creates a variable-length integer type.
    pub fn varint_n(&mut self, size: usize, unsigned: bool) -> TyId {
        self.intern(TyData::Int(IntTy::VarIntN { size, unsigned }))
    }

    /// Creates a bit string type of given size.
    pub fn bits(&mut self, size: usize) -> TyId {
        self.intern(TyData::Bits { size })
    }

    /// Creates a byte string type of given size.
    pub fn bytes(&mut self, size: usize) -> TyId {
        self.intern(TyData::Bytes { size })
    }

    /// Creates a tuple type.
    pub fn tuple(&mut self, elements: Vec<TyId>) -> TyId {
        self.intern(TyData::Tuple(elements))
    }

    /// Creates an array type.
    pub fn array(&mut self, element_ty: TyId) -> TyId {
        self.intern(TyData::Array(element_ty))
    }

    /// Creates a tensor type.
    pub fn tensor(&mut self, elements: Vec<TyId>) -> TyId {
        self.intern(TyData::Tensor(elements))
    }

    /// Creates a nullable union type `T | null`.
    pub fn nullable_union(&mut self, nullable: TyId) -> TyId {
        self.union(vec![nullable, self.ty_null])
    }

    /// Creates a union type from a list of types.
    ///
    /// This method automatically flattens nested unions and deduplicates types.
    pub fn union(&mut self, elements: Vec<TyId>) -> TyId {
        // Reserve to avoid multiple reallocations if possible
        let mut flat_variants = Vec::with_capacity(elements.len());

        for el in elements {
            let unwrapped = self.unwrap_alias(el);
            if let TyData::Union(variants) = self.data(unwrapped) {
                for &v in variants {
                    self.append_union_variant(v, &mut flat_variants);
                }
            } else {
                self.append_union_variant(el, &mut flat_variants);
            }
        }

        if flat_variants.len() == 1 {
            return flat_variants[0];
        }

        self.intern(TyData::Union(flat_variants))
    }

    fn append_union_variant(&self, variant: TyId, out: &mut Vec<TyId>) {
        let underlying = self.unwrap_alias(variant);
        let is_duplicate = out.iter().any(|&existing| {
            // C++: existing->equal_to(underlying_variant)
            self.equals(existing, underlying)
        });
        if !is_duplicate {
            out.push(variant);
        }
    }

    /// Creates a function type.
    pub fn func(&mut self, params: Vec<TyId>, return_ty: TyId) -> TyId {
        self.intern(TyData::Func { params, return_ty })
    }

    /// Creates a builtin type.
    pub fn builtin(&mut self, name: Arc<str>) -> TyId {
        self.intern(TyData::Builtin { name })
    }

    /// Creates an address type.
    pub fn address(&mut self, kind: AddressKind) -> TyId {
        self.intern(TyData::Address(kind))
    }

    /// Creates a map (dictionary) type.
    pub fn map_kv(&mut self, key: TyId, value: TyId) -> TyId {
        self.intern(TyData::MapKV { key, value })
    }

    /// Creates a struct type.
    pub fn struct_ty(&mut self, def: SymbolId, name: Arc<str>) -> TyId {
        self.intern(TyData::Struct {
            def,
            name,
            base: None,
            args: None,
        })
    }

    /// Creates an instantiated struct type.
    pub fn struct_instantiation(
        &mut self,
        def: SymbolId,
        name: Arc<str>,
        base: SymbolId,
        args: Vec<TyId>,
    ) -> TyId {
        self.intern(TyData::Struct {
            def,
            name,
            base: Some(base),
            args: Some(args),
        })
    }

    /// Creates an enum type.
    pub fn enum_ty(&mut self, def: SymbolId, name: Arc<str>) -> TyId {
        self.intern(TyData::Enum { def, name })
    }

    /// Creates a type alias.
    pub fn type_alias(&mut self, def: SymbolId, name: Arc<str>, inner_ty: TyId) -> TyId {
        self.intern(TyData::TypeAlias {
            def,
            name,
            inner_ty,
            args: None,
        })
    }

    /// Creates an instantiated type alias.
    pub fn type_alias_instantiation(
        &mut self,
        def: SymbolId,
        name: Arc<str>,
        inner_ty: TyId,
        args: Vec<TyId>,
    ) -> TyId {
        self.intern(TyData::TypeAlias {
            def,
            name,
            inner_ty,
            args: Some(args),
        })
    }

    /// Creates an instantiation of a generic type.
    pub fn generic_type_with_ts(&mut self, inner_ty: TyId, types: Vec<TyId>) -> TyId {
        self.intern(TyData::GenericTypeWithTs { inner_ty, types })
    }

    /// Creates a type parameter type.
    pub fn type_parameter(&mut self, name: String, default_type: Option<TyId>) -> TyId {
        self.intern(TyData::TypeParameter { name, default_type })
    }

    /// when `var v = rhs`, `v` is `undefined` before assignment (before rhs->inferred_type is assigned to it);
    /// when `var (v1,v2,v3) = rhs`, left side is `(undefined,undefined,undefined)`
    pub(crate) fn is_type_undefined_from_var_lhs_decl(&self, id: TyId) -> bool {
        if id == self.ty_undefined {
            return true;
        }
        if let TyData::Tensor(items) = self.data(id) {
            return items
                .iter()
                .all(|&item| self.is_type_undefined_from_var_lhs_decl(item));
        }
        false
    }

    /// Unwraps type aliases to get the underlying type.
    #[must_use]
    pub fn unwrap_alias(&self, id: TyId) -> TyId {
        let mut current = id;
        while let TyData::TypeAlias { inner_ty, .. } = self.data(current) {
            current = *inner_ty;
        }
        current
    }

    /// having `type UserId = int` and `type OwnerId = int` (when their underlying types are equal),
    /// make `UserId` and `OwnerId` NOT equal and NOT assignable (although they'll have the same `type_id`);
    /// it allows overloading methods for these types independently, e.g.
    /// > type BalanceList = dict
    /// > type AssetList = dict
    /// > fun BalanceList.validate(self)
    /// > fun AssetList.validate(self)
    fn are_two_equal_type_aliases_different(&self, a_id: TyId, b_id: TyId) -> bool {
        let (a_def, a_inner, a_args) = match self.data(a_id) {
            TyData::TypeAlias {
                def,
                inner_ty,
                args,
                ..
            } => (*def, *inner_ty, args.as_ref()),
            _ => return true,
        };
        let (b_def, b_inner, b_args) = match self.data(b_id) {
            TyData::TypeAlias {
                def,
                inner_ty,
                args,
                ..
            } => (*def, *inner_ty, args.as_ref()),
            _ => return true,
        };

        if a_def == b_def {
            return match (a_args, b_args) {
                (Some(aa), Some(bb)) => {
                    if aa.len() != bb.len() {
                        return true;
                    }
                    !aa.iter()
                        .zip(bb.iter())
                        .all(|(&at, &bt)| self.equals(at, bt))
                }
                (None, None) => false,
                _ => true,
            };
        }

        if let Some(a_args) = a_args
            && let Some(b_args) = b_args
        {
            if a_args.len() != b_args.len() {
                return true;
            }
            return !a_args
                .iter()
                .zip(b_args.iter())
                .all(|(&at, &bt)| self.equals(at, bt));
        }

        // handle `type MInt2 = MInt1`, as well as `type BalanceList = dict`, then they are equal
        let one_aliases_another = match (self.data(a_inner), self.data(b_inner)) {
            (TyData::TypeAlias { def: def1, .. }, _) if *def1 == b_def => true,
            (_, TyData::TypeAlias { def: def2, .. }) if *def2 == a_def => true,
            _ => false,
        };
        !one_aliases_another
    }

    /// Checks if a type contains any generic parameters.
    #[must_use]
    pub fn has_generics(&self, id: TyId) -> bool {
        let data = self.data(id);
        match data {
            TyData::TypeParameter { .. } => true,
            TyData::TypeAlias { inner_ty, args, .. } => {
                self.has_generics(*inner_ty)
                    || args
                        .as_ref()
                        .is_some_and(|list| list.iter().any(|&arg| self.has_generics(arg)))
            }
            TyData::Tensor(items) | TyData::Tuple(items) | TyData::Union(items) => {
                items.iter().any(|&item| self.has_generics(item))
            }
            TyData::Array(item) => self.has_generics(*item),
            TyData::Func { params, return_ty } => {
                params.iter().any(|&p| self.has_generics(p)) || self.has_generics(*return_ty)
            }
            TyData::GenericTypeWithTs { inner_ty, types } => {
                self.has_generics(*inner_ty) || types.iter().any(|&t| self.has_generics(t))
            }
            TyData::Struct { args, .. } => args
                .as_ref()
                .is_some_and(|list| list.iter().any(|&arg| self.has_generics(arg))),
            TyData::MapKV { key, value } => self.has_generics(*key) || self.has_generics(*value),
            _ => false,
        }
    }

    /// comparing types for equality (when implementation differs from a default "compare ids");
    /// two types are EQUAL is a much more strict property than "assignable";
    /// a union type can hold only non-equal types; for instance, having `type MyInt = int`, a union `int | MyInt` == `int`;
    /// searching for a compatible method for a receiver is also based on `equal_to()` as first priority
    #[must_use]
    pub fn equals(&self, a: TyId, b: TyId) -> bool {
        if a == b {
            return true;
        }

        let da = self.data(a);
        let db = self.data(b);

        // given `type UserId = int` and `type OwnerId = int`, treat them as NOT equal (they are also not assignable);
        // (but nevertheless, they will have the same type_id, and `UserId | OwnerId` is not a valid union)
        if matches!(da, TyData::TypeAlias { .. }) {
            if matches!(db, TyData::TypeAlias { .. })
                && let (
                    TyData::TypeAlias {
                        def: a_def,
                        inner_ty: ia,
                        ..
                    },
                    TyData::TypeAlias {
                        def: b_def,
                        inner_ty: ib,
                        ..
                    },
                ) = (da, db)
                && (*a_def == *b_def || self.equals(*ia, *ib))
            {
                return !self.are_two_equal_type_aliases_different(a, b);
            }
            if let TyData::TypeAlias { inner_ty: ia, .. } = da {
                return self.equals(*ia, b);
            }
        }

        if let TyData::TypeAlias { inner_ty: ib, .. } = db {
            return self.equals(a, *ib);
        }

        match (da, db) {
            (TyData::Int(ia), TyData::Int(ib)) => ia == ib,
            (TyData::Bool { value: va }, TyData::Bool { value: vb }) => va == vb,
            (TyData::Cell, TyData::Cell)
            | (TyData::Slice, TyData::Slice)
            | (TyData::Builder, TyData::Builder)
            | (TyData::Continuation, TyData::Continuation)
            | (TyData::Void, TyData::Void)
            | (TyData::Null, TyData::Null)
            | (TyData::Never, TyData::Never)
            | (TyData::Undefined, TyData::Undefined)
            | (TyData::Unknown, TyData::Unknown)
            | (TyData::UntypedTuple, TyData::UntypedTuple) => true,
            (TyData::Address(ka), TyData::Address(kb)) => ka == kb,
            (TyData::Bits { size: sa }, TyData::Bits { size: sb })
            | (TyData::Bytes { size: sa }, TyData::Bytes { size: sb }) => sa == sb,
            (TyData::Builtin { name: na }, TyData::Builtin { name: nb }) => na == nb,
            (
                TyData::Struct {
                    def: da,
                    base: base_a,
                    args: aa,
                    ..
                },
                TyData::Struct {
                    def: db,
                    base: bb,
                    args: ab,
                    ..
                },
            ) => {
                if da != db {
                    return false;
                }
                if let (Some(base_a), Some(base_b)) = (base_a, bb)
                    && base_a == base_b
                    && let (Some(args_a), Some(args_b)) = (aa, ab)
                    && args_a.len() == args_b.len()
                {
                    return args_a
                        .iter()
                        .zip(args_b.iter())
                        .all(|(&ta, &tb)| self.equals(ta, tb));
                }
                false
            }
            (TyData::Enum { def: da, .. }, TyData::Enum { def: db, .. }) => da == db,
            (TyData::Tensor(ta), TyData::Tensor(tb)) => {
                if ta.len() != tb.len() {
                    return false;
                }
                ta.iter()
                    .zip(tb.iter())
                    .all(|(&ea, &eb)| self.equals(ea, eb))
            }
            (TyData::Tuple(ta), TyData::Tuple(tb)) => {
                if ta.len() != tb.len() {
                    return false;
                }
                ta.iter()
                    .zip(tb.iter())
                    .all(|(&ea, &eb)| self.equals(ea, eb))
            }
            (TyData::Array(ta), TyData::Array(tb)) => self.equals(*ta, *tb),
            (
                TyData::Func {
                    params: pa,
                    return_ty: ra,
                },
                TyData::Func {
                    params: pb,
                    return_ty: rb,
                },
            ) => {
                if pa.len() != pb.len() {
                    return false;
                }
                if !self.equals(*ra, *rb) {
                    return false;
                }
                pa.iter()
                    .zip(pb.iter())
                    .all(|(&ea, &eb)| self.equals(ea, eb))
            }
            (TyData::Union(ua), TyData::Union(ub)) => {
                if ua.len() != ub.len() {
                    return false;
                }
                // self.has_all_variants_of(rhs)
                self.has_all_variants_of(a, b)
            }
            (TyData::MapKV { key: ka, value: va }, TyData::MapKV { key: kb, value: vb }) => {
                self.equals(*ka, *kb) && self.equals(*va, *vb)
            }
            (TyData::TypeParameter { name: na, .. }, TyData::TypeParameter { name: nb, .. }) => {
                na == nb
            }
            (
                TyData::GenericTypeWithTs {
                    inner_ty: ia,
                    types: ta,
                },
                TyData::GenericTypeWithTs {
                    inner_ty: ib,
                    types: tb,
                },
            ) => {
                if !self.same_generic_constructor(*ia, *ib) {
                    return false;
                }
                if ta.len() != tb.len() {
                    return false;
                }
                ta.iter()
                    .zip(tb.iter())
                    .all(|(&ea, &eb)| self.equals(ea, eb))
            }
            _ => false,
        }
    }

    fn array_element_type(&self, ty: TyId) -> Option<TyId> {
        let ty = self.unwrap_alias(ty);
        match self.data(ty) {
            TyData::Array(item) => Some(*item),
            _ => None,
        }
    }

    /// on `var lhs: <lhs_type> = rhs`, having inferred `rhs_type`, check that it can be assigned without any casts
    /// the same goes for passing arguments, returning values, etc. — where the "receiver" (lhs) checks "applier" (rhs)
    /// note, that `int8 | int16` is not assignable to `int` (even though both are assignable),
    /// because the only way to work with union types is to use `match`/`is` operators
    #[must_use]
    pub fn can_rhs_be_assigned(&self, lhs: TyId, rhs: TyId) -> bool {
        if self.equals(lhs, rhs) {
            return true;
        }

        let dl = self.data(lhs);
        let dr = self.data(rhs);

        if matches!(dl, TyData::Unknown) {
            return true;
        }
        if matches!(dl, TyData::Undefined) {
            return true;
        }
        if matches!(dr, TyData::Never) {
            return true;
        }

        if matches!(dl, TyData::TypeAlias { .. }) {
            // having `type UserId = int` and `type OwnerId = int`, make them NOT assignable without `as`
            // (although they both have the same type_id)
            if matches!(dr, TyData::TypeAlias { .. })
                && let (
                    TyData::TypeAlias { inner_ty: il, .. },
                    TyData::TypeAlias { inner_ty: ir, .. },
                ) = (dl, dr)
                && self.equals(*il, *ir)
            {
                return !self.are_two_equal_type_aliases_different(lhs, rhs);
            }
            if let TyData::TypeAlias { inner_ty: il, .. } = dl {
                return self.can_rhs_be_assigned(*il, rhs);
            }
        }

        if let TyData::TypeAlias { inner_ty: ir, .. } = dr {
            return self.can_rhs_be_assigned(lhs, *ir);
        }

        match (dl, dr) {
            (TyData::Union(lhs_variants), _) => {
                // `int` to `int | slice`, `int?` to `int8?`, `(int, null)` to `(int, T?) | slice`
                if let Some(_variant) =
                    self.calculate_exact_variant_to_fit_rhs(lhs, lhs_variants, rhs)
                {
                    return true;
                }
                if let TyData::Union(_) = dr {
                    return self.has_all_variants_of(lhs, rhs);
                }
                false
            }
            (_, TyData::Union(ur)) => {
                // If LHS is not a union, then all variants of RHS must be assignable to LHS
                ur.iter()
                    .all(|&variant| self.can_rhs_be_assigned(lhs, variant))
            }
            (TyData::Int(IntTy::IntN { .. }), TyData::Int(IntTy::Int))
            | (TyData::Int(IntTy::VarIntN { .. }), TyData::Int(IntTy::Int)) => true,
            (
                TyData::Int(IntTy::IntN {
                    size: sl,
                    unsigned: ul,
                    ..
                }),
                TyData::Int(IntTy::IntN {
                    size: sr,
                    unsigned: ur,
                    ..
                }),
            ) => {
                // `int8` is NOT assignable to `int32` without `as`
                sl == sr && ul == ur
            }
            (
                TyData::Int(IntTy::VarIntN {
                    size: sl,
                    unsigned: ul,
                    ..
                }),
                TyData::Int(IntTy::VarIntN {
                    size: sr,
                    unsigned: ur,
                    ..
                }),
            ) => sl == sr && ul == ur,
            (TyData::Int(il), _) => match il {
                IntTy::Int => matches!(
                    dr,
                    TyData::Int(IntTy::IntN { .. } | IntTy::VarIntN { .. } | IntTy::Coins)
                ),
                IntTy::Coins => matches!(dr, TyData::Int(IntTy::Int)),
                _ => false,
            },
            (TyData::Cell, TyData::Struct { name, .. }) => {
                // Typed cell `Cell<T>` is assignable to untyped `cell`.
                name.as_ref() == "Cell"
            }
            (TyData::Cell, TyData::GenericTypeWithTs { inner_ty, .. }) => {
                // Cell<Something> to cell, e.g. `contract.setData(obj.toCell())`
                if let TyData::Struct { name, .. } = self.data(*inner_ty) {
                    return name.as_ref() == "Cell";
                }
                false
            }
            (
                TyData::Func {
                    params: pl,
                    return_ty: rl,
                },
                TyData::Func {
                    params: pr,
                    return_ty: rr,
                },
            ) => {
                if pl.len() != pr.len() {
                    return false;
                }
                for (pl, pr) in pl.iter().zip(pr.iter()) {
                    if !self.can_rhs_be_assigned(*pr, *pl) || !self.can_rhs_be_assigned(*pl, *pr) {
                        return false;
                    }
                }
                self.can_rhs_be_assigned(*rl, *rr) && self.can_rhs_be_assigned(*rr, *rl)
            }
            (TyData::Tuple(tl), TyData::Tuple(tr)) => {
                if tl.len() != tr.len() {
                    return false;
                }
                tl.iter()
                    .zip(tr.iter())
                    .all(|(&el, &er)| self.can_rhs_be_assigned(el, er))
            }
            (TyData::Tensor(tl), TyData::Tensor(tr)) => {
                if tl.len() != tr.len() {
                    return false;
                }
                tl.iter()
                    .zip(tr.iter())
                    .all(|(&el, &er)| self.can_rhs_be_assigned(el, er))
            }
            (TyData::Array(item_l), TyData::Array(item_r)) => {
                self.can_rhs_be_assigned(*item_l, *item_r)
            }
            (TyData::Array(item_l), TyData::Tuple(items)) => items
                .iter()
                .all(|&item| self.can_rhs_be_assigned(*item_l, item)),
            (TyData::MapKV { key: kl, value: vl }, TyData::MapKV { key: kr, value: vr }) => {
                self.equals(*kl, *kr) && self.equals(*vl, *vr)
            }
            (
                TyData::GenericTypeWithTs {
                    inner_ty: il,
                    types: tl,
                },
                TyData::GenericTypeWithTs {
                    inner_ty: ir,
                    types: tr,
                },
            ) => {
                if !self.same_generic_constructor(*il, *ir) {
                    return false;
                }
                if tl.len() != tr.len() {
                    return false;
                }
                tl.iter()
                    .zip(tr.iter())
                    .all(|(&el, &er)| self.can_rhs_be_assigned(el, er))
            }
            (TyData::Struct { def: dl, .. }, TyData::Struct { def: dr, .. }) => {
                // C<C<int>> = C<CIntAlias>
                if dl != dr {
                    return false;
                }
                // Check struct equality using equal_to
                self.equals(lhs, rhs)
            }
            (TyData::Enum { def: dl, .. }, TyData::Enum { def: dr, .. }) => dl == dr,
            (TyData::Bits { size: sl }, TyData::Bits { size: sr })
            | (TyData::Bytes { size: sl }, TyData::Bytes { size: sr }) => sl == sr,
            _ => false,
        }
    }

    fn same_generic_constructor(&self, lhs_inner: TyId, rhs_inner: TyId) -> bool {
        if self.equals(lhs_inner, rhs_inner) {
            return true;
        }

        match (self.data(lhs_inner), self.data(rhs_inner)) {
            (TyData::Struct { def: lhs_def, .. }, TyData::Struct { def: rhs_def, .. })
                if lhs_def == rhs_def =>
            {
                return true;
            }
            (TyData::TypeAlias { def: lhs_def, .. }, TyData::TypeAlias { def: rhs_def, .. })
                if lhs_def == rhs_def =>
            {
                return true;
            }
            _ => {}
        }

        let lhs_unwrapped = self.unwrap_alias(lhs_inner);
        let rhs_unwrapped = self.unwrap_alias(rhs_inner);
        match (self.data(lhs_unwrapped), self.data(rhs_unwrapped)) {
            (TyData::Struct { def: lhs_def, .. }, TyData::Struct { def: rhs_def, .. })
            | (TyData::TypeAlias { def: lhs_def, .. }, TyData::TypeAlias { def: rhs_def, .. }) => {
                lhs_def == rhs_def
            }
            _ => false,
        }
    }

    /// Checks if `from` can be cast to `to` using the `as` operator.
    #[must_use]
    pub fn can_be_casted_with_as_operator(&self, from: TyId, to: TyId) -> bool {
        if self.can_rhs_be_assigned(to, from) {
            return true;
        }

        let df = self.data(from);
        let dt = self.data(to);

        if let TyData::TypeAlias { inner_ty, .. } = df {
            return self.can_be_casted_with_as_operator(*inner_ty, to);
        }
        if let TyData::TypeAlias { inner_ty, .. } = dt {
            return self.can_be_casted_with_as_operator(from, *inner_ty);
        }

        if let TyData::Union(variants) = dt {
            // common helper for union types:
            // - `int as int?` is ok
            // - `int8 as int16?` is ok (primitive 1-slot nullable don't store UTag, rules are less strict)
            // - `int as int | int16` is ok (exact match one of types)
            // - `int as slice | null` is NOT ok (no rhs subtype fits)
            // - `int as int8 | int16` is NOT ok (ambiguity)

            if self.is_primitive_nullable(to) {
                let or_null = self.get_union_or_null(to).unwrap_or_default();
                return from == self.ty_null || self.can_be_casted_with_as_operator(from, or_null);
            }

            // `int8 | int16` as `int16 | int8 | slice`
            if let TyData::Union(_) = df {
                return self.has_all_variants_of(to, from);
            }

            return self
                .calculate_exact_variant_to_fit_rhs(to, variants, from)
                .is_some();
        }

        match (df, dt) {
            // int as intN, intN as int, etc.
            // int as Color (all enums are integer)
            // bool as int
            // bool as intN (not uint)
            (TyData::Int(_), TyData::Int(_))
            | (TyData::Int(_), TyData::Enum { .. })
            | (TyData::Bool { .. }, TyData::Int(IntTy::Int))
            | (
                TyData::Bool { .. },
                TyData::Int(IntTy::IntN {
                    unsigned: false, ..
                }),
            )
            // `slice` to `bytes32` / `slice` to `bits8`
            // `slice` to `address`
            // `any_address` as `address` and any other casts are ok
            // all enums are integers, they can be `as` cast to each other
            | (TyData::Slice, TyData::Bits { .. })
            | (TyData::Slice, TyData::Bytes { .. })
            | (TyData::Slice, TyData::Address(_))
            | (TyData::Address(_), TyData::Slice)
            | (TyData::Address(_), TyData::Bits { .. })
            | (TyData::Address(_), TyData::Address(_))
            | (TyData::Enum { .. }, TyData::Int(_))
            | (TyData::Enum { .. }, TyData::Enum { .. })
            // `[int, int]` as `tuple`
            | (TyData::Tuple(_), TyData::UntypedTuple)
            | (TyData::Never, _) => true,
            (TyData::Cell, TyData::GenericTypeWithTs { inner_ty, .. }) => {
                // cell as Cell<T>
                if let TyData::Struct { name, .. } = self.data(*inner_ty) {
                    return name.as_ref() == "Cell";
                }
                false
            }
            (TyData::Tuple(tf), TyData::Tuple(tt)) if tf.len() == tt.len() => tf
                .iter()
                .zip(tt.iter())
                .all(|(&f, &t)| self.can_be_casted_with_as_operator(f, t)),
            (TyData::Tensor(tf), TyData::Tensor(tt)) if tf.len() == tt.len() => tf
                .iter()
                .zip(tt.iter())
                .all(|(&f, &t)| self.can_be_casted_with_as_operator(f, t)),
            (TyData::Unknown | TyData::Undefined, _) => self.get_width_on_stack(to) == 1,
            _ => false,
        }
    }

    /// calculate, how many stack slots the type occupies, e.g. `int`=1, `(int,int)`=2, `(int,int)?`=3
    /// it's calculated dynamically (not saved at `TypeData`*`::create`) to overcome problems with
    /// - recursive struct mentions (to create `TypeDataStruct` without knowing width of children)
    /// - uninitialized generics (that don't make any sense upon being instantiated)
    #[must_use]
    pub fn get_width_on_stack(&self, id: TyId) -> usize {
        match self.data(id) {
            TyData::TypeAlias { inner_ty, .. } => self.get_width_on_stack(*inner_ty),
            TyData::Tensor(items) | TyData::Tuple(items) => {
                items.iter().map(|&t| self.get_width_on_stack(t)).sum()
            }
            TyData::Union(_) => {
                if self.is_primitive_nullable(id)
                    && self
                        .can_hold_tvm_null_instead(self.get_union_or_null(id).unwrap_or_default())
                {
                    return 1;
                }
                if let TyData::Union(variants) = self.data(id) {
                    // `T1 | T2 | ...` occupy max(W[i]) + 1 slot for UTag (stores type_id or 0 for null)
                    let max_child_width = variants
                        .iter()
                        .filter(|&&v| v != self.ty_null) // `Empty | () | null` totally should be 1 (0 + 1 for UTag)
                        .map(|&v| self.get_width_on_stack(v))
                        .max()
                        .unwrap_or(0);
                    return max_child_width + 1;
                }
                1
            }
            TyData::Never | TyData::Void => 0,
            _ => {
                // Most types, including structs, occupy a single stack slot.
                // Struct width can be refined later if field-level accounting becomes necessary.
                1
            }
        }
    }

    /// assigning `null` to a primitive variable like `int?` / `cell?` can store TVM NULL inside the same slot
    /// (that's why the default implementation is just "return true", and most of types occupy 1 slot)
    /// but for complex variables, like `(int, int)?`, "null presence" is kept in a separate slot (`UTag` for union types)
    /// though still, tricky situations like `(int, ())?` can still "embed" TVM NULL in parallel with original value
    #[must_use]
    pub fn can_hold_tvm_null_instead(&self, id: TyId) -> bool {
        match self.data(id) {
            TyData::TypeAlias { inner_ty, .. } => self.can_hold_tvm_null_instead(*inner_ty),
            TyData::Struct { .. } => {
                if self.get_width_on_stack(id) != 1 {
                    // example that can hold null: `{ field: int }`
                    return false; // another example: `{ e: Empty, field: ((), int) }`
                } // examples can NOT: `{ field1: int, field2: int }`, `{ field1: int? }`
                // Check if the only field can hold null
                true
            }
            TyData::Tuple(items) | TyData::Tensor(items) => {
                if self.get_width_on_stack(id) != 1 {
                    // `(int, int)` / `()` can not hold null instead, since null is 1 slot
                    return false;
                }
                // only `((), int)` and similar can: one item is width 1 (and not nullable), others are 0
                items.iter().all(|&t| {
                    if self.get_width_on_stack(t) == 1 {
                        self.can_hold_tvm_null_instead(t)
                    } else {
                        true
                    }
                })
            }
            TyData::Union(_) => {
                if self.get_width_on_stack(id) != 1 {
                    // `(int, int)?` / `()?` can not hold null instead
                    return false; // only `int?` / `cell?` / `StructWith1IntField?` can
                }
                if let Some(or_null) = self.get_union_or_null(id) {
                    !self.can_hold_tvm_null_instead(or_null)
                } else {
                    false
                }
            }
            TyData::MapKV { .. } | TyData::Never | TyData::Void => false,
            _ => true,
        }
    }

    /// "primitive nullable" is `T?` which holds TVM NULL in the same slot (it other words, has no `UTag` slot)
    /// true : `int?`, `slice?`, `StructWith1IntField?`
    /// false: `(int, int)?`, `ComplexStruct?`, `()?`
    fn is_primitive_nullable(&self, id: TyId) -> bool {
        if let TyData::Union(variants) = self.data(id)
            && variants.len() == 2
        {
            return variants.contains(&self.ty_null);
        }
        false
    }

    fn get_union_or_null(&self, id: TyId) -> Option<TyId> {
        if let TyData::Union(variants) = self.data(id)
            && variants.len() == 2
            && variants.contains(&self.ty_null)
        {
            return variants.iter().find(|&&v| v != self.ty_null).copied();
        }
        None
    }

    //+ CHECKED
    /// given this = `T1 | T2 | ...` and `rhs_type`, find the only (not ambiguous) `T_i` that can accept it
    pub(crate) fn calculate_exact_variant_to_fit_rhs(
        &self,
        union_id: TyId,
        union_variants: &[TyId],
        rhs_id: TyId,
    ) -> Option<TyId> {
        let rhs_id_unwrapped = self.unwrap_alias(rhs_id);
        if let TyData::Union(_) = self.data(rhs_id_unwrapped) {
            // primitive 1-slot nullable don't store type_id, they can be assigned less strict, like `int?` to `int16?`
            if self.is_primitive_nullable(union_id) && self.is_primitive_nullable(rhs_id) {
                let or_null_l = self.get_union_or_null(union_id)?;
                let or_null_r = self.get_union_or_null(rhs_id)?;
                if self.can_rhs_be_assigned(or_null_l, or_null_r) {
                    return Some(union_id);
                }
            }
            return None;
        }

        // `int` to `int | int8` is okay: exact type matching
        for &variant in union_variants {
            if self.equals(variant, rhs_id) {
                return Some(variant);
            }
        }

        // find the only T_i; it would also be used for transition at IR generation, like `(int,null)` to `(int, User?) | int`
        let mut first_covering = None;
        for &variant in union_variants {
            if self.can_rhs_be_assigned(variant, rhs_id) {
                if first_covering.is_some() {
                    return None; // Ambiguous
                }
                first_covering = Some(variant);
            }
        }

        first_covering
    }

    #[must_use]
    pub fn has_all_variants_of(&self, union_a: TyId, union_b: TyId) -> bool {
        if let (TyData::Union(variants_a), TyData::Union(variants_b)) =
            (self.data(union_a), self.data(union_b))
        {
            for &vb in variants_b {
                if !self.has_variant_equal_to(variants_a, vb) {
                    return false;
                }
            }
            return true;
        }
        false
    }

    pub(crate) fn has_variant_equal_to(&self, variants: &[TyId], target: TyId) -> bool {
        variants.iter().any(|&v| self.equals(v, target))
    }

    /// "type lca" for a and b is T, so that both are assignable to T
    /// it's used
    /// 1) for auto-infer return type of the function if not specified
    ///    example: `fun f(x: int?) { ... return 1; ... return x; }`; lca(`int`,`int?`) = `int?`
    /// 2) for auto-infer type of ternary and `match` expressions
    ///    example: `cond ? beginCell() : null`; lca(`builder`,`null`) = `builder?`
    /// 3) when two data flows rejoin
    ///    example: `if (tensorVar != null) ... else ...` rejoin `(int,int)` and `null` into `(int,int)?`
    ///
    /// when lca can't be calculated (example: `(int,int)` and `(int,int,int)`), nullptr is returned
    pub fn calculate_type_lca(&mut self, a: TyId, b: TyId) -> TyId {
        if self.equals(a, b) {
            return a;
        }

        if a == self.ty_undefined || b == self.ty_undefined {
            return self.ty_undefined;
        }

        if a == self.ty_unknown || b == self.ty_unknown {
            return self.ty_unknown;
        }

        if a == self.ty_never {
            return b;
        }
        if b == self.ty_never {
            return a;
        }

        if a == self.ty_null {
            return self.nullable_union(b);
        }
        if b == self.ty_null {
            return self.nullable_union(a);
        }

        let data_a = self.data(a).clone();
        let data_b = self.data(b).clone();

        if let (TyData::Tensor(tensor1), TyData::Tensor(tensor2)) = (&data_a, &data_b)
            && tensor1.len() == tensor2.len()
        {
            let mut types_lca = Vec::with_capacity(tensor1.len());
            for i in 0..tensor1.len() {
                let next = self.calculate_type_lca(tensor1[i], tensor2[i]);
                types_lca.push(next);
            }
            return self.tensor(types_lca);
        }

        if let (TyData::Tuple(tuple1), TyData::Tuple(tuple2)) = (&data_a, &data_b)
            && tuple1.len() == tuple2.len()
        {
            let mut types_lca = Vec::with_capacity(tuple1.len());
            for i in 0..tuple1.len() {
                let next = self.calculate_type_lca(tuple1[i], tuple2[i]);
                types_lca.push(next);
            }
            return self.tuple(types_lca);
        }

        if let (Some(item_a), Some(item_b)) =
            (self.array_element_type(a), self.array_element_type(b))
        {
            let item_lca = self.calculate_type_lca(item_a, item_b);
            return self.array(item_lca);
        }

        // became_union parameter omitted for simplicity since we don't need it
        self.union(vec![a, b])
    }

    /// return `T`, so that `T + subtract_type` = type
    /// example: `int?` - `null` = `int`
    /// example: `int | slice | builder | bool` - `bool | slice` = `int | builder`
    /// what for: `if (x != null)` / `if (x is T)`, to smart cast x inside if
    pub fn calculate_type_subtract_rhs_type(&mut self, ty: TyId, subtract_ty: TyId) -> TyId {
        if self.equals(ty, self.ty_unknown) && self.equals(subtract_ty, self.ty_null) {
            // `unknown - null = unknown`
            return self.ty_unknown;
        }

        let Some(lhs_union) = self.collect_union_variants_for_subtract(ty, 0) else {
            return self.ty_never;
        };

        let mut rest_variants = Vec::new();

        if let Some(sub_union) = self.collect_union_variants_for_subtract(subtract_ty, 0) {
            let subtract_is_subset = sub_union
                .iter()
                .all(|&sub_variant| self.has_variant_equal_to(&lhs_union, sub_variant));
            if subtract_is_subset {
                rest_variants.reserve(lhs_union.len().saturating_sub(sub_union.len()));
                for &lhs_variant in &lhs_union {
                    if !self.has_variant_equal_to(&sub_union, lhs_variant) {
                        rest_variants.push(lhs_variant);
                    }
                }
            }
        } else if self.has_variant_equal_to(&lhs_union, subtract_ty) {
            rest_variants.reserve(lhs_union.len() - 1);
            for &lhs_variant in &lhs_union {
                if !self.equals(lhs_variant, subtract_ty) {
                    rest_variants.push(lhs_variant);
                }
            }
        }

        if rest_variants.is_empty() {
            return self.ty_never;
        }
        if rest_variants.len() == 1 {
            return rest_variants[0];
        }
        self.union(rest_variants)
    }

    fn collect_union_variants_for_subtract(&mut self, ty: TyId, depth: usize) -> Option<Vec<TyId>> {
        if depth > 8 {
            return None;
        }

        let unwrapped = self.unwrap_alias(ty);
        match self.data(unwrapped).clone() {
            TyData::Union(variants) => Some(variants),
            TyData::TypeAlias { inner_ty, .. } => {
                self.collect_union_variants_for_subtract(inner_ty, depth + 1)
            }
            TyData::GenericTypeWithTs { inner_ty, types } => {
                let inner_data = self.data(inner_ty).clone();
                if let TyData::TypeAlias {
                    inner_ty: alias_inner,
                    args: Some(formal_args),
                    ..
                } = inner_data
                {
                    let mut mapping = FxHashMap::default();
                    for (&formal, &actual) in formal_args.iter().zip(types.iter()) {
                        if let TyData::TypeParameter { name, .. } = self.data(formal) {
                            mapping.insert(name.clone(), actual);
                        }
                    }

                    let instantiated = if mapping.is_empty() {
                        alias_inner
                    } else {
                        let mut substitutor = TypeSubstitutor::new(self);
                        substitutor.substitute(alias_inner, &mapping)
                    };
                    return self.collect_union_variants_for_subtract(instantiated, depth + 1);
                }

                self.collect_union_variants_for_subtract(inner_ty, depth + 1)
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn as_nullable_union(&self, ty: TyId) -> Option<(TyId, TyId)> {
        let TyData::Union(elements) = self.data(ty) else {
            return None;
        };
        if elements.len() != 2 {
            return None;
        }
        let left = elements[0];
        let right = elements[1];
        if left == self.ty_null {
            return Some((right, left));
        }
        if right == self.ty_null {
            return Some((left, right));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tolk_resolver::file_index::SymbolId;

    #[test]
    fn test_builtin_type_ids_stable_order() {
        let interner = TypeInterner::new();

        assert_eq!(interner.ty_undefined, TyId(0));
        assert_eq!(interner.ty_unknown, TyId(1));
        assert!(matches!(
            interner.data(interner.ty_undefined),
            TyData::Undefined
        ));
        assert!(matches!(
            interner.data(interner.ty_unknown),
            TyData::Unknown
        ));
    }

    #[test]
    fn test_alias_nominal_equality() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let def_a = SymbolId {
            file_id: 1,
            local_id: 1,
        };
        let def_b = SymbolId {
            file_id: 1,
            local_id: 2,
        };

        let t_a = interner.type_alias(def_a, "A".into(), t_int);
        let t_b = interner.type_alias(def_b, "B".into(), t_int);

        // A and B both alias int, but they are nominally different
        assert!(!interner.equals(t_a, t_b));
        assert!(interner.equals(t_a, t_int));
        assert!(interner.equals(t_b, t_int));

        // A is assignable from int (because its underlying is int)
        assert!(interner.can_rhs_be_assigned(t_a, t_int));
        // B is NOT assignable from A without as (because they are different nominal aliases)
        assert!(!interner.can_rhs_be_assigned(t_a, t_b));
    }

    #[test]
    fn test_alias_transparency() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let def_a = SymbolId {
            file_id: 1,
            local_id: 1,
        };
        let def_b = SymbolId {
            file_id: 1,
            local_id: 2,
        };

        let t_a = interner.type_alias(def_a, "A".into(), t_int);
        let t_b = interner.type_alias(def_b, "B".into(), t_a); // B aliases A

        // B aliases A, so they should be equal
        assert!(interner.equals(t_b, t_a));
        assert!(interner.can_rhs_be_assigned(t_b, t_a));
        assert!(interner.can_rhs_be_assigned(t_a, t_b));
    }

    #[test]
    fn test_int_assignment() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_int8 = interner.int_n(8, false);
        let t_uint8 = interner.int_n(8, true);
        let t_coins = interner.ty_coins;

        assert!(interner.can_rhs_be_assigned(t_int, t_int8));
        assert!(interner.can_rhs_be_assigned(t_int, t_uint8));
        assert!(interner.can_rhs_be_assigned(t_int, t_coins));

        assert!(!interner.can_rhs_be_assigned(t_int8, t_uint8));
    }

    #[test]
    fn test_as_casting() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_int8 = interner.int_n(8, false);
        let t_slice = interner.ty_slice;
        let t_addr = interner.ty_address_internal;

        assert!(interner.can_be_casted_with_as_operator(t_int, t_int8));

        assert!(interner.can_be_casted_with_as_operator(t_slice, t_addr));
        assert!(interner.can_be_casted_with_as_operator(t_addr, t_slice));
    }

    #[test]
    fn test_stack_width() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_tuple2 = interner.tuple(vec![t_int, t_int]);
        let t_nullable_int = interner.union(vec![t_int, interner.ty_null]);
        let t_nullable_tuple2 = interner.union(vec![t_tuple2, interner.ty_null]);

        assert_eq!(interner.get_width_on_stack(t_int), 1);
        assert_eq!(interner.get_width_on_stack(t_tuple2), 2);
        assert_eq!(interner.get_width_on_stack(t_nullable_int), 1); // int? is optimized to 1 slot
        assert_eq!(interner.get_width_on_stack(t_nullable_tuple2), 3); // (int, int)? is 2 + 1 slots
    }

    #[test]
    fn test_generic_alias_equality() {
        let mut interner = TypeInterner::new();

        let def_w1 = SymbolId {
            file_id: 1,
            local_id: 1,
        };
        let def_w2 = SymbolId {
            file_id: 1,
            local_id: 2,
        };
        let t_int = interner.ty_int;

        // type Wrapper1<T> = T
        let t_w1_int =
            interner.type_alias_instantiation(def_w1, "Wrapper1".into(), t_int, vec![t_int]);
        // type Wrapper2<T> = T
        let t_w2_int =
            interner.type_alias_instantiation(def_w2, "Wrapper2".into(), t_int, vec![t_int]);

        assert!(interner.equals(t_w1_int, t_w2_int));
    }

    #[test]
    fn test_union_flattening_and_deduplication() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_slice = interner.ty_slice;
        let t_cell = interner.ty_cell;

        // basic dedup: int | int -> int
        let u1 = interner.union(vec![t_int, t_int]);
        assert_eq!(u1, t_int);

        // nominal alias dedup: type UserId = int; UserId | int -> UserId
        let def_user = SymbolId {
            file_id: 1,
            local_id: 1,
        };
        let t_user_id = interner.type_alias(def_user, "UserId".into(), t_int);
        let u2 = interner.union(vec![t_user_id, t_int]);
        assert_eq!(u2, t_user_id);

        // different nominal aliases dedup: UserId | OwnerId -> UserId
        let def_owner = SymbolId {
            file_id: 1,
            local_id: 2,
        };
        let t_owner_id = interner.type_alias(def_owner, "OwnerId".into(), t_int);
        let u3 = interner.union(vec![t_user_id, t_owner_id]);
        assert_eq!(u3, t_user_id);

        // nested union flattening: (int | slice) | (cell | int) -> int | slice | cell
        let u_int_slice = interner.union(vec![t_int, t_slice]);
        let u_cell_int = interner.union(vec![t_cell, t_int]);
        let u4 = interner.union(vec![u_int_slice, u_cell_int]);

        if let TyData::Union(variants) = interner.data(u4) {
            assert_eq!(variants.len(), 3);
            assert!(variants.contains(&t_int));
            assert!(variants.contains(&t_slice));
            assert!(variants.contains(&t_cell));
        } else {
            panic!("Expected union");
        }

        // flattening with aliases: type MyUnion = int | slice; MyUnion | cell -> int | slice | cell
        let def_mu = SymbolId {
            file_id: 1,
            local_id: 3,
        };
        let t_my_union = interner.type_alias(def_mu, "MyUnion".into(), u_int_slice);
        let u5 = interner.union(vec![t_my_union, t_cell]);
        if let TyData::Union(variants) = interner.data(u5) {
            assert_eq!(variants.len(), 3);
            assert!(variants.contains(&t_int));
            assert!(variants.contains(&t_slice));
            assert!(variants.contains(&t_cell));
        } else {
            panic!("Expected union");
        }
    }

    #[test]
    fn test_union_complex_structural_dedup() {
        let mut interner = TypeInterner::new();

        let def_base = SymbolId {
            file_id: 1,
            local_id: 1,
        };
        let t_int = interner.ty_int;

        // Box<int> instantiations
        let t_box_int1 = interner.struct_instantiation(
            SymbolId {
                file_id: 1,
                local_id: 2,
            },
            "Box".into(),
            def_base,
            vec![t_int],
        );

        // union(Box<int>, int) -> Box<int> | int (no dedup)
        let u2 = interner.union(vec![t_box_int1, t_int]);
        if let TyData::Union(variants) = interner.data(u2) {
            assert_eq!(variants.len(), 2);
        } else {
            panic!("Expected union");
        }

        // type Wrapper<T> = T; union(Wrapper<int>, int) -> Wrapper<int>
        let t_wrapper_int = interner.type_alias_instantiation(
            SymbolId {
                file_id: 1,
                local_id: 4,
            },
            "Wrapper".into(),
            t_int,
            vec![t_int],
        );
        let u3 = interner.union(vec![t_wrapper_int, t_int]);
        assert_eq!(u3, t_wrapper_int);
    }

    #[test]
    fn test_calculate_type_lca() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_null = interner.ty_null;
        let t_undefined = interner.ty_undefined;
        let t_never = interner.ty_never;

        assert_eq!(interner.calculate_type_lca(t_int, t_int), t_int);
        assert_eq!(interner.calculate_type_lca(t_int, t_undefined), t_undefined);
        assert_eq!(interner.calculate_type_lca(t_int, t_never), t_int);

        let t_int_nullable = interner.calculate_type_lca(t_int, t_null);
        assert!(interner.is_primitive_nullable(t_int_nullable));

        let t_tensor1 = interner.tensor(vec![t_int, t_null]);
        let t_tensor2 = interner.tensor(vec![t_null, t_int]);
        let t_lca_tensor = interner.calculate_type_lca(t_tensor1, t_tensor2);
        if let TyData::Tensor(items) = interner.data(t_lca_tensor) {
            assert_eq!(items.len(), 2);
            assert!(interner.is_primitive_nullable(items[0]));
            assert!(interner.is_primitive_nullable(items[1]));
        } else {
            panic!("Expected tensor");
        }

        let t_tensor3 = interner.tensor(vec![t_int]);
        let t_lca_diff_tensor = interner.calculate_type_lca(t_tensor1, t_tensor3);
        if let TyData::Union(variants) = interner.data(t_lca_diff_tensor) {
            assert_eq!(variants.len(), 2);
            assert!(variants.contains(&t_tensor1));
            assert!(variants.contains(&t_tensor3));
        } else {
            panic!("Expected union");
        }
    }

    #[test]
    fn test_calculate_type_subtract_rhs_type() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_null = interner.ty_null;
        let t_slice = interner.ty_slice;
        let t_builder = interner.ty_builder;
        let t_bool = interner.ty_bool;

        // int? - null = int
        let t_int_nullable = interner.union(vec![t_int, t_null]);
        assert_eq!(
            interner.calculate_type_subtract_rhs_type(t_int_nullable, t_null),
            t_int
        );

        // int | slice | builder | bool - bool | slice = int | builder
        let t_union_large = interner.union(vec![t_int, t_slice, t_builder, t_bool]);
        let t_union_sub = interner.union(vec![t_bool, t_slice]);
        let t_res = interner.calculate_type_subtract_rhs_type(t_union_large, t_union_sub);

        if let TyData::Union(variants) = interner.data(t_res) {
            assert_eq!(variants.len(), 2);
            assert!(variants.contains(&t_int));
            assert!(variants.contains(&t_builder));
        } else {
            panic!("Expected union, got {:?}", interner.data(t_res));
        }

        // non-union - anything = never
        assert_eq!(
            interner.calculate_type_subtract_rhs_type(t_int, t_null),
            interner.ty_never
        );

        // subtract all variants = never
        assert_eq!(
            interner.calculate_type_subtract_rhs_type(t_int_nullable, t_int_nullable),
            interner.ty_never
        );
    }
}
