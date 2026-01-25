use rustc_hash::{FxHashMap, FxHashSet};
use std::collections::HashMap;
use std::sync::Arc;
use tolk_resolver::resolve_index::LocalDefId;
use tolk_resolver::{AstNodeSpanExt, FileId, Resolved, Span, SymbolId, SymbolKind};
use tolk_syntax::{Assign, Call, CallArgument, DotAccess, SetAssign, TryFromNode};
use tolk_ty::{InferenceResult, TypeDb};
use tree_sitter::Node;

/// Represents a single call edge in the call graph.
#[derive(Debug, Clone)]
pub struct CallEdge {
    /// The symbol being called.
    pub callee: SymbolId,
    /// The span of the call expression.
    pub span: Span,
}

bitflags::bitflags! {
    pub struct UseFlags: u8 {
        const READ    = 1 << 1;
        const WRITE   = 1 << 2;
        const MUTATE  = 1 << 3;
    }
}

pub struct FileUseFacts {
    pub per_local: FxHashMap<LocalDefId, LocalUseFacts>,
}

pub struct LocalUseFacts {
    pub flags: UseFlags,
    pub first_write_span: Option<Span>,
}

pub struct AnalysisDb {
    use_facts: FxHashMap<FileId, Arc<FileUseFacts>>,
    /// Files that have been processed for call graph.
    call_graph_processed_files: FxHashSet<FileId>,
    /// Call graph edges: caller -> list of (callee, span).
    call_graph: FxHashMap<SymbolId, Vec<CallEdge>>,
    /// Set of (caller, callee) pairs for deduplication.
    call_graph_seen: FxHashSet<(SymbolId, SymbolId)>,
}

impl Default for AnalysisDb {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisDb {
    pub fn new() -> Self {
        Self {
            use_facts: FxHashMap::default(),
            call_graph_processed_files: FxHashSet::default(),
            call_graph: FxHashMap::default(),
            call_graph_seen: FxHashSet::default(),
        }
    }

    /// Returns the call graph edges for a given caller.
    pub fn calls_from(&self, caller: SymbolId) -> Option<&Vec<CallEdge>> {
        self.call_graph.get(&caller)
    }

    /// Returns the full call graph.
    pub fn call_graph(&self) -> &FxHashMap<SymbolId, Vec<CallEdge>> {
        &self.call_graph
    }

    /// Builds call graph for a file.
    /// This should be called for each file in the project.
    pub fn build_call_graph_for_file(
        &mut self,
        type_db: &TypeDb,
        body_types: &HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
        file_id: FileId,
    ) {
        if self.call_graph_processed_files.contains(&file_id) {
            return;
        }
        self.call_graph_processed_files.insert(file_id);

        let Some(file) = type_db.file_db.get_by_id(file_id) else {
            return;
        };
        let Some(resolved_index) = type_db.project_index.resolved_uses.get(&file_id) else {
            return;
        };
        let Some(file_inference) = body_types.get(&file_id) else {
            return;
        };

        let root = file.source().tree.root_node();

        for usage in &resolved_index.uses {
            // We only care about global symbols (functions/methods)
            let Resolved::Global(callee_id) = usage.resolved else {
                continue;
            };

            // Check if callee is a function/method
            let Some(callee_symbol) = type_db.project_index.resolve_symbol(callee_id) else {
                continue;
            };

            let is_callable = matches!(
                callee_symbol.kind,
                SymbolKind::Function { .. }
                    | SymbolKind::Method { .. }
                    | SymbolKind::GetMethod { .. }
            );
            if !is_callable {
                continue;
            }

            // Find the caller (which function contains this usage)
            let Some(caller_decl) = file.find_symbol_at(usage.decl as usize) else {
                continue;
            };

            // Only track calls from functions/methods
            let caller_is_callable = matches!(
                caller_decl.kind,
                SymbolKind::Function { .. }
                    | SymbolKind::Method { .. }
                    | SymbolKind::GetMethod { .. }
            );
            if !caller_is_callable {
                continue;
            }

            // Check if this usage is actually a call (not just a reference)
            let Some(usage_node) =
                root.descendant_for_byte_range(usage.span.start(), usage.span.end())
            else {
                continue;
            };

            let is_call = Self::is_call_usage(&usage_node);
            if !is_call {
                // Also check method calls via inference
                if let Some(inference) = file_inference.get(&caller_decl.id) {
                    if let Some(resolved) = inference.resolve(usage.span) {
                        if let Resolved::Global(_) = resolved.resolved {
                            // This is resolved via inference, check parent for DotAccess + Call
                            let mut current = usage_node.parent();
                            while let Some(node) = current {
                                if Call::try_from_node(node).is_ok() {
                                    break;
                                }
                                if let Ok(dot) = DotAccess::try_from_node(node) {
                                    if dot.is_obj(&usage_node) {
                                        if let Some(parent) = node.parent() {
                                            if Call::try_from_node(parent).is_ok() {
                                                // This is a method call
                                                self.add_call_edge(
                                                    caller_decl.id,
                                                    callee_id,
                                                    usage.span,
                                                );
                                            }
                                        }
                                    }
                                    break;
                                }
                                current = node.parent();
                            }
                        }
                    }
                }
                continue;
            }

            self.add_call_edge(caller_decl.id, callee_id, usage.span);
        }
    }

    /// Adds a call edge, avoiding duplicates by callee.
    fn add_call_edge(&mut self, caller: SymbolId, callee: SymbolId, span: Span) {
        if self.call_graph_seen.insert((caller, callee)) {
            self.call_graph
                .entry(caller)
                .or_default()
                .push(CallEdge { callee, span });
        }
    }

    /// Checks if неa usage node is part of a call expression.
    fn is_call_usage(usage_node: &Node) -> bool {
        let mut current = usage_node.parent();
        while let Some(node) = current {
            if Call::try_from_node(node).is_ok() {
                // Check if usage_node is the callee (not an argument)
                if let Ok(call) = Call::try_from_node(node) {
                    if let Some(callee) = call.callee() {
                        let callee_range = callee.syntax().byte_range();
                        if callee_range.contains(&usage_node.start_byte()) {
                            return true;
                        }
                    }
                }
                return false;
            }
            current = node.parent();
        }
        false
    }

    pub fn use_facts(
        &mut self,
        type_db: &mut TypeDb,
        body_types: &HashMap<FileId, HashMap<SymbolId, InferenceResult>>,
        file_id: FileId,
    ) -> Option<Arc<FileUseFacts>> {
        if let Some(facts) = self.use_facts.get(&file_id) {
            return Some(facts.clone());
        }

        let file = type_db.file_db.get_by_id(file_id)?;
        let resolved_index = type_db.project_index.resolved_uses.get(&file_id).cloned()?;
        let root = file.source().tree.root_node();
        let inference = body_types.get(&file_id)?;

        let mut per_local_facts: FxHashMap<LocalDefId, (UseFlags, Option<Span>)> = resolved_index
            .locals
            .iter()
            .map(|l| (l.id, (UseFlags::empty(), None)))
            .collect();

        for usage in &resolved_index.uses {
            let Resolved::Local(local_id) = usage.resolved else {
                continue;
            };

            let Some((flags, first_write_span)) = per_local_facts.get_mut(&local_id) else {
                continue;
            };

            if flags.contains(UseFlags::READ) && flags.contains(UseFlags::WRITE) {
                continue;
            }

            let mut is_write = false; // is this usage is mutation

            let Some(usage_node) =
                root.descendant_for_byte_range(usage.span.start(), usage.span.end())
            else {
                continue;
            };

            let mut current = usage_node.parent();
            while let Some(node) = current {
                if let Ok(assign) = Assign::try_from_node(node) {
                    if assign.is_lhs(&usage_node) {
                        flags.insert(UseFlags::WRITE);
                        if first_write_span.is_none() {
                            *first_write_span = Some(usage.span);
                        }
                        is_write = true;
                    }
                    break;
                } else if let Ok(assign) = SetAssign::try_from_node(node) {
                    if assign.is_lhs(&usage_node) {
                        flags.insert(UseFlags::READ | UseFlags::WRITE);
                        if first_write_span.is_none() {
                            *first_write_span = Some(usage.span);
                        }
                        is_write = true;
                    }
                    break;
                } else if let Ok(argument) = CallArgument::try_from_node(node) {
                    if argument.mutate() {
                        flags.insert(UseFlags::WRITE | UseFlags::MUTATE);
                        if first_write_span.is_none() {
                            *first_write_span = Some(usage.span);
                        }
                        is_write = true;
                        break;
                    }
                } else if let Ok(dot) = DotAccess::try_from_node(node)
                    && dot.is_obj(&usage_node)
                    && let Some(call) = node.parent().and_then(|p| Call::try_from_node(p).ok())
                    && let Some(callee) = call.callee_identifier()
                    && let Some(decl) = file.find_symbol_at(usage_node.start_byte())
                    && let Some(inference) = inference.get(&decl.id)
                {
                    let resolved = inference.resolve(callee.span());

                    if let Some(resolved) = resolved
                        && let Resolved::Global(id) = resolved.resolved
                    {
                        let resolved = type_db.project_index.resolve_symbol(id);
                        if let Some(resolved) = resolved
                            && let SymbolKind::Method { is_mutable, .. } = resolved.kind
                            && is_mutable
                        {
                            flags.insert(UseFlags::READ | UseFlags::WRITE | UseFlags::MUTATE);
                            if first_write_span.is_none() {
                                *first_write_span = Some(usage.span);
                            }
                            is_write = true;
                        }
                    } else {
                        // we cannot resolve this method call, assume it mutates to avoid false positives
                        flags.insert(UseFlags::READ | UseFlags::WRITE | UseFlags::MUTATE);
                        if first_write_span.is_none() {
                            *first_write_span = Some(usage.span);
                        }
                        is_write = true;
                    }
                    break;
                }

                current = node.parent();
            }

            if !is_write {
                // if this usage is not mutation then it is read
                flags.insert(UseFlags::READ);
            }
        }

        let use_facts = per_local_facts
            .into_iter()
            .map(|(id, (flags, first_write_span))| {
                (
                    id,
                    LocalUseFacts {
                        flags,
                        first_write_span,
                    },
                )
            })
            .collect();

        let facts = Arc::new(FileUseFacts {
            per_local: use_facts,
        });
        self.use_facts.insert(file_id, facts.clone());
        Some(facts)
    }
}
