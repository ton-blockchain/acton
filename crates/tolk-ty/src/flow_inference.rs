use crate::type_db::TypeDb;
use crate::type_interner::{TyId, TypeInterner};
use crate::types::TyData;
use smol_str::SmolStr;
use std::collections::{HashMap, VecDeque};
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use tolk_resolver::SymbolId;
use tolk_resolver::file_index::{AstNodeSpanExt, FileId, Span};
use tolk_resolver::resolve_index::{LocalDefId, NameUse};
use tolk_syntax::AstNode;

#[derive(Debug)]
pub struct ExprFlow {
    pub out_flow: FlowContext,

    // only calculated inside `if`, left of `&&`, etc. — there this expression is immediate condition, empty otherwise
    pub true_flow: FlowContext,
    pub false_flow: FlowContext,
}

impl ExprFlow {
    pub fn new(out_flow: FlowContext, true_flow: FlowContext, false_flow: FlowContext) -> ExprFlow {
        ExprFlow {
            out_flow,
            true_flow,
            false_flow,
        }
    }

    pub fn create(out_flow: FlowContext, clone_flow_for_condition: bool) -> ExprFlow {
        let (true_flow, false_flow) = if clone_flow_for_condition {
            (out_flow.clone(), out_flow.clone())
        } else {
            (FlowContext::default(), FlowContext::default())
        };
        Self::new(out_flow.clone(), true_flow, false_flow)
    }
}

/// UnreachableKind is a reason of why control flow is unreachable or interrupted
/// example: `return;` interrupts control flow
/// example: `if (true) ... else ...` inside "else" flow is unreachable because it can't happen
pub enum UnreachableKind {
    #[allow(dead_code)]
    Unknown, // no definite info or not unreachable
    CantHappen,
    ThrowStatement,
    ReturnStatement,
    CallNeverReturnFunction,
    Break,
    Continue,
}

/// FactsAboutExpr represents "everything known about SinkExpression at a given execution point"
/// remember, that indices/fields are also expressions, `t.1 = 2` or `u.id = 2` also store such facts
#[derive(Debug, Clone, PartialEq)]
pub struct FactsAboutExpr {
    /// originally declared type or smart cast (Unknown if no info)
    pub expr_type: TyId,
}

impl FactsAboutExpr {
    pub fn new(expr_type: TyId) -> FactsAboutExpr {
        FactsAboutExpr { expr_type }
    }
}

/// FlowContext represents "everything known about control flow at a given execution point"
/// while traversing AST, each statement node gets "in" FlowContext (prior knowledge)
/// and returns "output" FlowContext (representing a state AFTER execution of a statement)
/// on branching, like if/else, input context is cloned, two contexts for each branch calculated, and merged to a result
#[derive(Debug, Default)]
pub struct FlowContext {
    /// all local vars plus (optionally) indices/fields of tensors/tuples/objects
    known_facts: HashMap<SinkExpr, FactsAboutExpr>,
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
    pub fn new() -> Self {
        Self {
            known_facts: HashMap::new(),
            uses: Vec::new(),
            unreachable: false,
        }
    }

    pub fn from(other: &FlowContext) -> FlowContext {
        Self {
            known_facts: other.known_facts.clone(),
            uses: other.uses.clone(),
            unreachable: other.unreachable,
        }
    }

    pub fn is_unreachable(&self) -> bool {
        self.unreachable
    }

    /// invalidate knowledge about sub-fields of a variable or its field
    /// example: `tensorVar = 2`, invalidate facts about `tensorVar`, `tensorVar.0`, `tensorVar.1.2`, and all others
    /// example: `user.id = rhs`, invalidate facts about `user.id` (sign, etc.) and `user.id.*` if exist
    fn invalidate_all_subfields(&mut self, def: LocalDefId, parent_path: u64, parent_mask: u64) {
        let mut new_facts = HashMap::new();

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
    pub fn smart_cast_or_original(
        &self,
        s_expr: SinkExpr,
        originally_declared_type: TyId,
        intrn: &mut TypeInterner,
    ) -> TyId {
        let Some(facts) = self.known_facts.get(&s_expr) else {
            return originally_declared_type;
        };

        let smart_casted = facts.expr_type;
        if intrn.equals(smart_casted, originally_declared_type) {
            // given `var a: dict`, after merging control flow branches, restore `a: dict` instead of `a: cell?`
            // (same for struct fields and other sink expressions)
            return originally_declared_type;
        }

        smart_casted
    }

    /// update current type of `local_var` / `tensorVar.0` / `obj.field`
    /// example: `local_var = rhs`
    /// example: `f(mutate obj.field)`
    /// example: `if (t.0 != null)`, in true_flow `t.0` assigned to "not-null of current", in false_flow to null
    pub fn register_known_type(&mut self, expr: SinkExpr, ty: TyId) {
        // having index_path = (some bytes filled in the end),
        // calc index_mask: replace every filled byte with 0xFF
        // example: `t.0.1`, index_path = (1<<8) + 2, index_mask = 0xFFFF
        let mut index_path = expr.index_path;
        let mut index_mask = 0u64;

        while index_path > 0 {
            index_mask = index_path << 8 | 0xff;
            index_path >>= 8;
        }
        self.invalidate_all_subfields(expr.def, expr.index_path, index_mask);

        // if just `int` assigned, we have no considerations about its sign
        // so, even if something existed by the key s_expr, drop all knowledge
        // NOTE: we currently don't track sign in facts
        self.known_facts.insert(expr, FactsAboutExpr::new(ty));
    }

    /// mark control flow unreachable / interrupted
    pub fn mark_unreachable(&mut self, reason: UnreachableKind) {
        self.unreachable = true;

        // currently we don't save why control flow became unreachable (it's not obvious how, there may be consequent reasons),
        // but it helps debugging and reading outer code
        let _ = reason;
    }

    /// "merge" two data-flow contexts occurs on control flow rejoins (if/else branches merging, for example)
    /// it's generating a new context that describes "knowledge that definitely outcomes from these two"
    /// example: in one branch x is `int`, in x is `null`, result is `int?` unless any of them is unreachable
    pub fn merge_flow(&self, c2: &FlowContext, int: &mut TypeInterner) -> FlowContext {
        if !self.unreachable && c2.unreachable {
            return c2.merge_flow(self, int);
        }

        let mut unified = HashMap::new();

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
        };

        FlowContext {
            known_facts: unified,
            uses: self.uses.clone(),
            unreachable: self.unreachable && c2.unreachable,
        }
    }
}

/// SinkExpression is an expression that can be smart cast like `if (x != null)` (x is int inside)
/// or analyzed by data flow is some other way like `if (x > 0) ... else ...` (x <= 0 inside else).
/// In other words, it "absorbs" data flow facts.
/// Examples: `localVar`, `localTensor.1`, `localTuple.1.2.3`, `localObj.field`
/// These are NOT sink expressions: `globalVar`, `f()`, `f().1`
/// Note, that globals are NOT sink: don't encourage to use a global twice, it costs gas, better assign it to a local.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SinkExpr {
    /// smart casts and data flow applies only to locals
    pub def: LocalDefId,
    /// 0 for just `v`; for `v.N` it's (N+1), for `v.N.M` it's (N+1) + (M+1)<<8, etc.
    pub index_path: u64,
    /// Name of local for debug
    pub name: SmolStr,
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
            write!(f, "{}", index)?;
            cur_path >>= 8;
        }

        Ok(())
    }
}

impl SinkExpr {
    pub fn from_def(name: SmolStr, def: LocalDefId, index_path: u64) -> Self {
        Self {
            def,
            name,
            index_path,
        }
    }
}

#[derive(Debug)]
pub struct InferenceContext<'db, 'a> {
    pub type_db: &'db mut TypeDb<'a>,
    pub file_id: FileId,
    pub self_type: Option<TyId>,
    pub declared_return_ty: Option<TyId>,
    pub expression_types: HashMap<Span, TyId>,
    pub resolved_refs: Vec<NameUse>,
    pub return_types: Vec<TyId>,
    pub inferred_return_type: Option<TyId>,
    pub decl_start: u32,
    pub call_stack: VecDeque<SymbolId>,
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
            expression_types: HashMap::new(),
            resolved_refs: Vec::new(),
            return_types: Vec::new(),
            inferred_return_type: None,
            decl_start: 0,
            call_stack,
        }
    }

    pub fn set_resolved(&mut self, use_: NameUse) {
        self.resolved_refs.push(use_)
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
        self.type_db.top_level_types.insert(symbol_id, ty);
    }

    pub fn get_top_level_type(&mut self, symbol_id: SymbolId) -> Option<TyId> {
        self.type_db.top_level_types.get(&symbol_id).cloned()
    }

    pub fn set_node_type<'node, Node: AstNode<'node>>(&mut self, node: &Node, ty: TyId) {
        self.set_type(node.syntax().span(), ty)
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
            .unwrap_or(self.type_db.intrn.ty_unknown)
    }

    pub fn get_type(&self, span: Span) -> Option<TyId> {
        self.expression_types.get(&span).cloned()
    }
}

/// Represents the result of type inference for a declaration.
///
/// This structure contains all inferred types for expressions and resolved symbol references.
/// It is the main output of the `infer` function.
#[derive(Debug, Clone)]
pub struct InferenceResult {
    /// Map from AST node spans to their inferred types.
    pub expression_types: HashMap<Span, TyId>,
    /// List of resolved references (fields, methods).
    pub resolved_refs: Vec<NameUse>,
    /// The inferred return type of the function (if inference was run on a function).
    pub inferred_return_type: Option<TyId>,
}

impl InferenceResult {
    pub fn new(ctx: InferenceContext) -> Self {
        Self {
            expression_types: ctx.expression_types,
            resolved_refs: ctx.resolved_refs,
            inferred_return_type: ctx.inferred_return_type,
        }
    }

    /// Retrieves the type of an expression at the given span.
    pub fn type_of(&self, span: Span) -> Option<TyId> {
        self.expression_types.get(&span).cloned()
    }

    /// Resolves a reference at the given span.
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
}
