use rustc_hash::FxHashMap;
use std::collections::HashMap;
use std::sync::Arc;
use tolk_resolver::resolve_index::LocalDefId;
use tolk_resolver::{AstNodeSpanExt, FileId, Resolved, Symbol, SymbolId, SymbolKind};
use tolk_syntax::AstNode;
use tolk_syntax::AstNodeBytesKind;
use tolk_syntax::HasTreeSitterKind;
use tolk_syntax::{Assign, Call, CallArgument, DotAccess, SetAssign, match_parents};
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

        let mut use_facts = FxHashMap::default();

        for local in &resolved_index.locals {
            let mut usages = resolved_index.local_usages_of(local.id).peekable();
            if usages.peek().is_none() {
                use_facts.insert(
                    local.id,
                    LocalUseFacts {
                        flags: UseFlags::empty(),
                    },
                );
                continue;
            }

            // The variable is used for writing in a number of cases:
            // - if it is on the left side of an assignment
            // - if it is used in the `mutate` argument: `foo(mutate a)`
            // - if a mutating method is called on it

            let mut facts = UseFlags::empty();

            for usage in usages {
                let mut is_write = false; // is this usage is mutation

                let Some(usage_node) =
                    root.descendant_for_byte_range(usage.span.start(), usage.span.end())
                else {
                    continue;
                };

                // 1. Check assignments
                if let Some(assign) = match_parents!(usage_node, Assign(...))
                    && assign.is_lhs(&usage_node)
                {
                    facts.insert(UseFlags::WRITE);
                    is_write = true;
                } else if let Some(assign) = match_parents!(usage_node, SetAssign(...))
                    && assign.is_lhs(&usage_node)
                {
                    facts.insert(UseFlags::READ);
                    facts.insert(UseFlags::WRITE);
                    is_write = true;
                } else if let Some(argument) = match_parents!(usage_node, CallArgument) // 2. Check mutate arguments
                    && argument.mutate()
                {
                    facts.insert(UseFlags::WRITE);
                    is_write = true;
                } else if let Some((call, dot)) = match_parents!(usage_node, Call(dot: DotAccess))  // 3. Check method calls
                    && let Some(callee) = call.callee_identifier()
                    && dot.is_obj(&usage_node)
                    && let Some(decl) = Self::find_outer_decl(call, file_id, type_db)
                    && let Some(inference) = inference.get(&decl.id)
                {
                    let resolved2 = inference.resolve(callee.span());

                    if let Some(resolved) = resolved2
                        && let Resolved::Global(id) = resolved.resolved
                    {
                        let resolved = type_db.project_index.resolve_symbol(id);
                        if let Some(resolved) = resolved
                            && let SymbolKind::Method { is_mutable, .. } = resolved.kind
                            && is_mutable
                        {
                            facts.insert(UseFlags::READ);
                            facts.insert(UseFlags::WRITE);
                            facts.insert(UseFlags::MUTATE);
                            break;
                        }
                    } else {
                        // we cannot resolve this method call, assume it mutates to avoid false positives
                        facts.insert(UseFlags::READ);
                        facts.insert(UseFlags::WRITE);
                        facts.insert(UseFlags::MUTATE);
                        break;
                    }
                }

                if !is_write {
                    // if this usage is not mutation then it is read
                    facts.insert(UseFlags::READ);
                }

                if facts.contains(UseFlags::READ) && facts.contains(UseFlags::WRITE) {
                    // already found any possible uses
                    break;
                }
            }

            use_facts.insert(local.id, LocalUseFacts { flags: facts });
        }

        let facts = Arc::new(FileUseFacts {
            per_local: use_facts,
        });
        self.use_facts.insert(file_id, facts.clone());
        Some(facts)
    }

    fn find_outer_decl(call: Call, file_id: FileId, type_db: &TypeDb) -> Option<Symbol> {
        let file_indo = type_db.file_db.get_by_id(file_id)?;
        let mut current = call.syntax().parent();
        while let Some(parent) = current {
            let kind = parent.kind_bytes();
            if kind == b"function_declaration"
                || kind == b"method_declaration"
                || kind == b"get_method_declaration"
            {
                let decl = file_indo.find_declaration(&parent)?;
                return Some(decl.clone());
            }
            current = parent.parent();
        }
        None
    }
}
