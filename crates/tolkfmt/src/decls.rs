use crate::pretty::RcDoc;
use crate::{Context, comments, common, exprs, stmts, types};
use tolk_syntax::{
    Annotation, AnnotationArgs, AnnotationList, AsmBody, AstNode, Constant, Contract, ContractBody,
    ContractField, ContractFieldValue, Enum, EnumBody, EnumMember, Expr, Func, FuncBody,
    FunctionLike, GetMethod, GlobalVar, HasAnnotations, HasGenericParams, HasName, Ident, Import,
    LambdaParameter, Method, MethodReceiver, Parameter, SourceFile, Struct, StructBody,
    StructField, TolkRequiredVersion, TopLevel, Type, TypeAlias, TypeAliasUnderlyingType,
    TypeParameter, TypeParameters,
};
use tree_sitter::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ImportGroup {
    Stdlib,
    Acton,
    Other,
    Plain,
    RelativeCurrent,
    RelativeParent,
}

#[derive(Clone)]
struct ImportBlockItem<'tree> {
    import: Import<'tree>,
    group: ImportGroup,
    depth: usize,
    normalized_path: String,
    had_empty_line_after: bool,
}

fn strip_import_quotes(path: &str) -> &str {
    if path.len() >= 2 && path.starts_with('"') && path.ends_with('"') {
        &path[1..path.len() - 1]
    } else {
        path
    }
}

fn import_group(path: &str) -> ImportGroup {
    if path == "@stdlib" || path.strip_prefix("@stdlib/").is_some() {
        ImportGroup::Stdlib
    } else if path == "@acton" || path.strip_prefix("@acton/").is_some() {
        ImportGroup::Acton
    } else if path.starts_with('@') {
        ImportGroup::Other
    } else if !path.starts_with("./") && !path.starts_with("../") {
        ImportGroup::Plain
    } else if path.starts_with("./") {
        ImportGroup::RelativeCurrent
    } else if path.starts_with("../") {
        ImportGroup::RelativeParent
    } else {
        ImportGroup::RelativeCurrent
    }
}

fn import_depth(path: &str) -> usize {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .count()
}

fn build_import_items<'tree>(
    ctx: &Context<'tree>,
    imports: &[Import<'tree>],
) -> Vec<ImportBlockItem<'tree>> {
    imports
        .iter()
        .enumerate()
        .map(|(i, import)| {
            let path = import
                .path()
                .and_then(|p| p.0.utf8_text(ctx.code.as_ref().as_ref()).ok())
                .map(strip_import_quotes)
                .unwrap_or("");

            let had_empty_line_after = imports.get(i + 1).is_some_and(|next_import| {
                common::empty_lines_between(ctx, &import.0, &next_import.0) > 1
            });

            ImportBlockItem {
                import: *import,
                group: import_group(path),
                depth: import_depth(path),
                normalized_path: path.to_owned(),
                had_empty_line_after,
            }
        })
        .collect()
}

#[must_use]
pub fn print_source_file<'a>(ctx: &Context<'_>, file: &SourceFile) -> Option<RcDoc<'a>> {
    let mut sections = vec![];

    // tolk required version section
    let mut docs = vec![];

    // In theory, a file can have multiple Tolk versions, but we keep only one
    let required_version = file.top_levels().find_map(|decl| match decl {
        TopLevel::TolkRequiredVersion(decl) => Some(decl),
        _ => None,
    });

    // Tolk version is always printed at the beginning of the file, before imports
    if let Some(required_version) = required_version {
        let comments = ctx.comments.get(&required_version.0);
        comments::print_leading_comments(ctx, &mut docs, comments);

        let doc = print_tolk_required_version(ctx, &required_version);
        if let Some(doc) = doc {
            docs.push(doc);
        }

        comments::print_inline_comments(ctx, &mut docs, comments);
        docs.push(RcDoc::hardline());
        comments::print_trailing_comments(ctx, &mut docs, comments);
    }

    if !docs.is_empty() {
        sections.push(docs);
    }

    // imports section
    let mut docs = vec![];

    let imports = file
        .top_levels()
        .filter_map(|decl| match decl {
            TopLevel::Import(decl) => Some(decl),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut imports = build_import_items(ctx, &imports);
    imports.sort_by(|left, right| {
        left.group
            .cmp(&right.group)
            .then(left.depth.cmp(&right.depth))
            .then(left.normalized_path.cmp(&right.normalized_path))
            .then(left.import.0.start_byte().cmp(&right.import.0.start_byte()))
    });

    // After the optional Tolk version come imports.
    // Comments stay attached to their import during sorting.
    // Existing empty-line separators are preserved (normalized to one),
    // and optional group separators can be inserted by config.
    for (i, import) in imports.iter().enumerate() {
        let comments = ctx.comments.get(&import.import.0);
        comments::print_leading_comments(ctx, &mut docs, comments);

        let Some(doc) = print_import(ctx, &import.import) else {
            continue;
        };
        docs.push(doc);

        comments::print_inline_comments(ctx, &mut docs, comments);
        docs.push(RcDoc::hardline());
        comments::print_trailing_comments(ctx, &mut docs, comments);

        let add_group_separator = imports.get(i + 1).is_some_and(|next_import| {
            ctx.options.separate_import_groups && import.group != next_import.group
        });
        if import.had_empty_line_after || add_group_separator {
            docs.push(RcDoc::hardline());
        }
    }

    if !docs.is_empty() {
        sections.push(docs);
    }

    // declarations section
    let mut docs = vec![];

    let mut top_levels_iter = file
        .top_levels()
        .filter(|decl| {
            !matches!(
                decl,
                TopLevel::TolkRequiredVersion(_)
                    | TopLevel::Import(_)
                    | TopLevel::EmptyStmt(_)
                    | TopLevel::Unmapped(_)
            )
        })
        .peekable();

    while let Some(top_level) = top_levels_iter.next() {
        let node = top_level.syntax();
        let comments = ctx.comments.get(&node);

        if comments::has_fmt_ignore(ctx, comments) {
            docs.push(common::print_original_node_text(ctx, &node));
        } else {
            comments::print_leading_comments(ctx, &mut docs, comments);

            let Some(doc) = print_decl(ctx, &top_level) else {
                continue;
            };
            docs.push(doc);

            comments::print_inline_comments(ctx, &mut docs, comments);
            docs.push(RcDoc::hardline());
            comments::print_trailing_comments(ctx, &mut docs, comments);
        }

        // Add empty line between declarations if needed
        let next_decl = top_levels_iter.peek();
        if let Some(next_decl) = next_decl {
            let top_level_node = top_level.syntax();
            let next_top_level_node = next_decl.syntax();

            if next_top_level_node.kind() == top_level_node.kind()
                && next_top_level_node.kind() == "constant_declaration"
            {
                // don't add new line between constants if not requested
                if common::empty_lines_between(ctx, &top_level_node, &next_top_level_node) > 1 {
                    docs.push(RcDoc::hardline());
                }
            } else {
                docs.push(RcDoc::hardline());
            }
        }
    }

    if !docs.is_empty() {
        sections.push(docs);
    }

    Some(common::print_sections(sections))
}

#[must_use]
pub fn print_decl<'a>(ctx: &Context<'_>, decl: &TopLevel) -> Option<RcDoc<'a>> {
    match decl {
        TopLevel::TolkRequiredVersion(v) => print_tolk_required_version(ctx, v),
        TopLevel::Import(i) => print_import(ctx, i),
        TopLevel::Contract(c) => print_contract_declaration(ctx, c),
        TopLevel::GlobalVar(g) => print_global_var_declaration(ctx, g),
        TopLevel::Constant(constant) => print_constant_declaration(ctx, constant),
        TopLevel::TypeAlias(t) => print_type_alias_declaration(ctx, t),
        TopLevel::Struct(s) => print_struct_declaration(ctx, s),
        TopLevel::Enum(e) => print_enum_declaration(ctx, e),
        TopLevel::Func(func) => print_function(ctx, func),
        TopLevel::Method(m) => print_method_declaration(ctx, m),
        TopLevel::GetMethod(g) => print_get_method_declaration(ctx, g),
        TopLevel::EmptyStmt(_) => Some(RcDoc::text(";")),
        TopLevel::Unmapped(node) => common::print_node_text(ctx, &node.0),
    }
}

#[must_use]
pub fn print_tolk_required_version<'a>(
    ctx: &Context,
    v: &TolkRequiredVersion,
) -> Option<RcDoc<'a>> {
    let value = v.value()?;
    let value_doc = common::print_node_text(ctx, &value.0)?;
    Some(RcDoc::concat([RcDoc::text("tolk "), value_doc]))
}

#[must_use]
pub fn print_import<'a>(ctx: &Context<'_>, i: &Import) -> Option<RcDoc<'a>> {
    let path = i.path()?;
    let path_doc = common::print_node_text(ctx, &path.0)?;
    Some(RcDoc::concat([RcDoc::text("import "), path_doc]))
}

#[must_use]
pub fn print_contract_declaration<'a>(ctx: &Context<'_>, c: &Contract) -> Option<RcDoc<'a>> {
    let name = c.name()?;

    let mut parts = vec![RcDoc::text("contract "), exprs::print_ident(ctx, &name)?];
    if let Some(body) = c.body() {
        parts.push(RcDoc::space());
        parts.push(print_contract_body(ctx, &body)?);
    }
    Some(RcDoc::concat(parts))
}

pub fn print_contract_body<'a>(ctx: &Context<'_>, body: &ContractBody) -> Option<RcDoc<'a>> {
    let fields: Vec<_> = body.fields().collect();
    common::print_list(
        ctx,
        &fields,
        print_contract_field_declaration,
        |f| f.0,
        |_| collect_lonely_body_comments(body.0),
        common::ListOptions::curly_bracket_body(),
    )
}

#[must_use]
pub fn print_contract_field_declaration<'a>(
    ctx: &Context<'_>,
    f: &ContractField,
) -> Option<RcDoc<'a>> {
    let name = f.name()?;
    let value = f.value()?;

    let mut parts = vec![exprs::print_ident(ctx, &name)?, RcDoc::text(": ")];
    parts.push(match value {
        ContractFieldValue::Type(typ) => types::print_type(ctx, &typ)?,
        ContractFieldValue::Expr(expr) => exprs::print_expression(ctx, &expr)?,
    });
    Some(RcDoc::concat(parts))
}

#[must_use]
pub fn print_global_var_declaration<'a>(ctx: &Context, g: &GlobalVar) -> Option<RcDoc<'a>> {
    let name = g.name()?;
    let typ = g.typ()?;

    let mut parts = vec![];
    if let Some(annotations) = g.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("global "));
    parts.push(exprs::print_ident(ctx, &name)?);
    parts.push(RcDoc::text(": "));
    parts.push(types::print_type(ctx, &typ)?);

    Some(RcDoc::concat(parts))
}

#[must_use]
pub fn print_constant_declaration<'a>(ctx: &Context, constant: &Constant) -> Option<RcDoc<'a>> {
    let name = constant.name()?;

    let mut parts = vec![];
    if let Some(annotations) = constant.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }
    parts.push(RcDoc::text("const "));

    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(typ) = constant.typ() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &typ)?);
    }

    parts.push(RcDoc::space());

    if let Some(value) = constant.value() {
        parts.push(RcDoc::text("= "));
        parts.push(exprs::print_expression(ctx, &value)?);
    }

    Some(RcDoc::concat(parts))
}

#[must_use]
pub fn print_type_alias_declaration<'a>(ctx: &Context, t: &TypeAlias) -> Option<RcDoc<'a>> {
    let name = t.name()?;

    let mut parts = vec![];
    if let Some(annotations) = t.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }
    parts.push(RcDoc::text("type "));
    parts.push(exprs::print_ident(ctx, &name)?);
    if let Some(tp) = t.type_parameters() {
        parts.push(print_type_parameters(ctx, &tp)?);
    }

    let underlying = t.underlying_type()?;
    let is_union = matches!(
        underlying,
        TypeAliasUnderlyingType::Type(Type::UnionType(_))
    );

    parts.push(RcDoc::text(" ="));
    if is_union {
        parts.push(RcDoc::line());
    } else {
        parts.push(RcDoc::space());
    }

    parts.push(match underlying {
        TypeAliasUnderlyingType::Type(typ) => types::print_type(ctx, &typ)?,
        TypeAliasUnderlyingType::BuiltinSpecifier(_) => RcDoc::text("builtin"),
    });

    Some(RcDoc::group(RcDoc::concat(parts)))
}

#[must_use]
pub fn print_struct_declaration<'a>(ctx: &Context<'_>, s: &Struct) -> Option<RcDoc<'a>> {
    let name = s.name()?;

    let mut parts = vec![];
    if let Some(annotations) = s.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("struct "));
    if let Some(prefix) = s.pack_prefix() {
        parts.push(RcDoc::text("("));
        parts.push(common::print_node_text(ctx, &prefix.0)?);
        parts.push(RcDoc::text(") "));
    }

    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(tp) = s.type_parameters() {
        parts.push(print_type_parameters(ctx, &tp)?);
    }

    if let Some(body) = s.body() {
        parts.push(RcDoc::space());
        parts.push(print_struct_body(ctx, &body)?);
    }
    Some(RcDoc::concat(parts))
}

pub fn print_struct_body<'a>(ctx: &Context<'_>, body: &StructBody) -> Option<RcDoc<'a>> {
    let fields: Vec<_> = body.fields().collect();
    common::print_list(
        ctx,
        &fields,
        print_struct_field_declaration,
        |f| f.0,
        |_| collect_lonely_body_comments(body.0),
        common::ListOptions::curly_bracket_body(),
    )
}

#[must_use]
pub fn print_struct_field_declaration<'a>(ctx: &Context, f: &StructField) -> Option<RcDoc<'a>> {
    let name = f.name()?;
    let typ = f.typ()?;

    let mut parts = vec![];
    if let Some(modifiers) = f.modifiers() {
        for modifier in modifiers.modifiers() {
            parts.push(RcDoc::text(modifier.as_str()));
            parts.push(RcDoc::space());
        }
    }

    parts.push(exprs::print_ident(ctx, &name)?);
    parts.push(RcDoc::text(": "));
    parts.push(types::print_type(ctx, &typ)?);

    if let Some(default) = f.default() {
        parts.push(RcDoc::text(" = "));
        parts.push(exprs::print_expression(ctx, &default)?);
    }

    Some(RcDoc::concat(parts))
}

#[must_use]
pub fn print_enum_declaration<'a>(ctx: &Context<'_>, e: &Enum) -> Option<RcDoc<'a>> {
    let name = e.name()?;

    let mut parts = vec![];
    if let Some(annotations) = e.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("enum "));
    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(typ) = e.backed_type() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &typ)?);
    }

    if let Some(body) = e.body() {
        parts.push(RcDoc::space());
        parts.push(print_enum_body(ctx, &body)?);
    }
    Some(RcDoc::concat(parts))
}

pub fn print_enum_body<'a>(ctx: &Context<'_>, body: &EnumBody) -> Option<RcDoc<'a>> {
    let members: Vec<_> = body.members().collect();
    common::print_list(
        ctx,
        &members,
        print_enum_member_declaration,
        |m| m.0,
        |_| collect_lonely_body_comments(body.0),
        common::ListOptions::curly_bracket_body(),
    )
}

fn collect_lonely_body_comments(body: Node) -> Vec<Node> {
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .filter(|node| node.kind() == "comment")
        .collect()
}

#[must_use]
pub fn print_enum_member_declaration<'a>(ctx: &Context, m: &EnumMember) -> Option<RcDoc<'a>> {
    let mut parts = vec![];
    let name = m.name()?;
    parts.push(exprs::print_ident(ctx, &name)?);
    if let Some(default) = m.default() {
        parts.push(RcDoc::text(" = "));
        parts.push(exprs::print_expression(ctx, &default)?);
    }
    Some(RcDoc::concat(parts))
}

#[must_use]
pub fn print_function<'a>(ctx: &Context<'_>, func: &Func) -> Option<RcDoc<'a>> {
    let name = func.name()?;
    let parameters: Vec<_> = func.parameters().collect();
    let body = func.body()?;

    let mut parts = vec![];
    if let Some(annotations) = func.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("fun "));
    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(tp) = func.type_parameters() {
        parts.push(print_type_parameters(ctx, &tp)?);
    }

    parts.push(print_parameter_list(ctx, &parameters)?);

    if let Some(ret) = func.return_type() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &ret)?);
    }

    if should_print_inline_function_body(ctx, &body) {
        parts.push(RcDoc::space());
        parts.push(print_function_body(ctx, &body)?);
    } else {
        parts.push(RcDoc::concat([RcDoc::hardline(), print_function_body(ctx, &body)?]).nest(4));
    }

    Some(RcDoc::concat(parts))
}

#[must_use]
pub fn print_method_declaration<'a>(ctx: &Context<'_>, m: &Method) -> Option<RcDoc<'a>> {
    let name = m.name()?;
    let parameters: Vec<_> = m
        .parameters_ext(ctx.code.as_ref().as_ref(), false)
        .collect();
    let body = m.body()?;

    let mut parts = vec![];
    if let Some(annotations) = m.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("fun "));

    if let Some(receiver) = m.receiver() {
        parts.push(print_method_receiver(ctx, &receiver)?);
    }

    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(tp) = m.type_parameters() {
        parts.push(print_type_parameters(ctx, &tp)?);
    }

    parts.push(print_parameter_list(ctx, &parameters)?);

    if let Some(ret) = m.return_type() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &ret)?);
    }

    if should_print_inline_function_body(ctx, &body) {
        parts.push(RcDoc::space());
        parts.push(print_function_body(ctx, &body)?);
    } else {
        parts.push(RcDoc::concat([RcDoc::hardline(), print_function_body(ctx, &body)?]).nest(4));
    }

    Some(RcDoc::concat(parts))
}

#[must_use]
pub fn print_get_method_declaration<'a>(ctx: &Context, g: &GetMethod) -> Option<RcDoc<'a>> {
    let name = g.name()?;
    let parameters: Vec<_> = g.parameters().collect();
    let body = g.body()?;

    let mut parts = vec![];
    if let Some(annotations) = g.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("get fun "));
    parts.push(exprs::print_ident(ctx, &name)?);

    parts.push(print_parameter_list(ctx, &parameters)?);

    if let Some(ret) = g.return_type() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &ret)?);
    }

    if should_print_inline_function_body(ctx, &body) {
        parts.push(RcDoc::space());
        parts.push(print_function_body(ctx, &body)?);
    } else {
        parts.push(RcDoc::concat([RcDoc::hardline(), print_function_body(ctx, &body)?]).nest(4));
    }

    Some(RcDoc::concat(parts))
}

#[must_use]
pub fn print_method_receiver<'a>(ctx: &Context<'_>, r: &MethodReceiver) -> Option<RcDoc<'a>> {
    let typ = r.typ()?;
    let typ_doc = types::print_type(ctx, &typ)?;
    Some(RcDoc::concat([typ_doc, RcDoc::text(".")]))
}

fn should_print_inline_function_body(ctx: &Context<'_>, body: &FuncBody<'_>) -> bool {
    match body {
        FuncBody::Block(_) => true,
        FuncBody::AsmBody(asm) => is_single_triple_quoted_asm_body(ctx, asm),
        _ => false,
    }
}

fn is_single_triple_quoted_asm_body(ctx: &Context<'_>, asm: &AsmBody<'_>) -> bool {
    let mut instructions = asm.instructions();
    let Some(first) = instructions.next() else {
        return false;
    };
    if instructions.next().is_some() {
        return false;
    }

    first
        .0
        .utf8_text(ctx.code.as_ref().as_ref())
        .ok()
        .is_some_and(|text| {
            let trimmed = text.trim();
            trimmed.starts_with("\"\"\"") && trimmed.ends_with("\"\"\"")
        })
}

pub trait ParameterTrait {
    fn syntax<'tree>(&self) -> Node<'tree>
    where
        Self: 'tree;
    fn mutate(&self) -> bool;
    fn name<'tree>(&self) -> Option<Ident<'tree>>
    where
        Self: 'tree;
    fn typ<'tree>(&self) -> Option<Type<'tree>>
    where
        Self: 'tree;
    fn default<'tree>(&self) -> Option<Expr<'tree>>
    where
        Self: 'tree;
}

impl ParameterTrait for Parameter<'_> {
    fn syntax<'t>(&self) -> Node<'t>
    where
        Self: 't,
    {
        self.0
    }
    fn mutate(&self) -> bool {
        self.mutate()
    }
    fn name<'t>(&self) -> Option<Ident<'t>>
    where
        Self: 't,
    {
        self.0.field("name")
    }
    fn typ<'t>(&self) -> Option<Type<'t>>
    where
        Self: 't,
    {
        self.typ()
    }
    fn default<'t>(&self) -> Option<Expr<'t>>
    where
        Self: 't,
    {
        self.default()
    }
}

impl ParameterTrait for LambdaParameter<'_> {
    fn syntax<'t>(&self) -> Node<'t>
    where
        Self: 't,
    {
        self.0
    }
    fn mutate(&self) -> bool {
        self.mutate()
    }
    fn name<'t>(&self) -> Option<Ident<'t>>
    where
        Self: 't,
    {
        self.0.field("name")
    }
    fn typ<'t>(&self) -> Option<Type<'t>>
    where
        Self: 't,
    {
        self.typ()
    }
    fn default<'t>(&self) -> Option<Expr<'t>>
    where
        Self: 't,
    {
        None
    }
}

pub fn print_parameter_declaration<'a, 'tree, P>(
    ctx: &Context<'tree>,
    param: &P,
) -> Option<RcDoc<'a>>
where
    P: ParameterTrait + 'tree,
{
    let mut parts = vec![];
    if param.mutate() {
        parts.push(RcDoc::text("mutate "));
    }
    let name = param.name()?;
    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(typ) = param.typ() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &typ)?);
    }

    if let Some(default) = param.default() {
        parts.push(RcDoc::text(" = "));
        parts.push(exprs::print_expression(ctx, &default)?);
    }

    Some(RcDoc::concat(parts))
}

pub fn print_parameter_list<'a, 'tree, P>(ctx: &Context<'tree>, params: &[P]) -> Option<RcDoc<'a>>
where
    P: ParameterTrait + 'tree,
{
    common::print_list(
        ctx,
        params,
        print_parameter_declaration,
        P::syntax,
        |_| vec![],
        common::ListOptions::default(),
    )
}

#[must_use]
pub fn print_annotation_list<'a>(ctx: &Context<'_>, a: &AnnotationList) -> Option<RcDoc<'a>> {
    let annotations: Vec<_> = a.annotations().collect();
    let list_comments = ctx.comments.get(&a.0);

    let mut docs = vec![];
    comments::print_leading_comments(ctx, &mut docs, list_comments);

    for (i, annotation) in annotations.iter().enumerate() {
        let node = &annotation.0;
        let annotation_comments = ctx.comments.get(node);
        comments::print_leading_comments(ctx, &mut docs, annotation_comments);

        docs.push(print_annotation(ctx, annotation)?);

        comments::print_inline_comments(ctx, &mut docs, annotation_comments);
        if i + 1 == annotations.len() {
            comments::print_inline_comments(ctx, &mut docs, list_comments);
        }
        docs.push(RcDoc::hardline());
        comments::print_trailing_comments(ctx, &mut docs, annotation_comments);
        if i + 1 == annotations.len() {
            comments::print_trailing_comments(ctx, &mut docs, list_comments);
        }

        if let Some(next) = annotations.get(i + 1)
            && common::empty_lines_between(ctx, node, &next.0) > 1
        {
            docs.push(RcDoc::hardline());
        }
    }

    Some(RcDoc::concat(docs))
}

#[must_use]
pub fn print_annotation<'a>(ctx: &Context<'_>, a: &Annotation) -> Option<RcDoc<'a>> {
    let mut parts = vec![RcDoc::text("@")];
    if let Some(name) = a.name() {
        parts.push(exprs::print_ident(ctx, &name)?);
    }
    if let Some(args) = a.args() {
        let mut args_parts = vec![print_annotation_arguments(ctx, &args)?];
        let args_comments = ctx.comments.get(&args.0);
        comments::print_inline_comments(ctx, &mut args_parts, args_comments);
        parts.push(RcDoc::concat(args_parts));
    }
    Some(RcDoc::concat(parts))
}

pub fn print_annotation_arguments<'a>(ctx: &Context<'_>, a: &AnnotationArgs) -> Option<RcDoc<'a>> {
    let arguments: Vec<_> = a.args().collect();
    common::print_list(
        ctx,
        &arguments,
        exprs::print_expression,
        Expr::syntax,
        |_| vec![],
        common::ListOptions::default(),
    )
}

pub fn print_type_parameters<'a>(ctx: &Context<'_>, tp: &TypeParameters) -> Option<RcDoc<'a>> {
    let parameters: Vec<_> = tp.parameters().collect();
    common::print_list(
        ctx,
        &parameters,
        print_type_parameter,
        |p| p.0,
        |_| vec![],
        common::ListOptions::triangle_bracket_list(),
    )
}

#[must_use]
pub fn print_type_parameter<'a>(ctx: &Context<'_>, tp: &TypeParameter) -> Option<RcDoc<'a>> {
    let mut parts = vec![];
    let name = tp.name()?;
    parts.push(exprs::print_ident(ctx, &name)?);
    if let Some(default) = tp.default() {
        parts.push(RcDoc::text(" = "));
        parts.push(types::print_type(ctx, &default)?);
    }
    Some(RcDoc::concat(parts))
}

fn print_function_body<'a>(ctx: &Context<'_>, body: &FuncBody) -> Option<RcDoc<'a>> {
    match body {
        FuncBody::Block(block) => stmts::print_block_statement(ctx, block),
        FuncBody::AsmBody(asm) => print_asm_body(ctx, asm),
        FuncBody::BuiltinSpecifier(_) => Some(RcDoc::text("builtin")),
        FuncBody::Unmapped(node) => common::print_node_text(ctx, &node.0),
    }
}

#[must_use]
pub fn print_asm_body<'a>(ctx: &Context<'_>, asm: &AsmBody) -> Option<RcDoc<'a>> {
    let mut parts = vec![RcDoc::text("asm")];

    let params: Vec<_> = asm.params().collect();
    let returns: Vec<_> = asm.return_values().collect();

    if !params.is_empty() || !returns.is_empty() {
        parts.push(RcDoc::text("("));
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                parts.push(RcDoc::space());
            }
            parts.push(exprs::print_ident(ctx, p)?);
        }
        if !returns.is_empty() {
            parts.push(RcDoc::text(" -> "));
            for (i, r) in returns.iter().enumerate() {
                if i > 0 {
                    parts.push(RcDoc::space());
                }
                parts.push(common::print_node_text(ctx, &r.0)?);
            }
        }
        parts.push(RcDoc::text(")"));
    }

    let instructions: Vec<_> = asm.instructions().collect();
    let mut inst_docs = vec![];
    let keep_first_instruction_inline = instructions.len() == 1
        && instructions[0]
            .0
            .utf8_text(ctx.code.as_ref().as_ref())
            .ok()
            .is_some_and(|text| {
                let trimmed = text.trim();
                trimmed.starts_with("\"\"\"") && trimmed.ends_with("\"\"\"")
            });

    for (i, inst) in instructions.iter().enumerate() {
        let node = &inst.0;
        let comments = ctx.comments.get(node);

        if i == 0 {
            if keep_first_instruction_inline {
                inst_docs.push(RcDoc::space());
            } else {
                inst_docs.push(RcDoc::line());
            }
        }

        comments::print_leading_comments(ctx, &mut inst_docs, comments);

        inst_docs.push(common::print_node_text(ctx, node)?);

        comments::print_inline_comments(ctx, &mut inst_docs, comments);

        let is_last = i == instructions.len() - 1;
        if !is_last {
            inst_docs.push(RcDoc::line());
        }

        if let Some(c) = comments
            && c.iter()
                .any(|c| matches!(c.kind, comments::CommentKind::Trailing))
        {
            inst_docs.push(RcDoc::hardline());
            comments::print_trailing_comments(ctx, &mut inst_docs, comments);
        }

        if let Some(next) = instructions.get(i + 1)
            && common::empty_lines_between(ctx, node, &next.0) > 1
        {
            inst_docs.push(RcDoc::hardline());
        }
    }

    if inst_docs.is_empty() {
        return Some(RcDoc::concat(parts));
    }

    Some(RcDoc::group(RcDoc::concat([
        RcDoc::concat(parts),
        RcDoc::concat(inst_docs).nest(4),
    ])))
}
