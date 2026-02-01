use lsp_types::MessageType;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tolk_linter::Checker;
use tolk_resolver::ProjectIndexBuilder;
use tolk_resolver::symbol_resolver::resolve;
use tolk_ty::{TypeDb, TypeInterner, infer};
use tower_lsp::lsp_types::Url;
use tree_sitter::Tree;

use crate::backend::Backend;
use crate::backend::analysis::AnalysisResult;
use crate::backend::utils::FileInfoExt;

impl Backend {
    pub async fn analyze(&self, uri: Url) {
        self.analyze_incremental(uri, None).await;
    }

    pub async fn analyze_incremental(&self, uri: Url, old_tree: Option<Tree>) {
        let path = match uri.to_file_path() {
            Ok(p) => p,
            Err(_) => return,
        };

        let now = Instant::now();
        if let Some(content) = self.documents.get(&uri) {
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
            };
        }
        log::info!("Reparse took {:?}", now.elapsed());

        match self.run_analysis(path.clone()) {
            Ok(analysis) => {
                let arc_analysis = Arc::new(analysis);
                for &file_id in arc_analysis.all_body_types.keys() {
                    if let Some(info) = self.file_db.get_by_id(file_id)
                        && let Some(file_uri) = info.url()
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

    pub fn run_analysis(&self, root_path: PathBuf) -> anyhow::Result<AnalysisResult> {
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

        let now = Instant::now();
        let mut checker = Checker::new(&self.file_db, &mut type_db, &all_body_types);

        for file_id in &reachable {
            let file_info = self.file_db.get_by_id(*file_id).expect("file not found");
            if !file_info.is_workspace_file() {
                // we don't want to check non-workspace files
                continue;
            }
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
}
