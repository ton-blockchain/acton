mod add_import;
mod fill_struct_fields;
mod generate_struct_opcode;

use super::{TolkProjectConfig, TolkResolveSnapshot, TolkWorkspaceEngine};
use crate::{CodeAction, DocumentEdits, DocumentSnapshot, Range, TextEdit, WorkspaceEdit};
use std::sync::Arc;
use tolk_resolver::{FileId, FileInfo, Span};
use tolk_syntax::{AstNode, TryFromNode};
use tolk_ty::TyId;

trait CodeActionProvider {
    fn collect(
        &self,
        context: &TolkCodeActionContext<'_>,
        actions: &mut Vec<CodeAction>,
    ) -> Option<()>;
}

impl TolkWorkspaceEngine {
    pub(super) fn code_actions(
        &self,
        document: &DocumentSnapshot,
        range: Range,
    ) -> Vec<CodeAction> {
        let (snapshot, config) = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            (state.latest_snapshot.clone(), state.project_config.clone())
        };
        let Some(snapshot) = snapshot else {
            return Vec::new();
        };
        let Some(file_id) = snapshot.find_document_file(document) else {
            return Vec::new();
        };
        let Some(file) = snapshot.file_db.get_by_id(file_id) else {
            return Vec::new();
        };
        let offset = document
            .text_index()
            .position_to_offset(document.text(), range.start);
        let context = TolkCodeActionContext {
            document,
            snapshot: &snapshot,
            config: &config,
            file,
            file_id,
            offset,
        };
        let providers: [&dyn CodeActionProvider; 3] = [
            &add_import::AddImportProvider,
            &fill_struct_fields::FillStructFieldsProvider,
            &generate_struct_opcode::GenerateStructOpcodeProvider,
        ];
        let mut actions = Vec::new();

        for provider in providers {
            let _ = provider.collect(&context, &mut actions);
        }
        actions
    }
}

struct TolkCodeActionContext<'a> {
    document: &'a DocumentSnapshot,
    snapshot: &'a TolkResolveSnapshot,
    config: &'a TolkProjectConfig,
    file: Arc<FileInfo>,
    file_id: FileId,
    offset: usize,
}

impl TolkCodeActionContext<'_> {
    fn cursor_node(&self) -> Option<tree_sitter::Node<'_>> {
        self.file
            .source()
            .tree
            .root_node()
            .descendant_for_byte_range(self.offset, self.offset)
    }

    fn ancestor_as<'tree, N>(&'tree self) -> Option<N>
    where
        N: TryFromNode<'tree>,
    {
        let mut node = self.cursor_node()?;

        loop {
            if let Ok(typed) = N::try_from_node(node) {
                return Some(typed);
            }
            node = node.parent()?;
        }
    }

    fn text_of<'tree, N>(&self, node: N) -> &str
    where
        N: AstNode<'tree>,
    {
        node.syntax()
            .utf8_text(self.document.text().as_bytes())
            .unwrap_or_default()
    }

    fn type_of_node<'tree, N>(&self, node: N) -> Option<TyId>
    where
        N: AstNode<'tree>,
    {
        let syntax = node.syntax();
        let symbol = self.file.find_symbol_at(syntax.start_byte())?;
        self.snapshot
            .all_body_types
            .get(&self.file_id)?
            .get(&symbol.id)?
            .type_of(Span::from_syntax(&syntax))
    }

    fn action(&self, title: &str, edit: TextEdit) -> CodeAction {
        CodeAction::new(
            title,
            crate::CodeActionKind::QuickFix,
            WorkspaceEdit::new(vec![DocumentEdits::new(
                self.document.uri().clone(),
                vec![edit],
            )]),
        )
    }
}
