use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::Arc;
use tolk_resolver::resolve_index::LocalDefId;
use tolk_resolver::{AstNodeSpanExt, FileId, Resolved, Span, SymbolId, SymbolKind};
use tolk_syntax::{Assign, Call, CallArgument, DotAccess, SetAssign, TryFromNode};
use tolk_ty::{InferenceResult, TypeDb};

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
        }
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
