use super::{TolkResolveSnapshot, TolkWorkspaceEngine, logical_path_for_uri};
use crate::{DocumentSnapshot, InlayHint, InlayHintKind, Position, Range};
use tolk_analysis::{
    ConstantEvaluationContext, ConstantEvaluator, ConstantValue, compute_get_method_id,
    is_simple_literal,
};
use tolk_resolver::resolve_index::{LocalDef, LocalDefKind};
use tolk_resolver::{AstNodeSpanExt, FileDb, FileId, ProjectIndex, Resolved, Span, SymbolKind};
use tolk_syntax::ast::expressions::{Call, Expr};
use tolk_syntax::{AstNode, FunctionLike, HasName, TopLevel, TryFromNode};
use tolk_ty::{InferenceResult, TyId, TypeInterner};

impl TolkWorkspaceEngine {
    pub(super) fn inlay_hints(&self, document: &DocumentSnapshot, range: Range) -> Vec<InlayHint> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        };
        let Some(snapshot) = snapshot else {
            return Vec::new();
        };
        let path = logical_path_for_uri(document.uri());
        let Some(file_id) = snapshot.project_index.get_file_by_path(&path) else {
            return Vec::new();
        };

        snapshot.inlay_hints(document, file_id, range)
    }
}

impl TolkResolveSnapshot {
    fn inlay_hints(
        &self,
        document: &DocumentSnapshot,
        file_id: FileId,
        range: Range,
    ) -> Vec<InlayHint> {
        let Some(file) = self.file_db.get_by_id(file_id) else {
            return Vec::new();
        };
        let mut builder = TolkInlayHintsBuilder::new(document, range);
        let root = file.source().tree.root_node();
        let source = file.source().source.as_ref();

        if let (Some(inferences), Some(resolve_index)) = (
            self.all_body_types.get(&file_id),
            self.project_index.get_resolved_uses(file_id),
        ) {
            for local in &resolve_index.locals {
                if !builder.intersects_span(local.def_span) {
                    continue;
                }
                let Some(symbol) = file.find_symbol_at(local.def_span.start()) else {
                    continue;
                };
                let Some(inference) = inferences.get(&symbol.id) else {
                    continue;
                };
                collect_local_hint(inference, &self.type_interner, local, &mut builder);
            }
        }

        if let Some(inferences) = self.all_body_types.get(&file_id) {
            for (&symbol_id, inference) in inferences {
                let Some(declaration) = file.find_syntax_declaration(symbol_id) else {
                    continue;
                };
                if !builder.intersects_span(declaration.span()) {
                    continue;
                }
                collect_return_type_hint(
                    inference,
                    &self.type_interner,
                    &declaration,
                    &mut builder,
                );
                collect_constant_hint(inference, &self.type_interner, &declaration, &mut builder);
            }
        }

        collect_parameter_hints(self, file_id, root, source, &mut builder);

        let mut evaluator = ConstantEvaluator::new(self);
        if let Some(file_index) = self.project_index.get_file_index(file_id) {
            for symbol in &file_index.decls {
                if !builder.intersects_span(symbol.body_span) {
                    continue;
                }
                let Some(declaration) = file.find_syntax_declaration(symbol.id) else {
                    continue;
                };
                match declaration {
                    TopLevel::Constant(constant) => collect_constant_value_hint(
                        &mut evaluator,
                        symbol.id,
                        constant,
                        &mut builder,
                    ),
                    TopLevel::Enum(enum_decl) => {
                        collect_enum_value_hints(&mut evaluator, symbol, enum_decl, &mut builder)
                    }
                    TopLevel::GetMethod(method) => {
                        collect_get_method_id_hint(method, &mut builder);
                    }
                    _ => {}
                }
            }
        }

        builder.build()
    }
}

impl ConstantEvaluationContext for TolkResolveSnapshot {
    fn file_db(&self) -> &FileDb {
        self.file_db.as_ref()
    }

    fn project_index(&self) -> &ProjectIndex {
        self.project_index.as_ref()
    }

    fn resolve_at(&self, file_id: FileId, span: Span) -> Option<Resolved> {
        TolkResolveSnapshot::resolved_at(self, file_id, span.start())
    }
}

struct TolkInlayHintsBuilder<'a> {
    document: &'a DocumentSnapshot,
    range: Range,
    start_offset: usize,
    end_offset: usize,
    hints: Vec<InlayHint>,
}

impl<'a> TolkInlayHintsBuilder<'a> {
    fn new(document: &'a DocumentSnapshot, range: Range) -> Self {
        let start_offset = document
            .text_index()
            .position_to_offset(document.text(), range.start);
        let end_offset = document
            .text_index()
            .position_to_offset(document.text(), range.end);
        Self {
            document,
            range,
            start_offset,
            end_offset,
            hints: Vec::new(),
        }
    }

    fn add_type_hint_at_span(&mut self, span: Span, typ: String) {
        self.add_type_hint(self.position_for_offset(span.end()), typ);
    }

    fn add_type_hint(&mut self, position: Position, typ: String) {
        self.add_hint(InlayHint::new(
            position,
            format!(": {typ}"),
            InlayHintKind::Type,
        ));
    }

    fn add_hint(&mut self, hint: InlayHint) {
        if self.range.start <= hint.position && hint.position <= self.range.end {
            self.hints.push(hint);
        }
    }

    fn position_for_offset(&self, offset: usize) -> Position {
        self.document
            .text_index()
            .offset_to_position(self.document.text(), offset)
    }

    const fn intersects_span(&self, span: Span) -> bool {
        span.start() <= self.end_offset && self.start_offset <= span.end()
    }

    fn intersects_node(&self, node: tree_sitter::Node<'_>) -> bool {
        node.start_byte() <= self.end_offset && self.start_offset <= node.end_byte()
    }

    fn build(mut self) -> Vec<InlayHint> {
        self.hints.sort_by(|left, right| {
            left.position
                .cmp(&right.position)
                .then(left.label.cmp(&right.label))
        });
        self.hints
    }
}

fn collect_local_hint(
    inference: &InferenceResult,
    interner: &TypeInterner,
    local: &LocalDef,
    builder: &mut TolkInlayHintsBuilder<'_>,
) {
    if matches!(local.kind, LocalDefKind::TypeParameter) || local.name.starts_with('_') {
        return;
    }
    if let LocalDefKind::Param {
        has_type, is_self, ..
    } = local.kind
        && (has_type || is_self)
    {
        return;
    }
    if let LocalDefKind::Var { has_type, .. } = local.kind
        && has_type
    {
        return;
    }
    let Some(ty_id) = inference.type_of(local.def_span) else {
        return;
    };
    if is_undefined_type(interner, ty_id) {
        return;
    }
    builder.add_type_hint_at_span(local.def_span, interner.format(ty_id));
}

fn collect_return_type_hint(
    inference: &InferenceResult,
    interner: &TypeInterner,
    declaration: &TopLevel<'_>,
    builder: &mut TolkInlayHintsBuilder<'_>,
) {
    let Some(return_ty) = inference.inferred_return_type else {
        return;
    };
    if is_undefined_type(interner, return_ty) {
        return;
    }

    match declaration {
        TopLevel::Func(function) if function.return_type().is_none() => {
            add_return_type_hint(function, return_ty, interner, builder);
        }
        TopLevel::Method(method) if method.return_type().is_none() => {
            add_return_type_hint(method, return_ty, interner, builder);
        }
        TopLevel::GetMethod(method) if method.return_type().is_none() => {
            add_return_type_hint(method, return_ty, interner, builder);
        }
        _ => {}
    }
}

fn add_return_type_hint<'tree, T: AstNode<'tree>>(
    node: &T,
    return_ty: TyId,
    interner: &TypeInterner,
    builder: &mut TolkInlayHintsBuilder<'_>,
) {
    let Some(parameters) = node.syntax().child_by_field_name("parameters") else {
        return;
    };
    builder.add_type_hint(
        builder.position_for_offset(parameters.end_byte()),
        interner.format(return_ty),
    );
}

fn collect_constant_hint(
    inference: &InferenceResult,
    interner: &TypeInterner,
    declaration: &TopLevel<'_>,
    builder: &mut TolkInlayHintsBuilder<'_>,
) {
    let TopLevel::Constant(constant) = declaration else {
        return;
    };
    if constant.typ().is_some() {
        return;
    }
    let Some(name) = constant.name() else {
        return;
    };
    let Some(expression) = constant.value() else {
        return;
    };
    if has_obvious_type(&expression, builder.document.text()) {
        return;
    }
    let Some(ty_id) = inference.type_of(expression.span()) else {
        return;
    };
    if is_undefined_type(interner, ty_id) {
        return;
    }
    builder.add_type_hint_at_span(name.span(), interner.format(ty_id));
}

fn has_obvious_type(expression: &Expr<'_>, source: &str) -> bool {
    match expression {
        Expr::ObjectLit(_) => true,
        Expr::Call(call) => call
            .callee_identifier()
            .is_some_and(|callee| matches!(callee.text(source), "fromCell" | "fromSlice")),
        Expr::Lazy(lazy) => lazy
            .expr()
            .is_some_and(|inner| has_obvious_type(&inner, source)),
        _ => false,
    }
}

fn is_undefined_type(interner: &TypeInterner, ty_id: TyId) -> bool {
    ty_id == interner.ty_undefined
}

fn collect_parameter_hints(
    snapshot: &TolkResolveSnapshot,
    file_id: FileId,
    root: tree_sitter::Node<'_>,
    source: &str,
    builder: &mut TolkInlayHintsBuilder<'_>,
) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if !builder.intersects_node(node) {
            continue;
        }
        if let Ok(call) = Call::try_from_node(node) {
            collect_call_parameter_hints(snapshot, file_id, call, source, builder);
        }

        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }
}

fn collect_call_parameter_hints(
    snapshot: &TolkResolveSnapshot,
    file_id: FileId,
    call: Call<'_>,
    source: &str,
    builder: &mut TolkInlayHintsBuilder<'_>,
) {
    let Some(callee) = call.callee_identifier() else {
        return;
    };
    let Some(Resolved::Global(symbol_id)) = snapshot.resolved_at(file_id, callee.start_byte())
    else {
        return;
    };
    let Some(symbol) = snapshot.project_index.resolve_symbol(symbol_id) else {
        return;
    };
    if matches!(
        symbol.name.as_ref(),
        "ton" | "println" | "address" | "send" | "expect"
    ) {
        return;
    }

    let (parameters, skip_self) = match &symbol.kind {
        SymbolKind::Function { parameters, .. } | SymbolKind::GetMethod { parameters, .. } => {
            (parameters.as_slice(), false)
        }
        SymbolKind::Method {
            parameters,
            is_instance,
            ..
        } => (
            parameters.as_slice(),
            *is_instance && call.callee_qualifier().is_some(),
        ),
        _ => return,
    };
    let parameters = parameters.iter().skip(usize::from(skip_self));

    for (parameter, argument) in parameters.zip(call.arguments()) {
        let name = parameter.name.as_ref();
        if name.chars().count() == 1 || name == "constString" {
            continue;
        }
        let argument_text = argument.text(source);
        if argument_text == name
            || argument_text
                .strip_suffix(name)
                .is_some_and(|prefix| prefix.ends_with('.'))
        {
            continue;
        }
        let Some(expression) = argument.expr() else {
            continue;
        };
        if matches!(expression, Expr::ObjectLit(object) if object.typ().is_some()) {
            continue;
        }
        if let Expr::Call(nested_call) = expression
            && nested_call
                .callee_identifier()
                .is_some_and(|callee| callee.text(source) == name)
        {
            continue;
        }

        builder.add_hint(InlayHint::new(
            builder.position_for_offset(argument.syntax().start_byte()),
            format!("{name}:"),
            InlayHintKind::Parameter,
        ));
    }
}

fn collect_constant_value_hint(
    evaluator: &mut ConstantEvaluator<'_>,
    symbol_id: tolk_resolver::SymbolId,
    constant: tolk_syntax::ast::Constant<'_>,
    builder: &mut TolkInlayHintsBuilder<'_>,
) {
    let Some(expression) = constant.value() else {
        return;
    };
    if is_simple_literal(&expression) {
        return;
    }
    let value = evaluator.evaluate_constant(symbol_id);
    if value.is_unknown() {
        return;
    }
    let formatted = value.format();
    let mut hint = InlayHint::plain(
        builder.position_for_offset(expression.syntax().end_byte()),
        format!(" /* = {formatted} */"),
    );
    hint.tooltip = Some(format!("Evaluated value: {formatted}"));
    builder.add_hint(hint);
}

fn collect_enum_value_hints(
    evaluator: &mut ConstantEvaluator<'_>,
    symbol: &tolk_resolver::Symbol,
    enum_decl: tolk_syntax::ast::Enum<'_>,
    builder: &mut TolkInlayHintsBuilder<'_>,
) {
    let SymbolKind::Enum { members } = &symbol.kind else {
        return;
    };
    let Some(body) = enum_decl.body() else {
        return;
    };
    let Some(values) = evaluator.evaluate_enum_values(symbol.id) else {
        return;
    };

    for (member, member_symbol) in body.members().zip(members) {
        let Some(ConstantValue::Int(value)) = values.get(&member_symbol.id) else {
            continue;
        };
        let value = value.to_string();
        let explicit_value = member.default();
        if explicit_value
            .is_some_and(|expression| expression.text(builder.document.text()) == value)
        {
            continue;
        }
        let Some(anchor) = explicit_value
            .map(|expression| expression.syntax())
            .or_else(|| member.name().map(|name| name.syntax()))
        else {
            continue;
        };
        let mut hint = InlayHint::new(
            builder.position_for_offset(anchor.end_byte()),
            format!(" = {value}"),
            InlayHintKind::Parameter,
        );
        hint.tooltip = Some(format!("Enum value: {value}"));
        builder.add_hint(hint);
    }
}

fn collect_get_method_id_hint(
    method: tolk_syntax::ast::GetMethod<'_>,
    builder: &mut TolkInlayHintsBuilder<'_>,
) {
    let source = builder.document.text();
    if method.has_method_id_annotation(source) {
        return;
    }
    let Some(name) = method.name() else {
        return;
    };
    let name = name.normalized_name(source);
    if name.starts_with("test ") || name.starts_with("test_") || name.starts_with("test-") {
        return;
    }

    let Some(get_keyword) = method.get_keyword() else {
        return;
    };
    builder.add_hint(InlayHint::new(
        builder.position_for_offset(get_keyword.end_byte()),
        format!("(0x{:x})", compute_get_method_id(name)),
        InlayHintKind::Type,
    ));
}
