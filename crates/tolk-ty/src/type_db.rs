use crate::type_interner::{TyId, TypeInterner};
use crate::types::{AddressKind, TyData};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;
use tolk_resolver::file_db::FileDb;
use tolk_resolver::file_index::{
    FileId, OptionalSyntaxNodeSpanExt, Span, Symbol, SymbolId, SymbolKind,
};
use tolk_resolver::project_index::ProjectIndex;
use tolk_resolver::resolve_index::{LocalDefKind, Resolved};
use tolk_syntax::{
    AstNode, FunCallableType, FunctionLike, HasGenericParams, HasName, Method, NullableType,
    TensorType, TupleType, Type, TypeIdent, TypeInstantiatedTs, UnionType, match_parents,
};
use tolk_syntax::{HasTreeSitterKind, ast};

/// Represents a field in a structure.
#[derive(Debug, Clone)]
pub struct StructField {
    pub id: SymbolId,
    pub name: Arc<str>,
    pub span: Span,
    /// The zero-based index of the field in the structure.
    pub field_idx: usize,
    /// The declared type of the field.
    pub declared_type: TyId,
}

/// Represents a member in an enumeration.
#[derive(Debug, Clone)]
pub struct EnumMember {
    pub id: SymbolId,
    pub name: Arc<str>,
    pub span: Span,
}

/// Describes the initial type inference context.
///
/// In it, we infer types for all top-level definitions. When inferring types for top-level
/// definitions, the main task is to convert the AST representation of a type into the
/// canonical [`TyId`] form, which is an interned version of the [`TyData`].
///
/// [`TypeInterner`] is used to intern all types during analysis and subsequent
/// type inference inside function bodies.
#[derive(Debug)]
pub struct TypeDb<'a> {
    /// Since types in the form of [`TyData`] store names, we need access to files and their source code
    /// to convert `Span` into text snippets. Also, since the resolver operates with symbols that
    /// only contain a `Span`, we need the AST of files to infer types for top-level definitions.
    pub file_db: &'a FileDb,
    /// All types in the analysis are interned via [`TypeInterner`].
    pub intrn: &'a mut TypeInterner,
    /// Mostly used to retrieve a specific file's index by its ID.
    /// It is also used to get information about resolved symbols by their [`Span`].
    pub project_index: &'a ProjectIndex,
    /// Stores all inferred types for top-level definitions by their [`SymbolId`].
    pub top_level_types: FxHashMap<SymbolId, TyId>,
    /// Stores all receiver types for methods by their [`SymbolId`].
    pub receiver_types: FxHashMap<SymbolId, TyId>,
    /// Stores call graph
    pub call_graph: FxHashMap<SymbolId, FxHashSet<SymbolId>>,
    pub inverted_call_graph: FxHashMap<SymbolId, FxHashSet<SymbolId>>,
    /// Caches types by the AST node ID that represents a [`Type`].
    type_lower_cache: FxHashMap<usize, TyId>,
    /// Keeps track of definitions that have already been processed or are currently
    /// being processed (to handle `A -> B -> A` dependencies).
    currently_lowering: FxHashSet<SymbolId>,
}

impl<'a> TypeDb<'a> {
    /// Creates a new `TypeDb` and initializes it by collecting top-level types.
    ///
    /// This performs an initial pass to register structs and enums so they are available globally.
    pub fn new(
        type_interner: &'a mut TypeInterner,
        file_db: &'a FileDb,
        project_index: &'a ProjectIndex,
    ) -> TypeDb<'a> {
        let mut db = TypeDb {
            intrn: type_interner,
            file_db,
            project_index,
            inverted_call_graph: FxHashMap::default(),
            call_graph: FxHashMap::default(),
            type_lower_cache: FxHashMap::default(),
            top_level_types: FxHashMap::default(),
            receiver_types: FxHashMap::default(),
            currently_lowering: FxHashSet::default(),
        };
        db.collect_top_level_types();
        db
    }

    /// Retrieves the type of top-level symbol (function, global, struct, etc.).
    ///
    /// If the type hasn't been inferred yet, this triggers signature inference for that symbol.
    pub fn get_top_level_type(
        &mut self,
        kind: Option<&SymbolKind>,
        symbol_id: SymbolId,
    ) -> Option<TyId> {
        if let Some(ty) = self.top_level_types.get(&symbol_id)
            && !matches!(kind, Some(SymbolKind::Struct { .. }) | None)
        {
            return Some(*ty);
        }

        if !self.currently_lowering.insert(symbol_id) {
            // Cycle detected or already lowering
            if let Some(&ty) = self.top_level_types.get(&symbol_id) {
                return Some(ty);
            }
            return None;
        }

        let res = self.infer_single_symbol_type(symbol_id);
        self.currently_lowering.remove(&symbol_id);
        res
    }

    pub fn is_struct_generic(&self, struct_id: SymbolId) -> bool {
        let symbol = self.project_index.resolve_symbol(struct_id);
        if let Some(symbol) = symbol
            && let SymbolKind::Struct { is_generic, .. } = symbol.kind
        {
            return is_generic;
        }
        false
    }

    pub fn find_struct(&self, struct_ty: TyId) -> Option<SymbolId> {
        let unwrapped = self.intrn.unwrap_alias(struct_ty);
        match self.intrn.data(unwrapped) {
            TyData::Struct { def, .. } => Some(*def),
            TyData::GenericTypeWithTs { inner_ty, .. } => {
                if let TyData::Struct { def, .. } = self.intrn.data(*inner_ty) {
                    Some(*def)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn find_struct_field(&self, struct_id: SymbolId, field_name: &str) -> Option<StructField> {
        let symbol = self.project_index.resolve_symbol(struct_id)?;
        if let SymbolKind::Struct { fields, .. } = &symbol.kind {
            for (idx, field) in fields.iter().enumerate() {
                if field.name.as_ref() == field_name {
                    let declared_type = self
                        .top_level_types
                        .get(&field.id)
                        .cloned()
                        .unwrap_or(self.intrn.ty_unknown);
                    return Some(StructField {
                        id: field.id,
                        name: field.name.clone(),
                        span: field.name_span,
                        field_idx: idx,
                        declared_type,
                    });
                }
            }
        }
        None
    }

    pub fn find_enum_member(&self, enum_id: SymbolId, member_name: &str) -> Option<EnumMember> {
        let symbol = self.project_index.resolve_symbol(enum_id)?;
        if let SymbolKind::Enum { members } = &symbol.kind {
            for member in members {
                if member.name.as_ref() == member_name {
                    return Some(EnumMember {
                        id: member.id,
                        name: member.name.clone(),
                        span: member.name_span,
                    });
                }
            }
        }
        None
    }

    fn collect_top_level_types(&mut self) {
        // Pass 1: Identity types (struct, enum) — globally available first
        for file_index in self.project_index.files().values() {
            for symbol in &file_index.decls {
                let Symbol { kind, name, id, .. } = symbol;
                let ty = match kind {
                    SymbolKind::Struct {
                        type_parameters, ..
                    } => {
                        let base_ty = self.intrn.struct_ty(*id, name.clone());
                        if type_parameters.is_empty() {
                            Some(base_ty)
                        } else {
                            let type_parameters = type_parameters
                                .iter()
                                .map(|p| self.intrn.type_parameter(p.to_string(), None))
                                .collect();
                            Some(self.intrn.generic_type_with_ts(base_ty, type_parameters))
                        }
                    }
                    SymbolKind::Enum { .. } => Some(self.intrn.enum_ty(*id, name.clone())),
                    // other symbols will be inferred later, we need all top level types for them, or whole type inference
                    _ => continue,
                };
                if let Some(ty) = ty {
                    self.top_level_types.insert(symbol.id, ty);
                }
            }
        }

        // Pass 2: Lower everything else (triggered by demand or by iterating)
        for file_index in self.project_index.files().values() {
            for symbol in &file_index.decls {
                let _ = self.get_top_level_type(Some(&symbol.kind), symbol.id);
            }
        }
    }

    fn infer_single_symbol_type(&mut self, symbol_id: SymbolId) -> Option<TyId> {
        let symbol = self.project_index.resolve_symbol(symbol_id)?;
        let file_id = symbol_id.file_id;

        // If it's a struct, we might need to lower its fields even if the struct type itself is already known
        if let SymbolKind::Struct { .. } = &symbol.kind {
            self.lower_struct_fields(file_id, symbol);
            return self.top_level_types.get(&symbol_id).cloned(); // struct type already set by `symbol_to_type`
        }

        // like `type int = builtin` from stdlib
        if matches!(
            &symbol.kind,
            SymbolKind::TypeAlias {
                is_builtin: true,
                ..
            }
        ) {
            let ty = self.as_primitive_type(&symbol.name)?;
            self.top_level_types.insert(symbol_id, ty);
            return Some(ty);
        }

        let file_index = self.project_index.get_file_index(file_id)?;
        let file_info = self.file_db.get_by_path(&file_index.path)?;

        let ast_decl = file_info.find_syntax_declaration(symbol.id)?;
        let ty = self.lower_top_level_decl(file_id, &ast_decl, symbol)?;
        self.top_level_types.insert(symbol_id, ty);
        Some(ty)
    }

    fn lower_struct_fields(&mut self, file_id: FileId, symbol: &Symbol) -> Option<()> {
        let SymbolKind::Struct { fields, .. } = &symbol.kind else {
            return None;
        };
        let file_info = self.file_db.get_by_id(file_id)?;

        let Some(ast::TopLevel::Struct(s)) = file_info.find_syntax_declaration(symbol.id) else {
            return None;
        };

        let body = s.body()?;

        let ast_fields = body.fields();
        for (field_info, ast_field) in fields.iter().zip(ast_fields) {
            if let Some(field_ty) = self.lower_opt_type(file_id, ast_field.typ().as_ref()) {
                self.top_level_types.insert(field_info.id, field_ty);
            }
        }

        Some(())
    }

    fn lower_top_level_decl(
        &mut self,
        file_id: FileId,
        decl: &ast::TopLevel,
        symbol: &Symbol,
    ) -> Option<TyId> {
        match decl {
            ast::TopLevel::Func(f) => {
                let return_ty = self
                    .lower_opt_type(file_id, f.return_type().as_ref())
                    .unwrap_or(self.intrn.ty_auto);
                let params = f
                    .parameters()
                    .map(|p| {
                        self.lower_opt_type(file_id, p.typ().as_ref())
                            .unwrap_or(self.intrn.ty_unknown)
                    })
                    .collect();
                Some(self.intrn.func(params, return_ty))
            }
            ast::TopLevel::Method(m) => {
                let return_ty = self
                    .lower_opt_type(file_id, m.return_type().as_ref())
                    .unwrap_or(self.intrn.ty_auto);
                let source = self
                    .file_db
                    .get_by_path(&self.project_index.files()[&file_id].path)?
                    .source()
                    .source
                    .clone();

                let is_instance_method = m.is_instance(source.as_ref());
                let mut params = m
                    .parameters_ext(source.as_ref(), true)
                    .map(|p| {
                        self.lower_opt_type(file_id, p.typ().as_ref())
                            .unwrap_or(self.intrn.ty_unknown)
                    })
                    .collect::<Vec<_>>();

                let receiver_ty = self
                    .lower_opt_type(file_id, m.receiver_type().as_ref())
                    .unwrap_or(self.intrn.ty_unknown);

                self.receiver_types.insert(symbol.id, receiver_ty);

                if is_instance_method {
                    params.insert(0, receiver_ty);
                    return Some(self.intrn.func(params, return_ty));
                }

                Some(self.intrn.func(params, return_ty))
            }
            ast::TopLevel::GetMethod(g) => {
                let return_ty = self
                    .lower_opt_type(file_id, g.return_type().as_ref())
                    .unwrap_or(self.intrn.ty_auto);
                let params = g
                    .parameters()
                    .map(|p| {
                        self.lower_opt_type(file_id, p.typ().as_ref())
                            .unwrap_or(self.intrn.ty_unknown)
                    })
                    .collect();
                Some(self.intrn.func(params, return_ty))
            }
            ast::TopLevel::GlobalVar(v) => self.lower_opt_type(file_id, v.typ().as_ref()),
            ast::TopLevel::Constant(c) => self.lower_opt_type(file_id, c.typ().as_ref()),
            ast::TopLevel::TypeAlias(a) => {
                let inner = match a.underlying_type() {
                    Some(ast::TypeAliasUnderlyingType::Type(t)) => self.lower_type(file_id, &t),
                    Some(ast::TypeAliasUnderlyingType::BuiltinSpecifier(_)) => {
                        // like `type int = builtin`
                        let name = self
                            .file_db
                            .text(file_id, decl.name().map(|n| n.syntax()).span())?;
                        self.as_primitive_type(&name)?
                    }
                    _ => self.intrn.ty_unknown,
                };

                let name = symbol.name.clone();
                let alias_ty = self.intrn.type_alias(symbol.id, name, inner);

                if let Some(type_parameters) = a.type_parameters() {
                    let type_params = type_parameters
                        .parameters()
                        .map(|p| {
                            let name = p
                                .name()
                                .and_then(|n| self.file_db.text_of(file_id, &n))
                                .unwrap_or_else(|| "unknown".into());
                            self.intrn.type_parameter(name.to_string(), None)
                        })
                        .collect::<Vec<_>>();

                    return Some(self.intrn.generic_type_with_ts(alias_ty, type_params));
                }

                Some(alias_ty)
            }
            _ => None,
        }
    }

    pub fn lower_type(&mut self, file_id: FileId, type_node: &Type) -> TyId {
        if let Some(cached) = self.type_lower_cache.get(&type_node.syntax().id()) {
            return *cached;
        }

        let ty = self
            .lower_type_impl(file_id, type_node)
            .unwrap_or(self.intrn.ty_unknown);
        self.type_lower_cache.insert(type_node.syntax().id(), ty);
        ty
    }

    pub fn lower_opt_type(&mut self, file_id: FileId, type_node: Option<&Type>) -> Option<TyId> {
        let ty = type_node?;
        Some(self.lower_type(file_id, ty))
    }

    fn lower_type_impl(&mut self, file_id: FileId, type_node: &Type) -> Option<TyId> {
        match type_node {
            Type::TypeIdent(type_ident) => self.convert_type_identifier(type_ident, file_id),
            Type::NullableType(nullable) => self.convert_nullable_type(nullable, file_id),
            Type::TensorType(tensor) => self.convert_tensor_type(tensor, file_id),
            Type::TupleType(tuple) => self.convert_tuple_type(tuple, file_id),
            Type::TypeInstantiatedTs(inst) => self.convert_instantiated_type(inst, file_id),
            Type::FunCallableType(func) => self.convert_function_type(func, file_id),
            Type::UnionType(union) => self.convert_union_type(union, file_id),
            Type::ParenthesizedType(paren) => self.lower_opt_type(file_id, paren.inner().as_ref()),
            Type::NullLit(_) => Some(self.intrn.ty_null),
            Type::Unmapped(_) => Some(self.intrn.ty_unknown),
        }
    }

    fn convert_type_identifier(&mut self, type_ident: &TypeIdent, file_id: FileId) -> Option<TyId> {
        match self.resolve_type_identifier(type_ident, file_id) {
            Some(result) => Some(result),
            None => {
                let text = self.file_db.text_of(file_id, type_ident)?;

                // for `self` we need to find receiver type of current method, if any
                // fun Foo.bar(self): self {}
                //     ^^^ this
                if text == "self" {
                    let syntax = type_ident.syntax();
                    let method = match_parents!(syntax, Method(...));
                    if let Some(method) = method {
                        return self.lower_opt_type(file_id, method.receiver_type().as_ref());
                    }
                }

                // fallback to text search for builtin types
                self.as_primitive_type(&text)
            }
        }
    }

    fn resolve_type_identifier(&mut self, type_ident: &TypeIdent, file_id: FileId) -> Option<TyId> {
        let Some(name_use) = self
            .project_index
            .find_use(file_id, type_ident.0.start_byte())
        else {
            let resolved_index = self.project_index.get_resolved_uses(file_id)?;
            let local = resolved_index.find_local_at(type_ident.0.start_byte())?;
            if matches!(local.kind, LocalDefKind::TypeParameter) {
                // TODO: default type
                return Some(self.intrn.type_parameter(local.name.to_string(), None));
            }
            return None;
        };
        let symbol_id = match name_use.resolved {
            Resolved::Global(global) => global,
            Resolved::Local(local) => {
                if let Some(resolved) = self.project_index.get_resolved_uses(file_id)
                    && let Some(resolved) = resolved.find_local(local)
                    && matches!(resolved.kind, LocalDefKind::TypeParameter)
                {
                    // TODO: default type
                    return Some(self.intrn.type_parameter(resolved.name.to_string(), None));
                }
                return None;
            }
            Resolved::Unresolved => return None,
        };
        self.get_top_level_type(None, symbol_id)
    }

    pub fn as_primitive_type(&mut self, name: &str) -> Option<TyId> {
        match name {
            "int" => Some(self.intrn.ty_int),
            "bool" => Some(self.intrn.ty_bool),
            "void" => Some(self.intrn.ty_void),
            "never" => Some(self.intrn.ty_never),
            "null" => Some(self.intrn.ty_null),
            "tuple" => Some(self.intrn.ty_untyped_tuple),
            "coins" => Some(self.intrn.ty_coins),
            "cell" => Some(self.intrn.ty_cell),
            "slice" => Some(self.intrn.ty_slice),
            "builder" => Some(self.intrn.ty_builder),
            "address" => Some(self.intrn.address(AddressKind::Internal)),
            "any_address" => Some(self.intrn.address(AddressKind::Any)),
            "continuation" => Some(self.intrn.ty_continuation),
            _ if name.starts_with("int") && name.len() > 3 => {
                if let Ok(size) = name[3..].parse::<usize>() {
                    Some(self.intrn.int_n(size, false))
                } else {
                    None
                }
            }
            _ if name.starts_with("uint") && name.len() > 4 => {
                if let Ok(size) = name[4..].parse::<usize>() {
                    Some(self.intrn.int_n(size, true))
                } else {
                    None
                }
            }
            _ if name.starts_with("varint") && name.len() > 6 => {
                if let Ok(size) = name[6..].parse::<usize>() {
                    Some(self.intrn.varint_n(size, false))
                } else {
                    None
                }
            }
            _ if name.starts_with("varuint") && name.len() > 7 => {
                if let Ok(size) = name[7..].parse::<usize>() {
                    Some(self.intrn.varint_n(size, true))
                } else {
                    None
                }
            }
            _ if name.starts_with("bits") && name.len() > 4 => {
                if let Ok(size) = name[4..].parse::<usize>() {
                    Some(self.intrn.bits(size))
                } else {
                    None
                }
            }
            _ if name.starts_with("bytes") && name.len() > 5 => {
                if let Ok(size) = name[5..].parse::<usize>() {
                    Some(self.intrn.bytes(size))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn convert_nullable_type(&mut self, nullable: &NullableType, file_id: FileId) -> Option<TyId> {
        let inner_ty = self.lower_type(file_id, &nullable.inner()?);
        Some(self.intrn.union(vec![inner_ty, self.intrn.ty_null]))
    }

    fn convert_tensor_type(&mut self, tensor: &TensorType, file_id: FileId) -> Option<TyId> {
        let els = tensor.elements();
        let tys = els.map(|t| self.lower_type(file_id, &t)).collect();
        Some(self.intrn.tensor(tys))
    }

    fn convert_tuple_type(&mut self, tuple: &TupleType, file_id: FileId) -> Option<TyId> {
        let els = tuple.elements();
        let tys = els.map(|t| self.lower_type(file_id, &t)).collect();
        Some(self.intrn.tuple(tys))
    }

    fn convert_instantiated_type(
        &mut self,
        inst: &TypeInstantiatedTs,
        file_id: FileId,
    ) -> Option<TyId> {
        let name_node = inst.name()?;
        let args_list = inst.arguments()?;

        let inner_ty = self.lower_type(file_id, &Type::TypeIdent(name_node));

        let types = args_list.types();
        let tys = types
            .map(|t| self.lower_type(file_id, &t))
            .collect::<Vec<_>>();

        let non_generic = tys.iter().all(|t| !self.intrn.has_generics(*t));

        let inner_data = self.intrn.data(inner_ty);
        if let TyData::GenericTypeWithTs { inner_ty, .. } = inner_data {
            if non_generic {
                if let TyData::Struct { def, name, .. } = self.intrn.data(*inner_ty) {
                    return Some(
                        self.intrn
                            .struct_instantiation(*def, name.clone(), *def, tys),
                    );
                }

                if let TyData::TypeAlias {
                    def,
                    name,
                    inner_ty,
                    ..
                } = self.intrn.data(*inner_ty)
                {
                    return Some(self.intrn.type_alias_instantiation(
                        *def,
                        name.clone(),
                        *inner_ty,
                        tys,
                    ));
                }
            }

            return Some(self.intrn.generic_type_with_ts(*inner_ty, tys));
        }
        if matches!(inner_data, TyData::Struct { .. } | TyData::TypeAlias { .. }) {
            return Some(inner_ty);
        }

        Some(self.intrn.generic_type_with_ts(inner_ty, tys))
    }

    fn convert_function_type(&mut self, func: &FunCallableType, file_id: FileId) -> Option<TyId> {
        let params_ty = self.lower_opt_type(file_id, func.param_types().as_ref())?;
        let return_ty = self
            .lower_opt_type(file_id, func.return_type().as_ref())
            .unwrap_or(self.intrn.ty_unknown);

        let params_ty_data = self.intrn.data(params_ty);
        let param_types = if let TyData::Tensor(tensor) = params_ty_data {
            // (int, slice) -> int
            tensor.clone()
        } else {
            // int -> int
            vec![params_ty]
        };

        Some(self.intrn.func(param_types, return_ty))
    }

    fn convert_union_type(&mut self, union: &UnionType, file_id: FileId) -> Option<TyId> {
        let lhs_ty = self.lower_opt_type(file_id, union.lhs().as_ref())?;
        let rhs_ty = self.lower_opt_type(file_id, union.rhs().as_ref())?;

        let rhs_types = if let Type::UnionType(rhs_union) = union.rhs().as_ref()? {
            self.convert_union_type(rhs_union, file_id)?
        } else {
            rhs_ty
        };

        let rhs_types_data = self.intrn.data(rhs_types);

        let types = match rhs_types_data {
            TyData::Union(elements) => {
                let mut all_types = vec![lhs_ty];
                all_types.extend(elements);
                all_types
            }
            _ => vec![lhs_ty, rhs_types],
        };

        Some(self.intrn.union(types))
    }
}
