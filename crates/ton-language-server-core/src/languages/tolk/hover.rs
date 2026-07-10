use super::{TolkResolveSnapshot, TolkWorkspaceEngine, logical_path_for_uri};
use crate::{DocumentSnapshot, Hover, Position, Range};
use tolk_analysis::{
    ConstantEvaluator, SerializationSizeContext, compute_get_method_id,
    estimate_serialization_size, is_simple_literal,
};
use tolk_resolver::resolve_index::{LocalDef, LocalDefKind};
use tolk_resolver::{FileDb, ProjectIndex, Resolved, Symbol, SymbolId, SymbolKind};
use tolk_syntax::{
    Annotation, Assert, AstNode, CatchClause, Contract, ContractField, EnumMember, FunctionLike,
    HasGenericParams, HasName, Import, NumberLit, Parameter, StringLit, StructField, Throw,
    TopLevel, TryFromNode, Type, TypeAliasUnderlyingType, TypeParameter, VarDecl,
    parse_tolk_int_literal,
};
use tolk_ty::{TyData, TyId, TypeInterner};

mod documentation;

impl TolkWorkspaceEngine {
    pub(super) fn hover(&self, document: &DocumentSnapshot, position: Position) -> Option<Hover> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        }?;
        let path = logical_path_for_uri(document.uri());
        let file_id = snapshot.project_index.get_file_by_path(&path)?;
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position);
        if let Some(hover) = snapshot.special_hover(file_id, offset) {
            return Some(hover);
        }
        let range = snapshot.hover_range(file_id, offset);
        let resolved = snapshot.resolved_at(file_id, offset)?;

        match resolved {
            Resolved::Global(symbol_id) => {
                let symbol = snapshot.project_index.resolve_symbol(symbol_id)?;
                let contents = snapshot.render_symbol_hover(symbol, file_id, offset)?;
                Some(Hover::new(contents, range))
            }
            Resolved::Local(local_id) => {
                let local = snapshot
                    .project_index
                    .get_resolved_uses(local_id.file_id)?
                    .find_local(local_id)?;
                let contents = snapshot.render_local_hover(local)?;
                Some(Hover::new(contents, range))
            }
            Resolved::Unresolved => None,
        }
    }
}

impl TolkResolveSnapshot {
    fn special_hover(&self, file_id: u32, offset: usize) -> Option<Hover> {
        let file = self.file_db.get_by_id(file_id)?;
        let source = file.source().source.as_ref();
        let mut node = file
            .source()
            .tree
            .root_node()
            .descendant_for_byte_range(offset, offset.saturating_add(1))?;

        loop {
            if let Ok(field) = ContractField::try_from_node(node)
                && field
                    .name()
                    .is_some_and(|name| contains_offset(name.syntax(), offset))
            {
                let contents = contract_field_hover(field, source)?;
                return Some(Hover::new(
                    contents,
                    self.range_for_node(file_id, field.name()?.syntax()),
                ));
            }
            if let Ok(contract) = Contract::try_from_node(node)
                && contract
                    .name()
                    .is_some_and(|name| contains_offset(name.syntax(), offset))
            {
                let contents = contract_hover(contract, source)?;
                return Some(Hover::new(
                    contents,
                    self.range_for_node(file_id, contract.name()?.syntax()),
                ));
            }
            if let Ok(string) = StringLit::try_from_node(node)
                && Import::try_from_node(string.syntax().parent()?).is_ok()
            {
                let import = self
                    .project_index
                    .imports_of(file_id)?
                    .into_iter()
                    .find(|import| import.import().span.contains(offset))?;
                let contents = format!("```tolk\nimport \"{}\"\n```", import.path().display());
                return Some(Hover::new(
                    contents,
                    self.range_for_node(file_id, string.syntax()),
                ));
            }
            if let Ok(number) = NumberLit::try_from_node(node)
                && let Some(contents) = exit_code_hover(number, source)
            {
                return Some(Hover::new(
                    contents,
                    self.range_for_node(file_id, number.syntax()),
                ));
            }
            if let Ok(annotation) = Annotation::try_from_node(node) {
                let name = annotation.name()?.text(source);
                let contents = documentation::annotation(name)?.to_owned();
                return Some(Hover::new(
                    contents,
                    self.range_for_node(file_id, annotation.name()?.syntax()),
                ));
            }
            node = node.parent()?;
        }
    }

    fn hover_range(&self, file_id: u32, offset: usize) -> Option<Range> {
        let span = self
            .project_index
            .find_use(file_id, offset)
            .map(|name_use| name_use.span)
            .or_else(|| {
                self.project_index
                    .find_symbol_at(file_id, offset)
                    .map(|symbol| symbol.name_span)
            })
            .or_else(|| {
                self.project_index
                    .get_resolved_uses(file_id)?
                    .find_local_at(offset)
                    .map(|local| local.def_span)
            })?;
        self.range_for_span(file_id, span)
    }

    fn render_symbol_hover(
        &self,
        symbol: &Symbol,
        usage_file_id: u32,
        usage_offset: usize,
    ) -> Option<String> {
        let file = self.file_db.get_by_id(symbol.id.file_id)?;
        let source = file.source().source.as_ref();
        if matches!(symbol.kind, SymbolKind::StructField) {
            return self.render_struct_field_hover(symbol, source);
        }
        if matches!(symbol.kind, SymbolKind::EnumMember) {
            return self.render_enum_member_hover(symbol, source);
        }

        let declaration = file.find_syntax_declaration(symbol.id)?;
        let usage_name = self.usage_name(usage_file_id, usage_offset);
        let signature = match declaration {
            TopLevel::Func(function) => self.function_signature(symbol, function, source, true),
            TopLevel::Method(function) => self.function_signature(symbol, function, source, true),
            TopLevel::GetMethod(function) => {
                self.function_signature(symbol, function, source, false)
            }
            TopLevel::Struct(structure) => struct_signature(structure, source)?,
            TopLevel::Enum(enumeration) => enum_signature(enumeration, source)?,
            TopLevel::TypeAlias(alias) => {
                let name = matches!(
                    symbol.kind,
                    SymbolKind::TypeAlias {
                        is_builtin: true,
                        ..
                    }
                )
                .then_some(usage_name.as_deref())
                .flatten();
                type_alias_signature(alias, source, name)?
            }
            TopLevel::Constant(constant) => {
                let name = constant.name()?.text(source);
                let typ = constant.typ().map_or_else(
                    || self.symbol_value_type(symbol),
                    |typ| typ.text(source).to_owned(),
                );
                let value = constant.value()?;
                let evaluation = if is_simple_literal(&value) {
                    String::new()
                } else {
                    let evaluated = ConstantEvaluator::new(self).evaluate_constant(symbol.id);
                    (!evaluated.is_unknown())
                        .then(|| format!(" // {}", evaluated.format()))
                        .unwrap_or_default()
                };
                format!("const {name}: {typ} = {}{evaluation}", value.text(source))
            }
            TopLevel::GlobalVar(variable) => {
                let name = variable.name()?.text(source);
                let typ = variable.typ()?.text(source);
                format!("global {name}: {typ}")
            }
            _ => declaration.syntax().text(source).trim().to_owned(),
        };
        let mut documentation = match declaration {
            TopLevel::GetMethod(method) => {
                let method_id = method.explicit_method_id(source).or_else(|| {
                    method
                        .name()
                        .map(|name| compute_get_method_id(name.normalized_name(source)))
                })?;
                let docs = documentation_before(declaration.syntax(), source);
                join_documentation(format!("Method ID: `0x{method_id:x}`"), docs)
            }
            _ => documentation_before(declaration.syntax(), source),
        };

        let supports_serialized_size = matches!(symbol.kind, SymbolKind::Struct { .. })
            || matches!(
                symbol.kind,
                SymbolKind::TypeAlias {
                    is_builtin: false,
                    ..
                }
            );
        if supports_serialized_size
            && let Some(size) = self
                .type_db_cache
                .top_level_type(symbol.id)
                .map(|ty| estimate_serialization_size(self, ty))
                .filter(|size| size.valid)
        {
            documentation = join_documentation(
                format!("**Size:** {}.\n\n---", size.presentation()),
                documentation,
            );
        }

        if matches!(
            symbol.kind,
            SymbolKind::TypeAlias {
                is_builtin: true,
                ..
            }
        ) && let Some(name) = usage_name
            && let Some(tlb_docs) = documentation::tlb_type(&name)
        {
            documentation = join_documentation(format!("\n{tlb_docs}"), documentation);
        }

        Some(render_hover(signature, documentation))
    }

    fn render_struct_field_hover(&self, symbol: &Symbol, source: &str) -> Option<String> {
        let file = self.file_db.get_by_id(symbol.id.file_id)?;
        let name = file
            .source()
            .tree
            .root_node()
            .descendant_for_byte_range(symbol.name_span.start(), symbol.name_span.end())?;
        let field = StructField::try_from_node(name.parent()?).ok()?;
        let owner = field.owner()?;
        let signature = format!(
            "struct {}\n{}",
            owner.name()?.text(source),
            struct_field_signature(field, source)?
        );
        let documentation = field_documentation(field, source);
        Some(render_hover(signature, documentation))
    }

    fn render_enum_member_hover(&self, symbol: &Symbol, source: &str) -> Option<String> {
        let file = self.file_db.get_by_id(symbol.id.file_id)?;
        let name = file
            .source()
            .tree
            .root_node()
            .descendant_for_byte_range(symbol.name_span.start(), symbol.name_span.end())?;
        let member = EnumMember::try_from_node(name.parent()?).ok()?;
        let owner = member.owner()?;
        let default = member
            .default()
            .map(|value| format!(" = {}", value.text(source)))
            .unwrap_or_default();
        let signature = format!(
            "enum {}\n{}{}",
            owner.name()?.text(source),
            member.name()?.text(source),
            default
        );
        Some(render_hover(signature, field_documentation(member, source)))
    }

    fn render_local_hover(&self, local: &LocalDef) -> Option<String> {
        let file = self.file_db.get_by_id(local.id.file_id)?;
        let source = file.source().source.as_ref();
        let node = file
            .source()
            .tree
            .root_node()
            .descendant_for_byte_range(local.def_span.start(), local.def_span.end())?;
        let ty = self
            .local_type(local)
            .map(|ty| self.type_interner.format(ty))
            .unwrap_or_else(|| "unknown".to_owned());

        let signature = match local.kind {
            LocalDefKind::Param { .. } => {
                let parameter = Parameter::try_from_node(node.parent()?).ok()?;
                parameter_signature(parameter, source, &ty)?
            }
            LocalDefKind::Var { .. } => {
                let variable = VarDecl::try_from_node(node.parent()?).ok()?;
                local_variable_signature(variable, source, &ty)?
            }
            LocalDefKind::Catch => {
                let _catch = ancestor_as::<CatchClause<'_>>(node)?;
                format!("catch ({})", local.name)
            }
            LocalDefKind::TypeParameter => {
                if let Ok(parameter) = TypeParameter::try_from_node(node.parent()?) {
                    type_parameter_signature(parameter, source)?
                } else {
                    implicit_type_parameter_signature(local, file.as_ref(), source)?
                }
            }
        };

        Some(render_hover(signature, String::new()))
    }

    fn usage_name(&self, file_id: u32, offset: usize) -> Option<String> {
        let file = self.file_db.get_by_id(file_id)?;
        let source = file.source().source.as_ref();
        let span = self
            .project_index
            .find_use(file_id, offset)
            .map(|name_use| name_use.span)
            .or_else(|| {
                self.project_index
                    .find_symbol_at(file_id, offset)
                    .map(|symbol| symbol.name_span)
            })?;

        source
            .get(span.start()..span.end())
            .map(|name| name.trim_matches('`').to_owned())
    }

    fn symbol_value_type(&self, symbol: &Symbol) -> String {
        self.type_db_cache
            .top_level_type(symbol.id)
            .map(|ty| match self.type_interner.data(ty) {
                TyData::Func { return_ty, .. } => self.type_interner.format(*return_ty),
                _ => self.type_interner.format(ty),
            })
            .unwrap_or_else(|| "unknown".to_owned())
    }

    fn function_signature<'tree, F>(
        &self,
        symbol: &Symbol,
        function: F,
        source: &str,
        infer_return_type: bool,
    ) -> String
    where
        F: FunctionLike<'tree> + AstNode<'tree>,
    {
        let syntax = function.syntax();
        let end = function
            .body()
            .map_or(syntax.end_byte(), |body| body.syntax().start_byte());
        let mut signature = source[syntax.start_byte()..end].trim().to_owned();

        if infer_return_type
            && function.return_type().is_none()
            && let Some(function_ty) = self.type_db_cache.top_level_type(symbol.id)
            && let TyData::Func { return_ty, .. } = self.type_interner.data(function_ty)
        {
            signature.push_str(": ");
            signature.push_str(&self.type_interner.format(*return_ty));
        }
        signature
    }

    fn range_for_span(&self, file_id: u32, span: tolk_resolver::Span) -> Option<Range> {
        let file = self.file_db.get_by_id(file_id)?;
        let source = file.source().source.as_ref();
        let index = crate::TextIndex::new(source);
        Some(index.range_for_offsets(source, span.start(), span.end()))
    }

    fn range_for_node(&self, file_id: u32, node: tree_sitter::Node<'_>) -> Option<Range> {
        self.range_for_span(file_id, tolk_resolver::Span::from_syntax(&node))
    }
}

impl SerializationSizeContext for TolkResolveSnapshot {
    fn file_db(&self) -> &FileDb {
        &self.file_db
    }

    fn project_index(&self) -> &ProjectIndex {
        &self.project_index
    }

    fn type_interner(&self) -> &TypeInterner {
        &self.type_interner
    }

    fn type_of_symbol(&self, symbol_id: SymbolId) -> Option<TyId> {
        self.type_db_cache.top_level_type(symbol_id)
    }
}

fn exit_code_hover(number: NumberLit<'_>, source: &str) -> Option<String> {
    let parent = number.syntax().parent()?;
    let is_exit_code = if let Ok(statement) = Throw::try_from_node(parent) {
        statement.expr()?.syntax() == number.syntax()
    } else if let Ok(statement) = Assert::try_from_node(parent) {
        statement.expr()?.syntax() == number.syntax()
    } else {
        false
    };
    if !is_exit_code {
        return None;
    }

    let value = parse_tolk_int_literal(number.text(source))?.parse_i32()?;
    let (origin, description) = exit_code_info(value)?;
    Some(format!(
        "{description}\n\n**Phase**: {origin}\n\nLearn more about exit codes in documentation: \
         https://docs.ton.org/v3/documentation/tvm/tvm-exit-codes"
    ))
}

fn exit_code_info(code: i32) -> Option<(&'static str, &'static str)> {
    Some(match code {
        0 => (
            "Compute and action phases",
            "Standard successful execution exit code.",
        ),
        1 => (
            "Compute phase",
            "Alternative successful execution exit code. Reserved, but doesn’t occur.",
        ),
        2 => ("Compute phase", "Stack underflow."),
        3 => ("Compute phase", "Stack overflow."),
        4 => ("Compute phase", "Integer overflow."),
        5 => (
            "Compute phase",
            "Range check error — some integer is out of its expected range.",
        ),
        6 => ("Compute phase", "Invalid TVM opcode."),
        7 => ("Compute phase", "Type check error."),
        8 => ("Compute phase", "Cell overflow."),
        9 => ("Compute phase", "Cell underflow."),
        10 => ("Compute phase", "Dictionary error."),
        11 => (
            "Compute phase",
            "Described in TVM docs as “Unknown error, may be thrown by user programs”.",
        ),
        12 => (
            "Compute phase",
            "Fatal error. Thrown by TVM in situations deemed impossible.",
        ),
        13 => ("Compute phase", "Out of gas error."),
        -14 => (
            "Compute phase",
            "Same as 13. Negative, so that it cannot be faked.",
        ),
        14 => (
            "Compute phase",
            "VM virtualization error. Reserved, but never thrown.",
        ),
        32 => ("Action phase", "Action list is invalid."),
        33 => ("Action phase", "Action list is too long."),
        34 => ("Action phase", "Action is invalid or not supported."),
        35 => (
            "Action phase",
            "Invalid source address in outbound message.",
        ),
        36 => (
            "Action phase",
            "Invalid destination address in outbound message.",
        ),
        37 => ("Action phase", "Not enough Toncoin."),
        38 => ("Action phase", "Not enough extra currencies."),
        39 => (
            "Action phase",
            "Outbound message does not fit into a cell after rewriting.",
        ),
        40 => (
            "Action phase",
            "Cannot process a message — not enough funds, the message is too large or its Merkle \
             depth is too big.",
        ),
        41 => (
            "Action phase",
            "Library reference is null during library change action.",
        ),
        42 => ("Action phase", "Library change action error."),
        43 => (
            "Action phase",
            "Exceeded maximum number of cells in the library or the maximum depth of the Merkle \
             tree.",
        ),
        50 => ("Action phase", "Account state size exceeded limits."),
        _ => return None,
    })
}

fn struct_signature(structure: tolk_syntax::Struct<'_>, source: &str) -> Option<String> {
    let prefix = structure
        .pack_prefix()
        .map(|prefix| format!("({}) ", prefix.text(source)))
        .unwrap_or_default();
    let name = structure.name()?.text(source);
    let type_parameters = structure
        .type_parameters()
        .map(|parameters| parameters.text(source))
        .unwrap_or_default();
    let fields = structure
        .body()
        .into_iter()
        .flat_map(|body| body.fields())
        .filter_map(|field| struct_field_signature(field, source))
        .map(|field| format!("    {field}"))
        .collect::<Vec<_>>();
    let body = if fields.is_empty() {
        "{}".to_owned()
    } else {
        format!("{{\n{}\n}}", fields.join("\n"))
    };
    Some(format!("struct {prefix}{name}{type_parameters} {body}"))
}

fn struct_field_signature(field: StructField<'_>, source: &str) -> Option<String> {
    let modifiers = field
        .modifiers()
        .map(|modifiers| format!("{} ", modifiers.text(source)))
        .unwrap_or_default();
    let name = field.name()?.text(source);
    let typ = field.typ()?.text(source);
    let default = field
        .default()
        .map(|value| format!(" = {}", value.text(source)))
        .unwrap_or_default();
    Some(format!("{modifiers}{name}: {typ}{default}"))
}

fn enum_signature(enumeration: tolk_syntax::Enum<'_>, source: &str) -> Option<String> {
    let name = enumeration.name()?.text(source);
    let backed_type = enumeration
        .backed_type()
        .map(|typ| format!(": {}", typ.text(source)))
        .unwrap_or_default();
    let members = enumeration
        .body()
        .into_iter()
        .flat_map(|body| body.members())
        .filter_map(|member| {
            let name = member.name()?.text(source);
            let default = member
                .default()
                .map(|value| format!(" = {}", value.text(source)))
                .unwrap_or_default();
            Some(format!("    {name}{default}"))
        })
        .collect::<Vec<_>>();
    let body = if members.is_empty() {
        "{}".to_owned()
    } else {
        format!("{{\n{}\n}}", members.join("\n"))
    };
    Some(format!("enum {name}{backed_type} {body}"))
}

fn type_alias_signature(
    alias: tolk_syntax::TypeAlias<'_>,
    source: &str,
    name: Option<&str>,
) -> Option<String> {
    let name = name.unwrap_or_else(|| alias.name().map_or("", |name| name.text(source)));
    let type_parameters = alias
        .type_parameters()
        .map(|parameters| parameters.text(source))
        .unwrap_or_default();
    match alias.underlying_type()? {
        TypeAliasUnderlyingType::Type(typ) => {
            if matches!(typ, Type::UnionType(_)) {
                let mut variants = Vec::new();
                collect_union_variants(typ, source, &mut variants);
                Some(format!(
                    "type {name}{type_parameters} =\n    | {}",
                    variants.join("\n    | ")
                ))
            } else {
                Some(format!(
                    "type {name}{type_parameters} = {}",
                    typ.text(source)
                ))
            }
        }
        TypeAliasUnderlyingType::BuiltinSpecifier(builtin) => Some(format!(
            "type {name}{type_parameters} = {}",
            builtin.text(source)
        )),
    }
}

fn collect_union_variants<'tree>(
    typ: Type<'tree>,
    source: &'tree str,
    output: &mut Vec<&'tree str>,
) {
    if let Type::UnionType(union) = typ {
        if let Some(lhs) = union.lhs() {
            collect_union_variants(lhs, source, output);
        }
        if let Some(rhs) = union.rhs() {
            collect_union_variants(rhs, source, output);
        }
    } else {
        output.push(typ.text(source).trim());
    }
}

fn render_hover(signature: String, documentation: String) -> String {
    if documentation.is_empty() {
        format!("```tolk\n{signature}\n```")
    } else {
        format!("```tolk\n{signature}\n```\n{documentation}")
    }
}

fn contract_hover(contract: Contract<'_>, source: &str) -> Option<String> {
    let name = contract.name()?.text(source);
    let fields = contract
        .body()
        .into_iter()
        .flat_map(|body| body.fields())
        .filter_map(|field| {
            Some(format!(
                "    {}: {}",
                field.name()?.text(source),
                field.value()?.text(source)
            ))
        })
        .collect::<Vec<_>>();
    let body = if fields.is_empty() {
        "{}".to_owned()
    } else {
        format!("{{\n{}\n}}", fields.join("\n"))
    };

    Some(render_hover(
        format!("contract {name} {body}"),
        documentation_before(contract.syntax(), source),
    ))
}

fn contract_field_hover(field: ContractField<'_>, source: &str) -> Option<String> {
    let name = field.name()?.text(source);
    let owner = field.owner()?.name()?.text(source);
    let standard = documentation::contract_field(name).unwrap_or_default();
    let custom = field_documentation(field, source);
    let documentation = join_documentation(standard.to_owned(), custom);

    Some(render_hover(
        format!("contract {owner}\n{name}"),
        documentation,
    ))
}

fn parameter_signature(parameter: Parameter<'_>, source: &str, inferred: &str) -> Option<String> {
    let mutable = parameter.mutate().then_some("mutate ").unwrap_or_default();
    let name = parameter.name()?.text(source);
    let typ = parameter.typ().map_or(inferred, |typ| typ.text(source));
    let default = parameter
        .default()
        .map(|value| format!(" = {}", value.text(source)))
        .unwrap_or_default();

    Some(format!("{mutable}{name}: {typ}{default}"))
}

fn local_variable_signature(variable: VarDecl<'_>, source: &str, typ: &str) -> Option<String> {
    let declaration = variable.declaration()?;
    let assignment = declaration.assignment()?;
    let Some(tolk_syntax::VarDeclPattern::VarDecl(single)) = declaration.pattern() else {
        return Some(assignment.syntax().text(source).to_owned());
    };
    if single.syntax() != variable.syntax() {
        return Some(assignment.syntax().text(source).to_owned());
    }

    let name = variable.name()?.text(source);
    let value = declaration
        .assigned_value()
        .map(|value| format!(" = {}", value.text(source)))
        .unwrap_or_default();

    Some(format!(
        "{} {name}: {typ}{value}",
        declaration.kind().as_str()
    ))
}

fn type_parameter_signature(parameter: TypeParameter<'_>, source: &str) -> Option<String> {
    let owner = type_parameter_owner_signature(parameter.owner()?, source)?;
    let name = parameter.name()?.text(source);
    let default = parameter
        .default()
        .map(|default| format!(" = {}", default.text(source)))
        .unwrap_or_default();

    Some(format!("{owner}\n{name}{default}"))
}

fn implicit_type_parameter_signature(
    local: &LocalDef,
    file: &tolk_resolver::FileInfo,
    source: &str,
) -> Option<String> {
    let symbol = file.find_symbol_at(local.def_span.start())?;
    let owner = file.find_syntax_declaration(symbol.id)?;
    let owner = type_parameter_owner_signature(owner, source)?;

    Some(format!("{owner}\n{}", local.name))
}

fn type_parameter_owner_signature(owner: TopLevel<'_>, source: &str) -> Option<String> {
    Some(match owner {
        TopLevel::Func(function) => format!("fun {}", function.name()?.text(source)),
        TopLevel::Method(function) => format!(
            "fun {}.{}",
            function.receiver_type()?.text(source),
            function.name()?.text(source),
        ),
        TopLevel::GetMethod(function) => format!("get fun {}", function.name()?.text(source)),
        TopLevel::Struct(structure) => format!("struct {}", structure.name()?.text(source)),
        TopLevel::TypeAlias(alias) => format!("type {}", alias.name()?.text(source)),
        _ => return None,
    })
}

fn ancestor_as<'tree, N>(mut node: tree_sitter::Node<'tree>) -> Option<N>
where
    N: TryFromNode<'tree>,
{
    loop {
        if let Ok(result) = N::try_from_node(node) {
            return Some(result);
        }
        node = node.parent()?;
    }
}

fn contains_offset(node: tree_sitter::Node<'_>, offset: usize) -> bool {
    node.start_byte() <= offset && offset < node.end_byte()
}

fn join_documentation(first: String, second: String) -> String {
    match (first.is_empty(), second.is_empty()) {
        (true, _) => second,
        (_, true) => first,
        (false, false) => format!("{first}\n\n{second}"),
    }
}

fn documentation_before(node: tree_sitter::Node<'_>, source: &str) -> String {
    let mut comments = Vec::new();
    let mut sibling = node.prev_named_sibling();
    let mut boundary = node.start_byte();
    while let Some(comment) = sibling {
        if comment.kind() != "comment" {
            break;
        }
        if comment.prev_named_sibling().is_some_and(|previous| {
            previous.kind() != "comment"
                && previous.end_position().row == comment.start_position().row
        }) {
            break;
        }
        let gap = &source[comment.end_byte()..boundary];
        if gap.matches('\n').count() > 1 {
            break;
        }
        comments.push(comment.text(source).to_owned());
        boundary = comment.start_byte();
        sibling = comment.prev_named_sibling();
    }
    comments.reverse();
    comments
        .into_iter()
        .map(|comment| clean_comment(&comment))
        .filter(|comment| !comment.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn clean_comment(comment: &str) -> String {
    if let Some(comment) = comment.strip_prefix("///") {
        return comment.trim_start().to_owned();
    }
    if let Some(comment) = comment.strip_prefix("//") {
        return comment.trim().to_owned();
    }
    if let Some(comment) = comment
        .strip_prefix("/*")
        .and_then(|comment| comment.strip_suffix("*/"))
    {
        return comment
            .lines()
            .map(|line| line.trim().trim_start_matches('*').trim_start())
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned();
    }
    String::new()
}

fn field_documentation<'tree, N>(node: N, source: &str) -> String
where
    N: AstNode<'tree>,
{
    let node = node.syntax();
    let preceding = documentation_before(node, source);
    if !preceding.is_empty() {
        return preceding;
    }

    let Some(comment) = node.next_named_sibling() else {
        return String::new();
    };
    if comment.kind() != "comment" || comment.start_position().row != node.end_position().row {
        return String::new();
    }

    clean_comment(comment.text(source))
}
