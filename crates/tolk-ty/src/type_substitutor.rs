use crate::type_interner::{TyId, TypeInterner};
use crate::types::*;
use rustc_hash::FxHashMap;

pub(crate) struct TypeSubstitutor<'a> {
    interner: &'a mut TypeInterner,
    apply_defaults: bool,
}

impl<'a> TypeSubstitutor<'a> {
    pub(crate) const fn new(interner: &'a mut TypeInterner) -> Self {
        Self {
            interner,
            apply_defaults: false,
        }
    }

    pub(crate) const fn new_with_defaults(interner: &'a mut TypeInterner) -> Self {
        Self {
            interner,
            apply_defaults: true,
        }
    }

    pub(crate) fn substitute(&mut self, id: TyId, mapping: &FxHashMap<String, TyId>) -> TyId {
        let data = self.interner.data(id).clone();
        match data {
            TyData::TypeParameter {
                ref name,
                default_type,
            } => {
                if let Some(&new_id) = mapping.get(name) {
                    return new_id;
                }
                if self.apply_defaults
                    && let Some(default_ty) = default_type
                {
                    return self.substitute(default_ty, mapping);
                }
                id
            }
            TyData::TypeAlias {
                def,
                ref name,
                inner_ty: old_inner,
                ref args,
            } => {
                let inner_ty = self.substitute(old_inner, mapping);
                let mut new_args = Vec::new();
                let mut args_changed = false;
                if let Some(old_args) = args {
                    for &arg in old_args {
                        let new_arg = self.substitute(arg, mapping);
                        if new_arg != arg {
                            args_changed = true;
                        }
                        new_args.push(new_arg);
                    }
                }

                if inner_ty == old_inner && !args_changed {
                    return id;
                }
                self.interner.intern(TyData::TypeAlias {
                    def,
                    name: name.clone(),
                    inner_ty,
                    args: args.as_ref().map(|_| new_args),
                })
            }
            TyData::Tensor(old_elements) => {
                let mut changed = false;
                let mut elements = Vec::new();
                for &el in &old_elements {
                    let new_el = self.substitute(el, mapping);
                    if new_el != el {
                        changed = true;
                    }
                    elements.push(new_el);
                }
                if !changed {
                    return id;
                }
                self.interner.intern(TyData::Tensor(elements))
            }
            TyData::Tuple(old_elements) => {
                let mut changed = false;
                let mut elements = Vec::new();
                for &el in &old_elements {
                    let new_el = self.substitute(el, mapping);
                    if new_el != el {
                        changed = true;
                    }
                    elements.push(new_el);
                }
                if !changed {
                    return id;
                }
                self.interner.intern(TyData::Tuple(elements))
            }
            TyData::Array(old_element) => {
                let element_ty = self.substitute(old_element, mapping);
                if element_ty == old_element {
                    return id;
                }
                self.interner.array(element_ty)
            }
            TyData::Union(old_elements) => {
                let mut changed = false;
                let mut elements = Vec::new();
                for &el in &old_elements {
                    let new_el = self.substitute(el, mapping);
                    if new_el != el {
                        changed = true;
                    }
                    elements.push(new_el);
                }
                if !changed {
                    return id;
                }
                self.interner.union(elements)
            }
            TyData::Func {
                params: ref old_params,
                return_ty: old_return,
            } => {
                let mut changed = false;
                let mut params = Vec::new();
                for &p in old_params {
                    let new_p = self.substitute(p, mapping);
                    if new_p != p {
                        changed = true;
                    }
                    params.push(new_p);
                }
                let return_ty = self.substitute(old_return, mapping);
                if return_ty != old_return {
                    changed = true;
                }
                if !changed {
                    return id;
                }
                self.interner.intern(TyData::Func { params, return_ty })
            }
            TyData::GenericTypeWithTs {
                inner_ty: old_inner,
                types: ref old_types,
            } => {
                let inner_ty = self.substitute(old_inner, mapping);
                let mut changed = inner_ty != old_inner;
                let mut types = Vec::new();
                for &t in old_types {
                    let new_t = self.substitute(t, mapping);
                    if new_t != t {
                        changed = true;
                    }
                    types.push(new_t);
                }
                if !changed {
                    return id;
                }

                let non_generic = types.iter().all(|t| !self.interner.has_generics(*t));

                if non_generic {
                    match self.interner.data(inner_ty).clone() {
                        TyData::Struct { def, name, .. } => {
                            return self.interner.struct_instantiation(def, name, def, types);
                        }
                        TyData::TypeAlias {
                            def,
                            name,
                            inner_ty,
                            args,
                            ..
                        } => {
                            let mut instantiated_inner = inner_ty;
                            let mut alias_mapping = FxHashMap::default();

                            // For generic aliases represented as `GenericTypeWithTs(alias, [T1, T2, ...])`,
                            // map those original generic placeholders to instantiated `types`.
                            for (&param_ty, &actual_ty) in old_types.iter().zip(&types) {
                                if let TyData::TypeParameter { name, .. } =
                                    self.interner.data(param_ty)
                                {
                                    alias_mapping.insert(name.clone(), actual_ty);
                                }
                            }

                            // Fallback for aliases that keep formal args inside `TypeAlias.args`.
                            if let Some(alias_type_params) = args {
                                for (&param_ty, &actual_ty) in alias_type_params.iter().zip(&types)
                                {
                                    if let TyData::TypeParameter { name, .. } =
                                        self.interner.data(param_ty)
                                    {
                                        alias_mapping.insert(name.clone(), actual_ty);
                                    }
                                }
                            }

                            if !alias_mapping.is_empty() {
                                instantiated_inner =
                                    self.substitute(instantiated_inner, &alias_mapping);
                            }
                            return self.interner.type_alias_instantiation(
                                def,
                                name,
                                instantiated_inner,
                                types,
                            );
                        }
                        _ => {}
                    }
                }
                self.interner
                    .intern(TyData::GenericTypeWithTs { inner_ty, types })
            }
            TyData::Struct {
                def,
                ref name,
                base,
                ref args,
            } => {
                let mut new_args = Vec::new();
                let mut args_changed = false;
                if let Some(old_args) = args {
                    for &arg in old_args {
                        let new_arg = self.substitute(arg, mapping);
                        if new_arg != arg {
                            args_changed = true;
                        }
                        new_args.push(new_arg);
                    }
                }

                if !args_changed {
                    return id;
                }
                self.interner.intern(TyData::Struct {
                    def,
                    name: name.clone(),
                    base,
                    args: args.as_ref().map(|_| new_args),
                })
            }
            TyData::MapKV {
                key: old_key,
                value: old_value,
            } => {
                let key = self.substitute(old_key, mapping);
                let value = self.substitute(old_value, mapping);
                if key == old_key && value == old_value {
                    return id;
                }
                self.interner.intern(TyData::MapKV { key, value })
            }
            _ => id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_formatter::TypeFormatter;

    #[test]
    fn test_substitute_basic() {
        let mut interner = TypeInterner::new();
        let t_int = interner.ty_int;
        let t_param = interner.intern(TyData::TypeParameter {
            name: "T".to_string(),
            default_type: None,
        });

        let mut mapping = FxHashMap::default();
        mapping.insert("T".to_string(), t_int);

        let mut substitutor = TypeSubstitutor::new(&mut interner);
        let result = substitutor.substitute(t_param, &mapping);

        assert_eq!(result, t_int);
    }

    #[test]
    fn test_substitute_complex() {
        let mut interner = TypeInterner::new();
        let t_int = interner.ty_int;
        let t_bool = interner.ty_bool;

        let t_param_t = interner.intern(TyData::TypeParameter {
            name: "T".to_string(),
            default_type: None,
        });
        let t_param_u = interner.intern(TyData::TypeParameter {
            name: "U".to_string(),
            default_type: None,
        });

        // fun (T) -> U
        let t_func = interner.func(vec![t_param_t], t_param_u);

        let mut mapping = FxHashMap::default();
        mapping.insert("T".to_string(), t_int);
        mapping.insert("U".to_string(), t_bool);

        let mut substitutor = TypeSubstitutor::new(&mut interner);
        let result = substitutor.substitute(t_func, &mapping);

        let formatter = TypeFormatter::new(&interner);
        assert_eq!(formatter.format(result), "(int) -> bool");
    }

    #[test]
    fn test_substitute_no_change() {
        let mut interner = TypeInterner::new();
        let t_int = interner.ty_int;
        let t_tuple = interner.tuple(vec![t_int]);

        let mapping = FxHashMap::default();
        let mut substitutor = TypeSubstitutor::new(&mut interner);

        let result = substitutor.substitute(t_tuple, &mapping);
        assert_eq!(result, t_tuple); // Should return exactly the same TyId
    }

    #[test]
    fn test_substitute_nested() {
        let mut interner = TypeInterner::new();
        let t_int = interner.ty_int;
        let t_param = interner.intern(TyData::TypeParameter {
            name: "T".to_string(),
            default_type: None,
        });

        // [[T]]
        let t_inner_tuple = interner.tuple(vec![t_param]);
        let t_outer_tuple = interner.tuple(vec![t_inner_tuple]);

        let mut mapping = FxHashMap::default();
        mapping.insert("T".to_string(), t_int);

        let mut substitutor = TypeSubstitutor::new(&mut interner);
        let result = substitutor.substitute(t_outer_tuple, &mapping);

        let formatter = TypeFormatter::new(&interner);
        assert_eq!(formatter.format(result), "[[int]]");
    }
}
