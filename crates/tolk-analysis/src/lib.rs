use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::Arc;
use tolk_dataflow::{ControlFlowGraph, build_cfg_for_top_level_with_source};
use tolk_resolver::resolve_index::LocalDefId;
use tolk_resolver::{AstNodeSpanExt, FileId, Resolved, Span, SymbolId, SymbolKind};
use tolk_syntax::{Assign, Call, CallArgument, DotAccess, SetAssign, TryFromNode};
use tolk_ty::{InferenceResult, TypeDb};

mod constant_evaluator;
mod hashes;
mod serialization_size;

pub use constant_evaluator::{
    ConstantEvaluationContext, ConstantEvaluator, ConstantValue, is_simple_literal,
};
pub use hashes::{compute_get_method_id, compute_struct_opcode};
pub use serialization_size::{
    SerializationSize, SerializationSizeContext, estimate_serialization_size,
};

bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct UseFlags: u8 {
        const READ    = 1 << 1;
        const WRITE   = 1 << 2;
        const MUTATE  = 1 << 3;
    }
}

/// Cached read/write facts for name usages in one source file.
#[derive(Debug)]
pub struct FileUseFacts {
    /// Aggregate usage facts for each local definition in the file.
    pub per_local: FxHashMap<LocalDefId, LocalUseFacts>,
    /// Usage flags keyed by the exact byte span of each referenced name.
    ///
    /// The map includes references collected by both the resolver and type
    /// inference. Definition spans are not usages and therefore are absent.
    pub per_usage: FxHashMap<Span, UseFlags>,
}

#[derive(Debug)]
pub struct LocalUseFacts {
    pub flags: UseFlags,
    pub first_write_span: Option<Span>,
}

pub struct AnalysisDb {
    use_facts: FxHashMap<FileId, Arc<FileUseFacts>>,
    cfg_by_symbol: FxHashMap<SymbolId, Arc<ControlFlowGraph>>,
}

impl Default for AnalysisDb {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalysisDb {
    #[must_use]
    pub fn new() -> Self {
        Self {
            use_facts: FxHashMap::default(),
            cfg_by_symbol: FxHashMap::default(),
        }
    }

    pub fn cfg_for_symbol(
        &mut self,
        type_db: &TypeDb,
        symbol_id: SymbolId,
    ) -> Option<Arc<ControlFlowGraph>> {
        if let Some(cfg) = self.cfg_by_symbol.get(&symbol_id) {
            return Some(cfg.clone());
        }

        let file = type_db.file_db.get_by_id(symbol_id.file_id)?;
        let top_level = file.find_syntax_declaration(symbol_id)?;
        let resolved_index = type_db
            .project_index
            .resolved_uses
            .get(&symbol_id.file_id)
            .cloned()?;
        let source = file.source().source.as_ref();

        let cfg =
            build_cfg_for_top_level_with_source(&top_level, resolved_index.as_ref(), Some(source))?;
        let cfg = Arc::new(cfg);
        self.cfg_by_symbol.insert(symbol_id, cfg.clone());
        Some(cfg)
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
        let mut per_usage = FxHashMap::default();

        let usages = resolved_index.uses.iter().chain(
            inference
                .values()
                .flat_map(|inference| inference.resolved_refs.iter()),
        );
        for usage in usages {
            let Some(usage_node) =
                root.descendant_for_byte_range(usage.span.start(), usage.span.end())
            else {
                continue;
            };

            let mut usage_flags = UseFlags::READ;
            let mut current = usage_node.parent();
            while let Some(node) = current {
                if let Ok(assign) = Assign::try_from_node(node) {
                    if assign.is_lhs(&usage_node) {
                        usage_flags = UseFlags::WRITE;
                    }
                    break;
                } else if let Ok(assign) = SetAssign::try_from_node(node) {
                    if assign.is_lhs(&usage_node) {
                        usage_flags = UseFlags::READ | UseFlags::WRITE;
                    }
                    break;
                } else if let Ok(argument) = CallArgument::try_from_node(node) {
                    if argument.mutate() {
                        usage_flags = UseFlags::WRITE | UseFlags::MUTATE;
                        break;
                    }
                } else if let Ok(dot) = DotAccess::try_from_node(node)
                    && let Some(call) = node.parent().and_then(|p| Call::try_from_node(p).ok())
                    && let Some(callee) = call.callee_identifier()
                    && (dot.is_obj(&usage_node) || callee.span() == usage.span)
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
                            usage_flags = UseFlags::READ | UseFlags::WRITE | UseFlags::MUTATE;
                        }
                    } else {
                        // we cannot resolve this method call, assume it mutates to avoid false positives
                        usage_flags = UseFlags::READ | UseFlags::WRITE | UseFlags::MUTATE;
                    }
                    break;
                }

                current = node.parent();
            }

            per_usage
                .entry(usage.span)
                .and_modify(|flags: &mut UseFlags| flags.insert(usage_flags))
                .or_insert(usage_flags);

            if let Resolved::Local(local_id) = usage.resolved
                && let Some((flags, first_write_span)) = per_local_facts.get_mut(&local_id)
            {
                flags.insert(usage_flags);
                if usage_flags.contains(UseFlags::WRITE) && first_write_span.is_none() {
                    *first_write_span = Some(usage.span);
                }
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
            per_usage,
        });
        self.use_facts.insert(file_id, facts.clone());
        Some(facts)
    }
}
