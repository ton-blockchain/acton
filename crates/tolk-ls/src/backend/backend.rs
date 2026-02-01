use dashmap::DashMap;
use lsp_types::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tolk_linter::Checker;
use tolk_resolver::ProjectIndexBuilder;
use tolk_resolver::file_db::{FileDb, FileInfo};
use tolk_resolver::file_index::FileId;
use tolk_resolver::symbol_resolver::resolve;
use tolk_syntax::{AstNode, HasName, TopLevel};
use tolk_ty::TypeDb;
use tolk_ty::infer;
use tolk_ty::TypeInterner;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::Url;
use tower_lsp::{Client, LanguageServer};
use tree_sitter::{InputEdit, Point, Range as TSRange, Tree};

use crate::backend::analysis::{AnalysisResult, ChangeType};
use crate::backend::utils::*;
use crate::backend::diagnostics::convert_single_diagnostic;
use crate::backend::inlay_hints::collect_inlay_hints;

pub struct Backend {
    pub client: Client,
    pub file_db: Arc<FileDb>,
    pub documents: DashMap<Url, String>,
    pub analysis: DashMap<Url, Arc<AnalysisResult>>,
    pub line_offsets: DashMap<PathBuf, Arc<Vec<usize>>>,
    pub file_urls: DashMap<FileId, Url>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        let now = Instant::now();
        log::info!("Request: initialize");
        let res = Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                inlay_hint_provider: Some(OneOf::Left(true)),
                workspace_symbol_provider: Some(OneOf::Left(true)),
                code_action_provider: Some(CodeActionProviderCapability::Simple(true)),
                ..Default::default()
            },
            ..Default::default()
        });
        log::info!("Response: initialize took {:?}", now.elapsed());
        res
    }

    async fn initialized(&self, _: InitializedParams) {
        let now = Instant::now();
        log::info!("Notification: initialized");
        self.client
            .log_message(MessageType::INFO, "Tolk Language Server initialized")
            .await;
        log::info!("Notification: initialized took {:?}", now.elapsed());
    }

    async fn shutdown(&self) -> LspResult<()> {
        let now = Instant::now();
        log::info!("Request: shutdown");
        let res = Ok(());
        log::info!("Response: shutdown took {:?}", now.elapsed());
        res
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let now = Instant::now();
        log::info!("Notification: did_open for {}", params.text_document.uri);
        self.update_document(&params.text_document.uri, params.text_document.text);
        self.analyze(params.text_document.uri).await;
        log::info!("Notification: did_open took {:?}", now.elapsed());
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let now = Instant::now();
        let uri = params.text_document.uri;
        log::info!("Notification: did_change for {}", &uri);

        let path = uri.to_file_path().unwrap();
        let mut text = self
            .documents
            .get(&uri)
            .map(|d| d.clone())
            .unwrap_or_default();
        let mut old_tree = self
            .file_db
            .get_by_path(&path)
            .map(|f| f.source().tree.clone());
        let mut changes_ranges = Vec::new();

        for change in params.content_changes {
            if let Some(range) = change.range {
                let start_byte = get_byte_offset(&text, range.start);
                let old_end_byte = get_byte_offset(&text, range.end);
                let start_position = get_point(&text, range.start);
                let old_end_position = get_point(&text, range.end);

                text.replace_range(start_byte..old_end_byte, &change.text);

                let new_end_byte = start_byte + change.text.len();
                let new_end_position =
                    get_point(&text, offset_to_lsp_pos(new_end_byte, &text));

                if let Some(ref mut tree) = old_tree {
                    tree.edit(&InputEdit {
                        start_byte,
                        old_end_byte,
                        new_end_byte,
                        start_position,
                        old_end_position,
                        new_end_position,
                    });
                }

                let diff = (new_end_byte as isize) - (old_end_byte as isize);
                changes_ranges
                    .retain(|r: &TSRange| r.end_byte <= start_byte || r.start_byte >= old_end_byte);

                for r in changes_ranges.iter_mut() {
                    if r.start_byte >= old_end_byte {
                        r.start_byte = (r.start_byte as isize + diff) as usize;
                        r.end_byte = (r.end_byte as isize + diff) as usize;
                    }
                }

                changes_ranges.push(TSRange {
                    start_byte,
                    end_byte: new_end_byte,
                    start_point: start_position,
                    end_point: new_end_position,
                });
            } else {
                text = change.text;
                old_tree = None;
                changes_ranges.clear();
                changes_ranges.push(TSRange {
                    start_byte: 0,
                    end_byte: text.len(),
                    start_point: Point::new(0, 0),
                    end_point: get_point(&text, offset_to_lsp_pos(text.len(), &text)),
                });
            }
        }

        self.update_document(&uri, text.clone());
        self.analyze_incremental(uri, old_tree, changes_ranges)
            .await;

        log::info!("Notification: did_change took {:?}", now.elapsed());
    }

    async fn did_save(&self, _params: DidSaveTextDocumentParams) {
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> LspResult<Option<GotoDefinitionResponse>> {
        let now = Instant::now();
        let uri = params.text_document_position_params.text_document.uri;
        log::info!("Request: goto_definition for {}", uri);

        let position = params.text_document_position_params.position;

        let result = (|| {
            let analysis = self.analysis.get(&uri)?;
            let path = uri.to_file_path().ok()?;
            let file_info = self.file_db.get_by_path(&path)?;
            let file_id = file_info.id();

            let offsets = self.line_offsets.get(&path)?;
            let offset = (*offsets.get(position.line as usize)?) + position.character as usize;

            if let Some(body_types) = analysis.all_body_types.get(&file_id) {
                for results in body_types.values() {
                    if let Ok(idx) = results.resolved_refs.binary_search_by(|u| {
                        if (offset as u32) < u.span.start {
                            std::cmp::Ordering::Greater
                        } else if (offset as u32) >= u.span.end {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Equal
                        }
                    }) {
                        return self.resolve_to_location(&results.resolved_refs[idx], &analysis);
                    }
                }
            }

            // Search in project index (binary search inside find_use)
            if let Some(name_use) = analysis.project_index.find_use(file_id, offset)
                && let Some(res) = self.resolve_to_location(name_use, &analysis)
            {
                return Some(res);
            }

            None
        })();

        log::info!("Response: goto_definition took {:?}", now.elapsed());
        Ok(result)
    }

    async fn references(&self, params: ReferenceParams) -> LspResult<Option<Vec<Location>>> {
        let now = Instant::now();
        let uri = params.text_document_position.text_document.uri.clone();
        log::info!("Request: goto_references for {}", uri);

        let position = params.text_document_position.position;

        let result = (|| {
            let analysis = self.analysis.get(&uri)?;
            let path = uri.to_file_path().ok()?;
            let file_info = self.file_db.get_by_path(&path)?;
            let file_id = file_info.id();

            let offsets = self.line_offsets.get(&path)?;
            let offset = (*offsets.get(position.line as usize)?) + position.character as usize;

            let mut locations = Vec::new();

            let global_symbol = analysis
                .project_index
                .files()
                .get(&file_id)
                .and_then(|f| f.decls.iter().find(|decl| decl.name_span.contains(offset)));

            if let Some(global_symbol) = global_symbol {
                for (fid, file_resolve_index) in analysis.project_index.resolved_uses().iter() {
                    for usage in &file_resolve_index.uses {
                        if let tolk_resolver::resolve_index::Resolved::Global(usage_symbol_id) =
                            &usage.resolved
                            && usage_symbol_id == &global_symbol.id
                            && let Some(file_info) = self.file_db.get_by_id(*fid)
                            && let Ok(file_uri) = Url::from_file_path(&file_info.index().path)
                        {
                            let range = self.offset_to_range(&file_info, usage.span.start());
                            locations.push(Location::new(file_uri.clone(), range));
                        }
                    }
                }
            } else {
                let local_symbol_info = analysis
                    .project_index
                    .resolved_uses()
                    .get(&file_id)?
                    .locals
                    .iter()
                    .find(|local| local.def_span.contains(offset));

                if let Some(local_def) = local_symbol_info {
                    for (fid, file_resolve_index) in analysis.project_index.resolved_uses().iter() {
                        for usage in &file_resolve_index.uses {
                            if let tolk_resolver::resolve_index::Resolved::Local(usage_symbol_id) =
                                &usage.resolved
                                && usage_symbol_id == &local_def.id
                                && let Some(file_info) = self.file_db.get_by_id(*fid)
                                && let Ok(file_uri) = Url::from_file_path(&file_info.index().path)
                            {
                                let range = self.offset_to_range(&file_info, usage.span.start());
                                locations.push(Location::new(file_uri.clone(), range));
                            }
                        }
                    }
                }
            }

            Some(locations)
        })();

        log::info!(
            "Response: goto_references took {:?}, found {} references",
            now.elapsed(),
            result.as_ref().map(|v| v.len()).unwrap_or(0)
        );
        Ok(result)
    }

    async fn inlay_hint(&self, params: InlayHintParams) -> LspResult<Option<Vec<InlayHint>>> {
        let now = Instant::now();
        let uri = params.text_document.uri;
        log::info!("Request: inlay_hint for {}", uri);

        let result = (|| {
            let analysis = self.analysis.get(&uri)?;
            let path = uri.to_file_path().ok()?;
            let file_info = self.file_db.get_by_path(&path)?;
            let file_id = file_info.id();

            let mut hints = Vec::new();

            if let Some(body_types) = analysis.all_body_types.get(&file_id) {
                for (&symbol_id, inference_result) in body_types {
                    let decl = file_info.source().top_levels().find(|decl| {
                        file_info
                            .find_declaration(decl)
                            .is_some_and(|index_decl| index_decl.id == symbol_id)
                    });

                    if let Some(decl) = decl {
                        collect_inlay_hints(
                            inference_result,
                            &analysis.project_index,
                            &analysis.type_interner,
                            file_id,
                            &file_info,
                            &decl,
                            &mut hints,
                        );
                    }
                }
            }

            Some(hints)
        })();

        log::info!("Response: inlay_hint took {:?}", now.elapsed());
        Ok(result)
    }

    async fn code_action(&self, params: CodeActionParams) -> LspResult<Option<CodeActionResponse>> {
        let now = Instant::now();
        let uri = params.text_document.uri;
        log::info!("Request: code_action for {}", uri);

        let result = if let Some(analysis) = self.analysis.get(&uri) {
            if let Ok(path) = uri.to_file_path() {
                if let Some(file_info) = self.file_db.get_by_path(&path) {
                    let file_id = file_info.id();
                    let mut actions = Vec::new();

                    // Find diagnostics for this file that have fixes
                    for diag in &analysis.diagnostics {
                        if diag.file_id != file_id {
                            continue;
                        }

                        // Check if the diagnostic range intersects with the requested range
                        if let Some(first_annotation) = diag.annotations.first() {
                            let diag_range =
                                self.offset_to_range(&file_info, first_annotation.span.start());
                            if !ranges_intersect(&diag_range, &params.range) {
                                continue;
                            }
                        }

                        // Convert fixes to code actions
                        for (fix_idx, fix) in diag.fixes.iter().enumerate() {
                            let mut edits = Vec::new();
                            for edit in &fix.edits {
                                let start_range =
                                    self.offset_to_range(&file_info, edit.span.start());
                                let end_range = self.offset_to_range(&file_info, edit.span.end());
                                let edit_range = Range::new(start_range.start, end_range.start);

                                edits.push(TextEdit::new(edit_range, edit.replacement.clone()));
                            }

                            let action = CodeActionOrCommand::CodeAction(CodeAction {
                                title: fix.message.clone(),
                                kind: Some(CodeActionKind::QUICKFIX),
                                diagnostics: Some(vec![
                                    self.convert_single_diagnostic(diag, &file_info),
                                ]),
                                edit: Some(WorkspaceEdit {
                                    changes: Some(HashMap::from([(uri.clone(), edits)])),
                                    document_changes: None,
                                    change_annotations: None,
                                }),
                                command: None,
                                data: None,
                                is_preferred: Some(fix_idx == 0), // First fix is preferred
                                disabled: None,
                            });

                            actions.push(action);
                        }
                    }

                    Some(actions)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        log::info!("Response: code_action took {:?}", now.elapsed());
        Ok(result)
    }

    async fn symbol(
        &self,
        params: WorkspaceSymbolParams,
    ) -> LspResult<Option<Vec<SymbolInformation>>> {
        let now = Instant::now();
        log::info!("Request: workspace/symbol query='{}'", params.query);

        let query = params.query.to_lowercase();

        let analysis = self.analysis.iter().next().map(|r| r.value().clone());
        let Some(analysis) = analysis else {
            return Ok(None);
        };

        let mut symbols = Vec::new();

        for (fqn, ids) in analysis.project_index.global_symbols() {
            if !fqn.to_lowercase().contains(&query) {
                continue;
            }

            for &id in ids {
                if let Some(symbol) = analysis.project_index.resolve_symbol(id)
                    && let Some(file_info) = self.file_db.get_by_id(id.file_id)
                    && let Ok(uri) = Url::from_file_path(&file_info.index().path)
                {
                    let range = self.offset_to_range(&file_info, symbol.name_span.start());
                    symbols.push(SymbolInformation {
                        name: symbol.fqn.to_string(),
                        kind: self.to_lsp_symbol_kind(&symbol.kind),
                        location: Location::new(uri, range),
                        container_name: None,
                        tags: None,
                        #[allow(deprecated)]
                        deprecated: None,
                    });
                }
            }
        }

        log::info!(
            "Response: workspace/symbol took {:?}, found {} symbols",
            now.elapsed(),
            symbols.len()
        );
        Ok(Some(symbols))
    }
}

impl Backend {
    fn update_document(&self, uri: &Url, text: String) {
        let offsets = compute_offsets(&text);
        self.line_offsets
            .insert(uri.to_file_path().unwrap(), Arc::new(offsets));
        self.documents.insert(uri.clone(), text);
    }

    async fn analyze(&self, uri: Url) {
        self.analyze_incremental(uri, None, vec![]).await;
    }

    async fn analyze_incremental(&self, uri: Url, old_tree: Option<Tree>, changes: Vec<TSRange>) {
        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return,
        };

        let old_info = self.file_db.get_by_path(&path);

        let now = Instant::now();
        let new_info = if let Some(content) = self.documents.get(&uri) {
            match self.file_db.process_content_incremental(
                path.clone(),
                &content,
                old_tree.as_ref(),
            ) {
                Ok(info) => Some(info),
                Err(e) => {
                    self.client
                        .log_message(
                            MessageType::ERROR,
                            format!("Failed to process content: {}", e),
                        )
                        .await;
                    return;
                }
            }
        } else {
            None
        };
        log::info!("Reparse took {:?}", now.elapsed());

        let changed_decl = if let Some(info) = &new_info
            && let Some(first_change) = changes.first()
            && let Some(decl) = info
                .source()
                .find_top_levels_at(first_change.start_byte, first_change.end_byte)
            && let Some(name) = decl.name()
        {
            log::info!(
                "Changes inside {}",
                info.text(&name.syntax()).expect("invalid utf8")
            );
            Some(decl)
        } else {
            None
        };

        if let (Some(old), Some(new)) = (old_info, &new_info) {
            let change_type = self.detect_change_type(&old, new.source(), &changes);
            log::info!("Change type for {}: {:?}", path.display(), change_type);
        }

        match self.run_analysis(path.clone(), changed_decl) {
            Ok(analysis) => {
                let arc_analysis = Arc::new(analysis);
                for &file_id in arc_analysis.all_body_types.keys() {
                    if let Some(info) = self.file_db.get_by_id(file_id)
                        && let Ok(file_uri) = Url::from_file_path(&info.index().path)
                    {
                        self.analysis.insert(file_uri, arc_analysis.clone());
                    }
                }

                // Publish diagnostics to client
                let diagnostics_by_uri =
                    self.convert_linter_diagnostics_to_lsp(&arc_analysis.diagnostics);
                for (uri, diagnostics) in diagnostics_by_uri {
                    self.client
                        .publish_diagnostics(uri, diagnostics, None)
                        .await;
                }
            }
            Err(e) => {
                self.client
                    .log_message(
                        MessageType::ERROR,
                        format!("Analysis failed for {}: {}", path.display(), e),
                    )
                    .await;
            }
        }
    }

    fn run_analysis(
        &self,
        root_path: PathBuf,
        _changed_decl: Option<TopLevel>,
    ) -> anyhow::Result<AnalysisResult> {
        let now = Instant::now();

        let stdlib_path = self
            .file_db
            .canonicalize("/Users/petrmakhnev/emulator-rs/crates/tolkc/assets/tolk-stdlib")?;

        let root_path = self.file_db.canonicalize(root_path)?;

        let mut index = ProjectIndexBuilder::new(&self.file_db, root_path.clone())
            .with_stdlib(stdlib_path)
            .build()?;
        resolve(&self.file_db, &mut index);

        let resolving_time = now.elapsed();
        let now = Instant::now();

        let mut interner = TypeInterner::new();
        let mut type_db = TypeDb::new(&mut interner, &self.file_db, &index);

        let mut all_body_types = HashMap::new();

        let root_file_id = index
            .get_file_by_path(&root_path)
            .ok_or_else(|| anyhow::anyhow!("Root file id not found"))?;
        let reachable = index.reachable_files(root_file_id);

        for file_id in &reachable {
            let file_info = self.file_db.get_by_id(*file_id).expect("file not found");

            let mut body_types = HashMap::new();

            for decl in file_info.source().top_levels() {
                let Some(index_decl) = file_info.find_declaration(&decl) else {
                    continue;
                };

                let res = infer(&mut type_db, *file_id, index_decl.id, &decl);
                body_types.insert(index_decl.id, res);
            }

            all_body_types.insert(*file_id, body_types);
        }

        let type_inference_time = now.elapsed();

        let bodies = all_body_types.values().flat_map(|b| b.keys()).count();
        log::info!(
            "Analysing took: resolving {resolving_time:?}, type inference {type_inference_time:?}, bodies: {bodies}"
        );

        // Pre-compute offsets for all reachable files
        for file_id in &reachable {
            let file_info = self.file_db.get_by_id(*file_id).expect("file not found");
            if !self.line_offsets.contains_key(&file_info.index().path) {
                let offsets = compute_offsets(&file_info.source().source);
                self.line_offsets
                    .insert(file_info.index().path.clone(), Arc::new(offsets));
            }
        }

        let now = Instant::now();
        let mut checker = Checker::new(&self.file_db, &mut type_db, &all_body_types);

        for file_id in &reachable {
            let file_info = self.file_db.get_by_id(*file_id).expect("file not found");
            if !file_info.is_workspace_file() {
                // we don't want to check non-workspace files
                continue;
            }
            println!("{:?}", file_info.index().path);
            checker.process_file(file_info.source(), *file_id);
        }

        let diagnostics = checker.diagnostics;
        let linting_time = now.elapsed();
        log::info!("Linting took {:?}", linting_time);

        Ok(AnalysisResult {
            project_index: Arc::new(index),
            type_interner: Arc::new(interner),
            all_body_types,
            diagnostics,
        })
    }

    fn resolve_to_location(
        &self,
        name_use: &tolk_resolver::resolve_index::NameUse,
        analysis: &AnalysisResult,
    ) -> Option<GotoDefinitionResponse> {
        match name_use.resolved {
            tolk_resolver::resolve_index::Resolved::Local(def_id) => {
                let target_info = self.file_db.get_by_id(def_id.file_id)?;
                let target_uri = self
                    .file_urls
                    .entry(def_id.file_id)
                    .or_insert_with(|| Url::from_file_path(&target_info.index().path).unwrap())
                    .clone();
                let range = self.offset_to_range(&target_info, def_id.local as usize);
                Some(GotoDefinitionResponse::Scalar(Location::new(
                    target_uri, range,
                )))
            }
            tolk_resolver::resolve_index::Resolved::Global(sym_id) => {
                let symbol = analysis.project_index.resolve_symbol(sym_id)?;
                let target_info = self.file_db.get_by_id(sym_id.file_id)?;
                let target_uri = self
                    .file_urls
                    .entry(sym_id.file_id)
                    .or_insert_with(|| Url::from_file_path(&target_info.index().path).unwrap())
                    .clone();
                let range = self.offset_to_range(&target_info, symbol.name_span.start());
                Some(GotoDefinitionResponse::Scalar(Location::new(
                    target_uri, range,
                )))
            }
            _ => None,
        }
    }

    fn offset_to_range(&self, file_info: &Arc<FileInfo>, offset: usize) -> Range {
        let offsets = self
            .line_offsets
            .get(&file_info.index().path)
            .map(|o| o.clone())
            .unwrap_or_else(|| Arc::new(compute_offsets(&file_info.source().source)));

        crate::backend::utils::offset_to_range(&offsets, &file_info.source().source, offset)
    }

    fn detect_change_type(
        &self,
        old_file: &FileInfo,
        new_file: &tolk_syntax::SourceFile,
        changes: &[TSRange],
    ) -> ChangeType {
        let changed_ranges = old_file
            .source()
            .tree
            .changed_ranges(&new_file.tree)
            .collect::<Vec<_>>();

        if changed_ranges.is_empty() {
            return ChangeType::WhitespaceOnly;
        }

        let mut common_function = None;

        for range in changes {
            let node = new_file
                .tree
                .root_node()
                .descendant_for_byte_range(range.start_byte, range.end_byte);

            // Find if this node is inside a function body
            let mut current = node;
            let mut found_function = None;

            while let Some(n) = current {
                let kind = n.kind();
                if kind == "function_declaration"
                    || kind == "method_declaration"
                    || kind == "get_method_declaration"
                {
                    // Check if the change is within the body
                    if let Some(body) = n.child_by_field_name("body")
                        && range.start_byte >= body.start_byte()
                        && range.end_byte <= body.end_byte()
                    {
                        let name = n
                            .child_by_field_name("name")
                            .and_then(|id| id.utf8_text(new_file.source.as_bytes()).ok())
                            .unwrap_or("unknown")
                            .to_string();
                        found_function = Some(name);
                        break;
                    }
                    // If it's in the function but not in the body (e.g. signature changed), it's global
                    return ChangeType::Global;
                }
                current = n.parent();
            }

            match found_function {
                Some(name) => {
                    if let Some(ref existing_name) = common_function {
                        if existing_name != &name {
                            return ChangeType::Global; // Multiple functions changed
                        }
                    } else {
                        common_function = Some(name);
                    }
                }
                None => return ChangeType::Global, // Change outside any function
            }
        }

        if let Some(name) = common_function {
            ChangeType::Local(name)
        } else {
            // Should be unreachable if changed_ranges is not empty but changes is empty
            // But if we had full reparse (changes empty) -> Global
            if changes.is_empty() {
                ChangeType::Global
            } else {
                ChangeType::WhitespaceOnly
            }
        }
    }

    fn to_lsp_symbol_kind(
        &self,
        kind: &tolk_resolver::file_index::SymbolKind,
    ) -> lsp_types::SymbolKind {
        match kind {
            tolk_resolver::file_index::SymbolKind::GlobalVariable => {
                lsp_types::SymbolKind::VARIABLE
            }
            tolk_resolver::file_index::SymbolKind::Function { .. } => {
                lsp_types::SymbolKind::FUNCTION
            }
            tolk_resolver::file_index::SymbolKind::Method { .. } => lsp_types::SymbolKind::METHOD,
            tolk_resolver::file_index::SymbolKind::GetMethod { .. } => {
                lsp_types::SymbolKind::METHOD
            }
            tolk_resolver::file_index::SymbolKind::Struct { .. } => lsp_types::SymbolKind::STRUCT,
            tolk_resolver::file_index::SymbolKind::StructField => lsp_types::SymbolKind::FIELD,
            tolk_resolver::file_index::SymbolKind::Enum { .. } => lsp_types::SymbolKind::ENUM,
            tolk_resolver::file_index::SymbolKind::EnumMember => lsp_types::SymbolKind::ENUM_MEMBER,
            tolk_resolver::file_index::SymbolKind::Constant => lsp_types::SymbolKind::CONSTANT,
            tolk_resolver::file_index::SymbolKind::TypeAlias { .. } => lsp_types::SymbolKind::CLASS,
        }
    }

    fn convert_single_diagnostic(
        &self,
        diag: &tolk_linter::diagnostic::Diagnostic,
        file_info: &Arc<FileInfo>,
    ) -> Diagnostic {
        let offsets = self
            .line_offsets
            .get(&file_info.index().path)
            .map(|o| o.clone())
            .unwrap_or_else(|| Arc::new(compute_offsets(&file_info.source().source)));
        
        convert_single_diagnostic(diag, file_info, &offsets)
    }

    fn convert_linter_diagnostics_to_lsp(
        &self,
        linter_diagnostics: &[tolk_linter::diagnostic::Diagnostic],
    ) -> HashMap<Url, Vec<Diagnostic>> {
        let mut diagnostics_by_uri = HashMap::new();

        for diag in linter_diagnostics {
            let file_info = match self.file_db.get_by_id(diag.file_id) {
                Some(info) => info,
                None => continue,
            };

            let uri = match Url::from_file_path(&file_info.index().path) {
                Ok(uri) => uri,
                Err(_) => continue,
            };

            let lsp_diag = self.convert_single_diagnostic(diag, &file_info);
            diagnostics_by_uri
                .entry(uri)
                .or_insert_with(Vec::new)
                .push(lsp_diag);
        }

        diagnostics_by_uri
    }
}
