use crate::type_interner::{TyId, TypeInterner};
use crate::type_substitutor::TypeSubstitutor;
use crate::types::TyData;
use rustc_hash::FxHashMap;

/// Stores the mapping of generic parameter names to their actual types.
#[derive(Debug, Clone, Default)]
pub(crate) struct GenericsSubstitutions {
    pub mapping: FxHashMap<String, TyId>,
}

impl GenericsSubstitutions {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set_type_t(&mut self, name: String, ty: TyId) {
        self.mapping.entry(name).or_insert(ty);
    }

    #[allow(dead_code)]
    pub(crate) fn get_substitution(&self, name: &str) -> Option<TyId> {
        self.mapping.get(name).copied()
    }
}

/// this struct helps to deduce Ts on the fly
/// purpose: having `f<T>(value: T)` and call `f(5)`, deduce T = int
/// while analyzing a call, arguments are handled one by one, by `auto_deduce_from_argument()`
/// note, that manually specified substitutions like `f<int>(5)` are NOT handled by this class, it's not deducing
pub(crate) struct GenericSubstitutionsDeducing {
    pub substitutions: GenericsSubstitutions,
}

impl Default for GenericSubstitutionsDeducing {
    fn default() -> Self {
        Self::new()
    }
}

impl GenericSubstitutionsDeducing {
    pub(crate) fn new() -> Self {
        Self {
            substitutions: GenericsSubstitutions::new(),
        }
    }

    /// purpose: having `f<T>(value: T)` and call `f(5)`, deduce T = int
    /// generally, there may be many generic Ts for declaration, and many arguments
    /// for every argument, `consider_next_condition()` is called
    /// example: `f<T1, T2>(a: int, b: T1, c: (T1, T2))` and call `f(6, 7, (8, cs))`
    /// - `a` does not affect, it doesn't depend on generic Ts
    /// - next condition: param_type = `T1`, arg_type = `int`, deduce T1 = int
    /// - next condition: param_type = `(T1, T2)` = `(int, T2)`, arg_type = `(int, slice)`, deduce T2 = slice
    pub(crate) fn consider_next_condition(
        &mut self,
        param_ty: TyId,
        arg_ty: TyId,
        interner: &mut TypeInterner,
    ) {
        // all Ts deduced up to this point are apriori
        let mut substitutor = TypeSubstitutor::new(interner);
        let param_ty = substitutor.substitute(param_ty, &self.substitutions.mapping);

        if !interner.has_generics(param_ty) {
            return;
        }

        match (
            interner.data(param_ty).clone(),
            interner.data(arg_ty).clone(),
        ) {
            (TyData::TypeParameter { name, .. }, _) => {
                // `(arg: T)` called as `f([1, 2])` => T is [int, int]
                self.substitutions.set_type_t(name, arg_ty);
            }
            (TyData::Union(p_variants), TyData::Union(a_variants)) => {
                // `arg: T1 | T2` called as `f(intOrBuilder)` => T1 is int, T2 is builder
                // `arg: int | T1` called as `f(builderOrIntOrSlice)` => T1 is builder|slice
                let mut a_sub_p = a_variants;
                let mut p_generic = Vec::new();
                let mut is_sub_correct = true;

                for &p_v in &p_variants {
                    if !interner.has_generics(p_v) {
                        if let Some(pos) = a_sub_p.iter().position(|&a_v| interner.equals(a_v, p_v))
                        {
                            a_sub_p.remove(pos);
                        } else {
                            is_sub_correct = false;
                        }
                    } else {
                        p_generic.push(p_v);
                    }
                }

                if is_sub_correct {
                    if p_generic.len() == 1 && a_sub_p.len() > 1 {
                        let a_union = interner.union(a_sub_p);
                        self.consider_next_condition(p_generic[0], a_union, interner);
                    } else if p_generic.len() == a_sub_p.len() {
                        for (p, a) in p_generic.into_iter().zip(a_sub_p.into_iter()) {
                            self.consider_next_condition(p, a, interner);
                        }
                    }
                }
            }
            (TyData::Union(p_variants), _) => {
                // `arg: int | MyData<T>` called as `f(MyData<int>)` => T is int
                for &p_v in &p_variants {
                    self.consider_next_condition(p_v, arg_ty, interner);
                }
            }
            (TyData::Tensor(p_items), TyData::Tensor(a_items))
            | (TyData::Tuple(p_items), TyData::Tuple(a_items)) => {
                // `arg: (int, T)` called as `f((5, cs))` => T is slice
                if p_items.len() == a_items.len() {
                    for (&p, &a) in p_items.iter().zip(a_items.iter()) {
                        self.consider_next_condition(p, a, interner);
                    }
                }
            }
            (
                TyData::Func {
                    params: p_params,
                    return_ty: p_ret,
                },
                TyData::Func {
                    params: a_params,
                    return_ty: a_ret,
                },
            ) => {
                // `arg: fun(TArg) -> TResult` called as `f(calcTupleLen)` => TArg is tuple, TResult is int
                if p_params.len() == a_params.len() {
                    for (&p, &a) in p_params.iter().zip(a_params.iter()) {
                        self.consider_next_condition(p, a, interner);
                    }
                    self.consider_next_condition(p_ret, a_ret, interner);
                }
            }
            (
                TyData::GenericTypeWithTs {
                    inner_ty: p_inner,
                    types: p_args,
                },
                _,
            ) => {
                // `arg: Wrapper<T>` called as `f(wrappedInt)` => T is int
                // In Rust version, we check if arg_unwrapped is also an instantiation or a struct/alias that is an instantiation
                let aaaa = interner.data(arg_ty).clone();
                match aaaa {
                    TyData::Struct {
                        def: a_def,
                        args: Some(a_args),
                        ..
                    } => {
                        if let TyData::Struct { def: p_def, .. } =
                            interner.data(interner.unwrap_alias(p_inner))
                            && *p_def == a_def
                            && p_args.len() == a_args.len()
                        {
                            for (&p, &a) in p_args.iter().zip(a_args.iter()) {
                                self.consider_next_condition(p, a, interner);
                            }
                        }
                    }
                    TyData::TypeAlias {
                        def: a_def,
                        args: Some(a_args),
                        ..
                    } => {
                        if let TyData::TypeAlias { def: p_def, .. } =
                            interner.data(interner.unwrap_alias(p_inner))
                            && *p_def == a_def
                            && p_args.len() == a_args.len()
                        {
                            for (&p, &a) in p_args.iter().zip(a_args.iter()) {
                                self.consider_next_condition(p, a, interner);
                            }
                        }
                    }
                    TyData::GenericTypeWithTs {
                        inner_ty: a_inner,
                        types: a_args,
                    } => {
                        if p_inner == a_inner && p_args.len() == a_args.len() {
                            for (&p, &a) in p_args.iter().zip(a_args.iter()) {
                                self.consider_next_condition(p, a, interner);
                            }
                        }
                    }
                    _ => {}
                }
            }
            (
                TyData::MapKV {
                    key: p_k,
                    value: p_v,
                },
                TyData::MapKV {
                    key: a_k,
                    value: a_v,
                },
            ) => {
                // `arg: map<K, V>` called as `f(someMapInt32Slice)` => K = int32, V = slice
                self.consider_next_condition(p_k, a_k, interner);
                self.consider_next_condition(p_v, a_v, interner);
            }
            _ => {}
        }
    }

    pub(crate) fn auto_deduce_from_argument(
        &mut self,
        param_ty: TyId,
        arg_ty: TyId,
        interner: &mut TypeInterner,
    ) -> TyId {
        self.consider_next_condition(param_ty, arg_ty, interner);
        self.replace_ts_with_currently_deduced(param_ty, interner)
    }

    pub(crate) fn replace_ts_with_currently_deduced(
        &self,
        ty: TyId,
        interner: &mut TypeInterner,
    ) -> TyId {
        let mut substitutor = TypeSubstitutor::new(interner);
        substitutor.substitute(ty, &self.substitutions.mapping)
    }

    #[allow(dead_code)]
    pub(crate) fn flush(self) -> GenericsSubstitutions {
        self.substitutions
    }
}
