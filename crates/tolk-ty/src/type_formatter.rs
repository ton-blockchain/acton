use crate::type_interner::{TyId, TypeInterner};
use crate::types::*;
use std::sync::Arc;

pub(crate) struct TypeFormatter<'a> {
    interner: &'a TypeInterner,
}

impl<'a> TypeFormatter<'a> {
    pub(crate) const fn new(interner: &'a TypeInterner) -> Self {
        Self { interner }
    }

    pub(crate) fn format(&self, id: TyId) -> String {
        match self.interner.data(id) {
            TyData::Int(int_ty) => match int_ty {
                IntTy::Int => "int".to_string(),
                IntTy::IntN { size, unsigned } => {
                    format!("{}{}", if *unsigned { "uint" } else { "int" }, size)
                }
                IntTy::VarIntN { size, unsigned } => {
                    format!("{}{}", if *unsigned { "varuint" } else { "varint" }, size)
                }
                IntTy::Coins => "coins".to_string(),
            },
            TyData::Bool { .. } => "bool".to_string(),
            TyData::Cell => "cell".to_string(),
            TyData::Slice => "slice".to_string(),
            TyData::Builder => "builder".to_string(),
            TyData::Continuation => "continuation".to_string(),
            TyData::Address(kind) => match kind {
                AddressKind::Internal => "address".to_string(),
                AddressKind::Any => "any_address".to_string(),
            },
            TyData::MapKV { key, value } => {
                format!("map<{}, {}>", self.format(*key), self.format(*value))
            }
            TyData::Void => "void".to_string(),
            TyData::Null => "null".to_string(),
            TyData::Undefined => "undefined".to_string(),
            TyData::Unknown => "unknown".to_string(),
            TyData::Never => "never".to_string(),
            TyData::UntypedTuple => "tuple".to_string(),
            TyData::Bits { size } => format!("bits{}", size),
            TyData::Bytes { size } => format!("bytes{}", size),
            TyData::Builtin { name } => name.to_string(),
            TyData::Tuple(elements) => {
                let parts = elements.iter().map(|t| self.format(*t)).collect::<Vec<_>>();
                format!("[{}]", parts.join(", "))
            }
            TyData::Array(element_ty) => format!("array<{}>", self.format(*element_ty)),
            TyData::Tensor(elements) => {
                let parts = elements.iter().map(|t| self.format(*t)).collect::<Vec<_>>();
                format!("({})", parts.join(", "))
            }
            TyData::Func { params, return_ty } => {
                let ps = params.iter().map(|t| self.format(*t)).collect::<Vec<_>>();
                format!("({}) -> {}", ps.join(", "), self.format(*return_ty))
            }
            TyData::Union(elements) => {
                let null_like_count = elements
                    .iter()
                    .filter(|&&t| self.interner.equals(t, self.interner.ty_null))
                    .count();
                let non_null_like = elements
                    .iter()
                    .copied()
                    .filter(|&t| !self.interner.equals(t, self.interner.ty_null))
                    .collect::<Vec<_>>();

                if null_like_count == 1 && non_null_like.len() == 1 {
                    let inner = non_null_like[0];
                    let inner_text = self.format(inner);
                    let inner_unwrapped = self.interner.unwrap_alias(inner);
                    let needs_parens =
                        matches!(self.interner.data(inner_unwrapped), TyData::Func { .. });
                    if needs_parens {
                        return format!("({inner_text})?");
                    }
                    return format!("{inner_text}?");
                }

                let parts = elements
                    .iter()
                    .map(|t| {
                        if self.interner.equals(*t, self.interner.ty_null) {
                            return "null".to_string();
                        }
                        let text = self.format(*t);
                        let t = self.interner.unwrap_alias(*t);
                        if matches!(self.interner.data(t), TyData::Func { .. }) {
                            format!("({text})")
                        } else {
                            text
                        }
                    })
                    .collect::<Vec<_>>();
                parts.join(" | ")
            }
            TyData::Struct { name, args, .. } => {
                if let Some(value) = self.format_with_type_args(name, args) {
                    return value;
                }
                name.to_string()
            }
            TyData::TypeAlias {
                name,
                inner_ty,
                args,
                ..
            } => {
                if self.interner.equals(*inner_ty, self.interner.ty_null) {
                    return "null".to_string();
                }
                if self.interner.equals(*inner_ty, self.interner.ty_void) {
                    return "void".to_string();
                }
                if let Some(value) = self.format_with_type_args(name, args) {
                    return value;
                }
                name.to_string()
            }
            TyData::Enum { name, .. } => name.to_string(),
            TyData::TypeParameter { name, .. } => name.clone(),
            TyData::GenericTypeWithTs { inner_ty, types } => {
                let a = types
                    .iter()
                    .map(|t| self.format(*t))
                    .collect::<Vec<_>>()
                    .join(", ");
                let base = match self.interner.data(*inner_ty) {
                    TyData::Struct { name, .. } | TyData::TypeAlias { name, .. } => {
                        name.to_string()
                    }
                    _ => self.format(*inner_ty),
                };
                format!("{base}<{a}>")
            }
            TyData::Auto => "auto".to_string(),
        }
    }

    fn format_with_type_args(&self, name: &Arc<str>, args: &Option<Vec<TyId>>) -> Option<String> {
        if let Some(args) = args {
            let type_args = args
                .iter()
                .map(|arg| self.format(*arg))
                .collect::<Vec<_>>()
                .join(", ");
            return Some(format!("{name}<{type_args}>"));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tolk_resolver::file_index::SymbolId;

    #[test]
    fn test_format_basic() {
        let interner = TypeInterner::new();
        let formatter = TypeFormatter::new(&interner);

        assert_eq!(formatter.format(interner.ty_bool), "bool");
        assert_eq!(formatter.format(interner.ty_undefined), "undefined");
        assert_eq!(formatter.format(interner.ty_unknown), "unknown");
        assert_eq!(formatter.format(interner.ty_never), "never");
        assert_eq!(formatter.format(interner.ty_void), "void");
        assert_eq!(formatter.format(interner.ty_null), "null");
        assert_eq!(formatter.format(interner.ty_string), "string");
        assert_eq!(formatter.format(interner.ty_untyped_tuple), "tuple");
    }

    #[test]
    fn test_format_int() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_uint8 = interner.int_n(8, true);
        let t_varint16 = interner.varint_n(16, false);
        let t_coins = interner.ty_coins;

        let formatter = TypeFormatter::new(&interner);
        assert_eq!(formatter.format(t_int), "int");
        assert_eq!(formatter.format(t_uint8), "uint8");
        assert_eq!(formatter.format(t_varint16), "varint16");
        assert_eq!(formatter.format(t_coins), "coins");
    }

    #[test]
    fn test_format_complex() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;

        // [int, bool]
        let t_tuple = interner.tuple(vec![t_int, interner.ty_bool]);

        // (int, bool) -> void
        let t_func = interner.func(vec![t_int, interner.ty_bool], interner.ty_void);

        let dummy_id = SymbolId {
            file_id: 0,
            local_id: 0,
        };
        let t_struct = interner.struct_ty(dummy_id, "MyStruct".into());

        // MyStruct<int>
        let t_inst = interner.generic_type_with_ts(t_struct, vec![t_int]);

        // int | bool
        let t_union = interner.union(vec![t_int, interner.ty_bool]);
        let t_array = interner.array(t_int);

        let formatter = TypeFormatter::new(&interner);
        assert_eq!(formatter.format(t_tuple), "[int, bool]");
        assert_eq!(formatter.format(t_func), "(int, bool) -> void");
        assert_eq!(formatter.format(t_struct), "MyStruct");
        assert_eq!(formatter.format(t_inst), "MyStruct<int>");
        assert_eq!(formatter.format(t_array), "array<int>");
        assert_eq!(formatter.format(t_union), "int | bool");
    }
}
