use super::document_symbols::{symbol_detail, symbol_kind};
use super::file_info::FileInfoExt;
use super::{TolkResolveSnapshot, TolkWorkspaceEngine};
use crate::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, DocumentSnapshot,
    DocumentUri, Position, TextIndex,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use tolk_resolver::{
    AstNodeSpanExt, FileId, FileInfo, NameUse, Resolved, Span, SymbolId, SymbolKind,
};
use tolk_syntax::{Call, TryFromNode};
use tolk_ty::GlobalUsages;

impl TolkWorkspaceEngine {
    pub(super) fn prepare_call_hierarchy(
        &self,
        document: &DocumentSnapshot,
        position: Position,
    ) -> Option<CallHierarchyItem> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        }?;
        let file_id = snapshot.find_document_file(document)?;
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position);
        let symbol_id = snapshot.callable_at(file_id, offset)?;
        snapshot.call_hierarchy_item(symbol_id)
    }

    pub(super) fn incoming_calls(
        &self,
        uri: &DocumentUri,
        position: Position,
    ) -> Vec<CallHierarchyIncomingCall> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        };
        snapshot.map_or_else(Vec::new, |snapshot| snapshot.incoming_calls(uri, position))
    }

    pub(super) fn outgoing_calls(
        &self,
        uri: &DocumentUri,
        position: Position,
    ) -> Vec<CallHierarchyOutgoingCall> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        };
        snapshot.map_or_else(Vec::new, |snapshot| snapshot.outgoing_calls(uri, position))
    }
}

impl TolkResolveSnapshot {
    fn incoming_calls(
        &self,
        uri: &DocumentUri,
        position: Position,
    ) -> Vec<CallHierarchyIncomingCall> {
        let Some(target_id) = self.callable_at_uri_position(uri, position) else {
            return Vec::new();
        };
        let usages = GlobalUsages::new(self.project_index.as_ref(), &self.all_body_types);
        let mut callers = BTreeMap::<SymbolId, BTreeSet<Span>>::new();

        for reference in usages.for_symbol(target_id) {
            let Some(file) = self.file_db.get_by_id(reference.file_id) else {
                continue;
            };
            if !is_callee_usage(file.as_ref(), reference.usage) {
                continue;
            }
            let Some(caller) = file.find_symbol_at(reference.usage.span.start()) else {
                continue;
            };
            if !is_callable(&caller.kind) {
                continue;
            }
            callers
                .entry(caller.id)
                .or_default()
                .insert(reference.usage.span);
        }

        let mut calls = callers
            .into_iter()
            .filter_map(|(caller_id, spans)| {
                let file = self.file_db.get_by_id(caller_id.file_id)?;
                Some(CallHierarchyIncomingCall {
                    from: self.call_hierarchy_item(caller_id)?,
                    from_ranges: spans
                        .into_iter()
                        .map(|span| file.range_for_span(span))
                        .collect(),
                })
            })
            .collect::<Vec<_>>();
        calls.sort_by(|left, right| compare_items(&left.from, &right.from));
        calls
    }

    fn outgoing_calls(
        &self,
        uri: &DocumentUri,
        position: Position,
    ) -> Vec<CallHierarchyOutgoingCall> {
        let Some(caller_id) = self.callable_at_uri_position(uri, position) else {
            return Vec::new();
        };
        let Some(file) = self.file_db.get_by_id(caller_id.file_id) else {
            return Vec::new();
        };
        let Some(caller) = self.project_index.resolve_symbol(caller_id) else {
            return Vec::new();
        };
        let decl_start = caller.body_span.start;
        let indexed = self
            .project_index
            .get_resolved_uses(caller_id.file_id)
            .into_iter()
            .flat_map(|index| index.uses.iter())
            .filter(move |usage| usage.decl == decl_start);
        let inferred = self
            .all_body_types
            .get(&caller_id.file_id)
            .and_then(|types| types.get(&caller_id))
            .into_iter()
            .flat_map(|inference| inference.resolved_refs.iter());
        let mut callees = BTreeMap::<SymbolId, BTreeSet<Span>>::new();

        for usage in indexed.chain(inferred) {
            let Resolved::Global(target_id) = usage.resolved else {
                continue;
            };
            let Some(target) = self.project_index.resolve_symbol(target_id) else {
                continue;
            };
            if !is_callable(&target.kind) || !is_callee_usage(file.as_ref(), usage) {
                continue;
            }
            callees.entry(target_id).or_default().insert(usage.span);
        }

        let mut calls = callees
            .into_iter()
            .filter_map(|(callee_id, spans)| {
                Some(CallHierarchyOutgoingCall {
                    to: self.call_hierarchy_item(callee_id)?,
                    from_ranges: spans
                        .into_iter()
                        .map(|span| file.range_for_span(span))
                        .collect(),
                })
            })
            .collect::<Vec<_>>();
        calls.sort_by(|left, right| compare_items(&left.to, &right.to));
        calls
    }

    fn callable_at_uri_position(&self, uri: &DocumentUri, position: Position) -> Option<SymbolId> {
        let file_id = self.project_index.get_file_by_path(&uri.logical_path())?;
        let file = self.file_db.get_by_id(file_id)?;
        let source = file.source().source.as_ref();
        let offset = TextIndex::new(source).position_to_offset(source, position);
        self.callable_at(file_id, offset)
    }

    fn callable_at(&self, file_id: FileId, offset: usize) -> Option<SymbolId> {
        let Resolved::Global(symbol_id) = self.resolved_at(file_id, offset)? else {
            return None;
        };
        self.project_index
            .resolve_symbol(symbol_id)
            .filter(|symbol| is_callable(&symbol.kind))
            .map(|symbol| symbol.id)
    }

    fn call_hierarchy_item(&self, symbol_id: SymbolId) -> Option<CallHierarchyItem> {
        let symbol = self.project_index.resolve_symbol(symbol_id)?;
        let file = self.file_db.get_by_id(symbol_id.file_id)?;
        let source = file.source().source.as_ref();
        let name = match symbol.kind {
            SymbolKind::Method { .. } => symbol.fqn.to_string(),
            SymbolKind::GetMethod { .. } => format!("get {}", symbol.name),
            _ => symbol.name.to_string(),
        };

        Some(CallHierarchyItem {
            name,
            detail: symbol_detail(symbol, file.as_ref(), source),
            kind: symbol_kind(&symbol.kind),
            uri: self.file_uri(symbol_id.file_id)?.clone(),
            range: file.range_for_span(symbol.body_span),
            selection_range: file.range_for_span(symbol.name_span),
        })
    }
}

fn compare_items(left: &CallHierarchyItem, right: &CallHierarchyItem) -> Ordering {
    left.uri
        .as_str()
        .cmp(right.uri.as_str())
        .then(left.selection_range.start.cmp(&right.selection_range.start))
        .then(left.name.cmp(&right.name))
}

fn is_callee_usage(file: &FileInfo, usage: &NameUse) -> bool {
    let Some(mut node) = file.find_node_at_span(usage.span) else {
        return false;
    };
    loop {
        if let Ok(call) = Call::try_from_node(node) {
            return call
                .callee_identifier()
                .is_some_and(|callee| callee.span() == usage.span);
        }
        let Some(parent) = node.parent() else {
            return false;
        };
        node = parent;
    }
}

const fn is_callable(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Function { .. } | SymbolKind::Method { .. } | SymbolKind::GetMethod { .. }
    )
}
