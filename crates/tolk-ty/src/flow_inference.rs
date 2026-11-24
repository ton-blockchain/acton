use crate::generics_helpers::GenericSubstitutionsDeducing;
use crate::overload_resolution::MethodCallCandidate;
use crate::type_db::TypeDb;
use crate::type_interner::{TyId, TypeInterner};
use crate::type_substitutor::TypeSubstitutor;
use crate::types::TyData;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use std::collections::VecDeque;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use tolk_resolver::file_index::{AstNodeSpanExt, FileId, Span};
use tolk_resolver::resolve_index::{LocalDefId, NameUse};
use tolk_resolver::{Resolved, SymbolId};
use tolk_syntax::{AstNode, DotAccessField, Expr, TopLevel};

#[derive(Debug)]
pub(crate) struct ExprFlow {
    pub out_flow: FlowContext,

    // only calculated inside `if`, left of `&&`, etc. — there this expression is immediate condition, empty otherwise
    pub true_flow: FlowContext,
    pub false_flow: FlowContext,
}

impl ExprFlow {
    pub(crate) const fn new(
        out_flow: FlowContext,
        true_flow: FlowContext,
        false_flow: FlowContext,
    ) -> ExprFlow {
        ExprFlow {
            out_flow,
            true_flow,
            false_flow,
        }
    }

    pub(crate) fn create(out_flow: FlowContext, clone_flow_for_condition: bool) -> ExprFlow {
        let (true_flow, false_flow) = if clone_flow_for_condition {
            (out_flow.clone(), out_flow.clone())
        } else {
            (FlowContext::default(), FlowContext::default())
        };
        Self::new(out_flow, true_flow, false_flow)
    }
}

/// `UnreachableKind` is a reason of why control flow is unreachable or interrupted
/// example: `return;` interrupts control flow
/// example: `if (true) ... else ...` inside "else" flow is unreachable because it can't happen
pub(crate) enum UnreachableKind {
    #[allow(dead_code)]
    Unknown, // no definite info or not unreachable
    CantHappen,
    ThrowStatement,
    ReturnStatement,
    CallNeverReturnFunction,
    Break,
    Continue,
}

/// `FactsAboutExpr` represents "everything known about `SinkExpression` at a given execution point"
/// remember, that indices/fields are also expressions, `t.1 = 2` or `u.id = 2` also store such facts
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FactsAboutExpr {
    /// originally declared type or smart cast (Unknown if no info)
    pub expr_type: TyId,
}

impl FactsAboutExpr {
    pub(crate) const fn new(expr_type: TyId) -> FactsAboutExpr {
        FactsAboutExpr { expr_type }
    }
}

/// `FlowContext` represents "everything known about control flow at a given execution point"
/// while traversing AST, each statement node gets "in" `FlowContext` (prior knowledge)
/// and returns "output" `FlowContext` (representing a state AFTER execution of a statement)
/// on branching, like if/else, input context is cloned, two contexts for each branch calculated, and merged to a result
#[derive(Debug, Default)]
pub(crate) struct FlowContext {
    /// all local vars plus (optionally) indices/fields of tensors/tuples/objects
    known_facts: FxHashMap<SinkExpr, FactsAboutExpr>,
    /// if execution can't reach this point (after `return`, for example)
    unreachable: bool,
    uses: Vec<NameUse>,
}

impl Clone for FlowContext {
    fn clone(&self) -> Self {
        FlowContext::from(self)
    }
}

impl FlowContext {
    pub(crate) fn new() -> Self {
        Self {
            known_facts: FxHashMap::default(),
            uses: Vec::new(),
            unreachable: false,
        }
    }

    pub(crate) fn from(other: &FlowContext) -> FlowContext {
        Self {
            known_facts: other.known_facts.clone(),
            uses: other.uses.clone(),
            unreachable: other.unreachable,
        }
    }

    pub(crate) const fn is_unreachable(&self) -> bool {
        self.unreachable
    }

    pub(crate) fn equivalent_to(&self, other: &Self) -> bool {
        self.unreachable == other.unreachable && self.known_facts == other.known_facts
    }

    /// invalidate knowledge about sub-fields of a variable or its field
    /// example: `tensorVar = 2`, invalidate facts about `tensorVar`, `tensorVar.0`, `tensorVar.1.2`, and all others
    /// example: `user.id = rhs`, invalidate facts about `user.id` (sign, etc.) and `user.id.*` if exist
    fn invalidate_all_subfields(&mut self, def: LocalDefId, parent_path: u64, parent_mask: u64) {
        let mut new_facts = FxHashMap::default();

        for (sink, facts) in &self.known_facts {
            let should_remove = sink.def == def && (sink.index_path & parent_mask) == parent_path;
            if !should_remove {
                new_facts.insert(sink.clone(), facts.clone());
            }
        }

        self.known_facts = new_facts;
    }

    //+ CHECKED
    /// get the resulting type of variable or struct field
    pub(crate) fn smart_cast_or(&self, s_expr: SinkExpr, originally_declared_type: TyId) -> TyId {
        self.known_facts
            .get(&s_expr)
            .map_or(originally_declared_type, |facts| facts.expr_type)
    }

    /// Reads a field type through the smart casts of each parent in its path.
    fn get_effective_type(&self, expr: &SinkExpr, ctx: &mut InferenceContext) -> Option<TyId> {
        let mut remaining_path = expr.index_path;
        let mut parent = expr.clone();
        parent.index_path = 0;
        let mut current = self.smart_cast_or(parent.clone(), parent.get_declared_type(ctx)?);
        let mut shift = 0;

        while remaining_path != 0 {
            let index = (remaining_path & 0xff) as usize - 1;
            parent.index_path |= (remaining_path & 0xff) << shift;
            current = if let Some(facts) = self.known_facts.get(&parent) {
                facts.expr_type
            } else {
                get_child_type(current, index, ctx.type_db)?
            };
            remaining_path >>= 8;
            shift += 8;
        }
        Some(current)
    }

    /// Restores source-level types after branches rejoin without losing narrower casts.
    /// Parent casts from the entry flow take precedence over declared field types.
    pub(crate) fn reanchor_to(&mut self, before: &Self, ctx: &mut InferenceContext) {
        self.known_facts.retain(|expr, facts| {
            if let Some(type_before) = before.get_effective_type(expr, ctx)
                && ctx.type_db.intrn.equals(facts.expr_type, type_before)
            {
                if let Some(entry_facts) = before.known_facts.get(expr) {
                    facts.expr_type = entry_facts.expr_type;
                    return true;
                }
                return false;
            }

            if let Some(declared_type) = expr.get_declared_type(ctx)
                && ctx.type_db.intrn.equals(facts.expr_type, declared_type)
            {
                if expr.index_path == 0 {
                    facts.expr_type = declared_type;
                    return true;
                }
                return false;
            }
            true
        });
    }

    /// update current type of `local_var` / `tensorVar.0` / `obj.field`
    /// example: `local_var = rhs`
    /// example: `f(mutate obj.field)`
    /// example: `if (t.0 != null)`, in `true_flow` `t.0` assigned to "not-null of current", in `false_flow` to null
    pub(crate) fn register_known_type(&mut self, expr: SinkExpr, ty: TyId) {
        // having index_path = (some bytes filled in the end),
        // calc index_mask: replace every filled byte with 0xFF
        // example: `t.0.1`, index_path = (1<<8) + 2, index_mask = 0xFFFF
        let mut index_path = expr.index_path;
        let mut index_mask = 0u64;

        while index_path > 0 {
            index_mask = index_mask << 8 | 0xff;
            index_path >>= 8;
        }
        self.invalidate_all_subfields(expr.def, expr.index_path, index_mask);

        // if just `int` assigned, we have no considerations about its sign
        // so, even if something existed by the key s_expr, drop all knowledge
        // NOTE: we currently don't track sign in facts
        self.known_facts.insert(expr, FactsAboutExpr::new(ty));
    }

    /// mark control flow unreachable / interrupted
    pub(crate) const fn mark_unreachable(&mut self, reason: UnreachableKind) {
        self.unreachable = true;

        // currently we don't save why control flow became unreachable (it's not obvious how, there may be consequent reasons),
        // but it helps debugging and reading outer code
        let _ = reason;
    }

    /// "merge" two data-flow contexts occurs on control flow rejoins (if/else branches merging, for example)
    /// it's generating a new context that describes "knowledge that definitely outcomes from these two"
    /// example: in one branch x is `int`, in x is `null`, result is `int?` unless any of them is unreachable
    pub(crate) fn merge_flow(&self, c2: &FlowContext, int: &mut TypeInterner) -> FlowContext {
        if !self.unreachable && c2.unreachable {
            return c2.merge_flow(self, int);
        }

        let mut unified = FxHashMap::default();

        if self.unreachable && !c2.unreachable {
            // `if (...) return; else ...;` — copy facts about common variables only from else (c2)
            for (expr, i2) in &c2.known_facts {
                let it1 = self.known_facts.get(expr);
                let need_add = it1.is_some() || expr.index_path != 0;
                if need_add {
                    unified.insert(expr.clone(), i2.clone());
                }
            }
        } else {
            // either both reachable, or both not — merge types and restrictions of common variables and fields
            for (expr, i1) in &self.known_facts {
                if let Some(i2) = c2.known_facts.get(expr) {
                    if i1 == i2 {
                        unified.insert(expr.clone(), i1.clone());
                    } else {
                        unified.insert(
                            expr.clone(),
                            FactsAboutExpr::new(int.calculate_type_lca(i1.expr_type, i2.expr_type)),
                        );
                    }
                }
            }
        }

        FlowContext {
            known_facts: unified,
            uses: self.uses.clone(),
            unreachable: self.unreachable && c2.unreachable,
        }
    }
}

/// `SinkExpression` is an expression that can be smart cast like `if (x != null)` (x is int inside)
/// or analyzed by data flow is some other way like `if (x > 0) ... else ...` (x <= 0 inside else).
/// In other words, it "absorbs" data flow facts.
/// Examples: `localVar`, `localTensor.1`, `localTuple.1.2.3`, `localObj.field`
/// These are NOT sink expressions: `globalVar`, `f()`, `f().1`
/// Note, that globals are NOT sink: don't encourage to use a global twice, it costs gas, better assign it to a local.
#[derive(Debug, Clone, Eq)]
pub(crate) struct SinkExpr {
    /// smart casts and data flow applies only to locals
    pub def: LocalDefId,
    /// 0 for just `v`; for `v.N` it's (N+1), for `v.N.M` it's (N+1) + (M+1)<<8, etc.
    pub index_path: u64,
    /// Name of local for debug
    pub name: SmolStr,
}

impl PartialEq for SinkExpr {
    fn eq(&self, other: &Self) -> bool {
        self.def == other.def && self.index_path == other.index_path
    }
}

impl Hash for SinkExpr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.def.file_id);
        state.write_u32(self.def.local);
        state.write_u64(self.index_path);
    }
}

impl Display for SinkExpr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;

        let mut cur_path = self.index_path;
        while cur_path != 0 {
            write!(f, ".")?;
            let index = ((cur_path & 0xff) as i32 - 1).to_string();
            write!(f, "{index}")?;
            cur_path >>= 8;
        }

        Ok(())
    }
}

impl SinkExpr {
    pub(crate) const fn from_def(name: SmolStr, def: LocalDefId, index_path: u64) -> Self {
        Self {
            def,
            index_path,
            name,
        }
    }

    fn get_declared_type(&self, ctx: &mut InferenceContext) -> Option<TyId> {
        let local = ctx
            .type_db
            .project_index
            .get_resolved_uses(self.def.file_id)?
            .find_local(self.def)?;
        let mut current = ctx.get_type(local.def_span)?;
        let mut remaining_path = self.index_path;
        while remaining_path != 0 {
            let index = (remaining_path & 0xff) as usize - 1;
            current = get_child_type(current, index, ctx.type_db)?;
            remaining_path >>= 8;
        }
        Some(current)
    }
}

/// Looks up a declared field or tuple element, substituting generic struct arguments.
fn get_child_type(parent: TyId, index: usize, type_db: &mut TypeDb) -> Option<TyId> {
    let mut parent = type_db.intrn.unwrap_alias(parent);
    if let Some((inner, _)) = type_db.intrn.as_nullable_union(parent) {
        parent = type_db.intrn.unwrap_alias(inner);
    }
    if let TyData::Tensor(items) | TyData::Tuple(items) = type_db.intrn.data(parent) {
        return items.get(index).copied();
    }

    let def = type_db.find_struct(parent)?;
    let symbol = type_db.project_index.resolve_symbol(def)?;
    let tolk_resolver::SymbolKind::Struct { fields, .. } = &symbol.kind else {
        return None;
    };
    let field = type_db.find_struct_field(def, &fields.get(index)?.name)?;
    let mut field_type = field.declared_type;
    if type_db.intrn.has_generics(field_type) {
        let struct_type = type_db.get_top_level_type(None, def)?;
        let mut deducer = GenericSubstitutionsDeducing::new();
        deducer.auto_deduce_from_argument(struct_type, parent, type_db.intrn);
        field_type = TypeSubstitutor::new(type_db.intrn)
            .substitute(field_type, &deducer.substitutions.mapping);
    }
    Some(field_type)
}

/// Recovers the declaration behind a narrowed receiver, including instantiated fields
/// and method chains that return `self`. Inference and completion share this lookup.
fn declared_type_before_smart_cast(
    expr: Expr<'_>,
    file_id: FileId,
    expression_types: &FxHashMap<Span, TyId>,
    resolved_refs: &[NameUse],
    type_db: &mut TypeDb<'_>,
) -> Option<TyId> {
    let resolve = |span: Span| {
        resolved_refs
            .iter()
            .rev()
            .find(|usage| usage.span == span)
            .or_else(|| type_db.project_index.find_use(file_id, span.start()))
            .map(|usage| &usage.resolved)
    };
    match expr {
        Expr::Ident(ident) => {
            if let Some(Resolved::Local(local)) = resolve(ident.span())
                && let Some(local) = type_db
                    .project_index
                    .get_resolved_uses(local.file_id)
                    .and_then(|index| index.find_local(*local))
            {
                return expression_types.get(&local.def_span).copied();
            }
        }
        Expr::Call(call) => {
            let callee = match call.callee()? {
                Expr::Instantiation(instantiation) => instantiation.expr()?,
                callee => callee,
            };
            if let Expr::DotAccess(dot) = callee
                && let Some(DotAccessField::Ident(field)) = dot.field()
                && let Some(Resolved::Global(method_id)) = resolve(field.span())
                && let Some(file) = type_db.file_db.get_by_id(method_id.file_id)
                && let Some(TopLevel::Method(method)) = file.find_syntax_declaration(*method_id)
                && method.is_instance(file.source().source.as_ref())
                && let Some(return_type) = method.return_type()
                && file.text_at(return_type.span()) == "self"
                && let Some(obj) = dot.obj()
            {
                return declared_type_before_smart_cast(
                    obj,
                    file_id,
                    expression_types,
                    resolved_refs,
                    type_db,
                );
            }
        }
        Expr::DotAccess(dot) => {
            let obj_ty = *expression_types.get(&dot.obj()?.span())?;
            let obj_ty = type_db.intrn.unwrap_alias(obj_ty);
            match dot.field()? {
                DotAccessField::Ident(ident) => {
                    let name = type_db.file_db.text_of(file_id, &ident)?;
                    if let Some(def) = type_db.find_struct(obj_ty)
                        && let Some(field) = type_db.find_struct_field(def, name.trim_matches('`'))
                    {
                        return get_child_type(obj_ty, field.field_idx, type_db);
                    }
                }
                DotAccessField::NumericIndex(index) => {
                    let index = type_db
                        .file_db
                        .text_of(file_id, &index)?
                        .parse::<usize>()
                        .ok()?;
                    return get_child_type(obj_ty, index, type_db);
                }
            }
        }
        Expr::Paren(paren) => {
            return declared_type_before_smart_cast(
                paren.inner()?,
                file_id,
                expression_types,
                resolved_refs,
                type_db,
            );
        }
        _ => {}
    }
    expression_types.get(&expr.span()).copied()
}

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct MethodKey(pub TyId, pub SmolStr);

#[derive(Debug)]
pub struct InferenceContext<'db, 'a> {
    pub type_db: &'db mut TypeDb<'a>,
    pub file_id: FileId,
    pub self_type: Option<TyId>,
    pub declared_return_ty: Option<TyId>,
    pub expression_types: FxHashMap<Span, TyId>,
    pub resolved_refs: Vec<NameUse>,
    pub return_types: Vec<TyId>,
    pub inferred_return_type: Option<TyId>,
    pub decl_start: u32,
    pub caller_function: Option<SymbolId>,
    pub call_stack: VecDeque<SymbolId>,
    pub computed_methods: FxHashMap<MethodKey, Option<MethodCallCandidate>>,
}

impl<'db, 'a> InferenceContext<'db, 'a> {
    pub fn new(
        file_id: FileId,
        type_db: &'db mut TypeDb<'a>,
        call_stack: VecDeque<SymbolId>,
    ) -> InferenceContext<'db, 'a> {
        Self {
            type_db,
            file_id,
            self_type: None,
            declared_return_ty: None,
            expression_types: FxHashMap::default(),
            resolved_refs: Vec::new(),
            return_types: Vec::new(),
            inferred_return_type: None,
            decl_start: 0,
            caller_function: None,
            call_stack,
            computed_methods: FxHashMap::default(),
        }
    }

    pub(crate) fn declared_type_before_smart_cast(&mut self, expr: Expr<'_>) -> Option<TyId> {
        declared_type_before_smart_cast(
            expr,
            self.file_id,
            &self.expression_types,
            &self.resolved_refs,
            self.type_db,
        )
    }

    pub fn set_resolved(&mut self, use_: NameUse) {
        // Names are not always visited in source order (an object literal records a field
        // key after inferring its value), while lookups binary-search this list by span.
        let index = self
            .resolved_refs
            .partition_point(|existing| existing.span.start <= use_.span.start);
        self.resolved_refs.insert(index, use_);
    }

    pub fn get_resolved_node<'node, Node: AstNode<'node>>(&self, node: &Node) -> Option<&NameUse> {
        self.get_resolved(node.span())
    }

    pub fn get_resolved(&self, span: Span) -> Option<&NameUse> {
        let pos = span.start;
        if let Some(resolved) = self
            .resolved_refs
            .binary_search_by(|u| {
                if pos < u.span.start {
                    std::cmp::Ordering::Greater
                } else if pos >= u.span.end {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok()
            .map(|idx| &self.resolved_refs[idx])
        {
            return Some(resolved);
        }

        self.type_db
            .project_index
            .find_use(self.file_id, span.start())
    }

    pub fn text(&self, span: Span) -> Option<SmolStr> {
        self.type_db.file_db.text(self.file_id, span)
    }

    pub fn text_of<'node, Node: AstNode<'node>>(&self, node: &Node) -> Option<SmolStr> {
        self.type_db.file_db.text_of(self.file_id, node)
    }

    pub fn text_matches<'node, Node: AstNode<'node>>(&self, node: &Node, text: &str) -> bool {
        if node.span().len() != text.len() {
            // fast path
            return false;
        }
        let node_text = self.type_db.file_db.text_of(self.file_id, node);
        node_text.as_deref() == Some(text)
    }

    pub fn set_type(&mut self, span: Span, ty: TyId) {
        self.expression_types.insert(span, ty);
    }

    pub fn set_top_level_type(&mut self, symbol_id: SymbolId, ty: TyId) {
        self.type_db.top_level_types.to_mut().insert(symbol_id, ty);
    }

    pub fn get_top_level_type(&mut self, symbol_id: SymbolId) -> Option<TyId> {
        if let Some(&ty) = self.type_db.top_level_types.get(&symbol_id) {
            // fast path
            return Some(ty);
        }

        let kind = self
            .type_db
            .project_index
            .resolve_symbol(symbol_id)
            .map(|s| &s.kind);
        self.type_db.get_top_level_type(kind, symbol_id)
    }

    pub fn set_node_type<'node, Node: AstNode<'node>>(&mut self, node: &Node, ty: TyId) {
        self.set_type(node.syntax().span(), ty);
    }

    pub fn get_node_type<'node, Node: AstNode<'node>>(&self, node: &Node) -> Option<TyId> {
        self.get_type(node.syntax().span())
    }

    pub fn get_node_type_data<'node, Node: AstNode<'node>>(&self, node: &Node) -> Option<&TyData> {
        self.get_type(node.syntax().span())
            .map(|ty| self.type_db.intrn.data(ty))
    }

    pub fn get_node_type_or_unknown<'node, Node: AstNode<'node>>(&self, node: &Node) -> TyId {
        self.get_type(node.syntax().span())
            .unwrap_or(self.type_db.intrn.ty_undefined)
    }

    pub fn get_type(&self, span: Span) -> Option<TyId> {
        self.expression_types.get(&span).copied()
    }
}

/// Represents the result of type inference for a declaration.
///
/// This structure contains all inferred types for expressions and resolved symbol references.
/// It is the main output of the `infer` function.
#[derive(Debug, Clone)]
pub struct InferenceResult {
    /// Map from AST node spans to their inferred types.
    pub expression_types: FxHashMap<Span, TyId>,
    /// List of resolved references (fields, methods).
    pub resolved_refs: Vec<NameUse>,
    /// The inferred return type of the function (if inference was run on a function).
    pub inferred_return_type: Option<TyId>,
}

impl InferenceResult {
    #[must_use]
    pub fn new(ctx: InferenceContext) -> Self {
        Self {
            expression_types: ctx.expression_types,
            resolved_refs: ctx.resolved_refs,
            inferred_return_type: ctx.inferred_return_type,
        }
    }

    /// Retrieves the type of expression at the given span.
    #[must_use]
    pub fn type_of(&self, span: Span) -> Option<TyId> {
        self.expression_types.get(&span).copied()
    }

    /// Retrieves the declared receiver type after smart casts for method lookup.
    /// `expr` must belong to this inference result and `file_id`; `type_db` must
    /// use the same interner as this result.
    pub fn declared_type_before_smart_cast(
        &self,
        expr: Expr<'_>,
        file_id: FileId,
        type_db: &mut TypeDb<'_>,
    ) -> Option<TyId> {
        declared_type_before_smart_cast(
            expr,
            file_id,
            &self.expression_types,
            &self.resolved_refs,
            type_db,
        )
    }

    /// Relocates all source positions after an unchanged declaration moves in its file.
    #[must_use]
    pub fn shifted(&self, file_id: FileId, delta: i64) -> Option<Self> {
        let expression_types = self
            .expression_types
            .iter()
            .map(|(&span, &ty)| Some((span.shifted(delta)?, ty)))
            .collect::<Option<FxHashMap<_, _>>>()?;
        let resolved_refs = self
            .resolved_refs
            .iter()
            .map(|reference| {
                let mut reference = reference.clone();
                reference.decl = Span {
                    start: reference.decl,
                    end: reference.decl,
                }
                .shifted(delta)?
                .start;
                reference.span = reference.span.shifted(delta)?;

                if let Resolved::Local(local) = &mut reference.resolved
                    && local.file_id == file_id
                {
                    local.local = Span {
                        start: local.local,
                        end: local.local,
                    }
                    .shifted(delta)?
                    .start;
                }

                Some(reference)
            })
            .collect::<Option<Vec<_>>>()?;

        Some(Self {
            expression_types,
            resolved_refs,
            inferred_return_type: self.inferred_return_type,
        })
    }

    /// Resolves a reference at the given span.
    #[must_use]
    pub fn resolve(&self, span: Span) -> Option<&NameUse> {
        let pos = span.start;
        self.resolved_refs
            .binary_search_by(|u| {
                if pos < u.span.start {
                    std::cmp::Ordering::Greater
                } else if pos >= u.span.end {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .ok()
            .map(|idx| &self.resolved_refs[idx])
    }

    /// Returns an iterator over all usages of the given global symbol.
    pub fn global_usages_of(&self, symbol_id: SymbolId) -> impl Iterator<Item = &NameUse> {
        self.resolved_refs
            .iter()
            .filter(move |u| matches!(u.resolved, Resolved::Global(id) if id == symbol_id))
    }
}
