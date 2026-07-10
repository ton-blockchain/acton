use super::{TolkResolveSnapshot, TolkWorkspaceEngine, syntax::find_call_at_offset};
use crate::{DocumentSnapshot, Position, SignatureHelp, SignatureInformation};
use tolk_resolver::{FileId, Resolved, Symbol, SymbolKind};
use tolk_syntax::{BaseFunction, TryFromNode};

impl TolkWorkspaceEngine {
    pub(super) fn signature_help(
        &self,
        document: &DocumentSnapshot,
        position: Position,
    ) -> Option<SignatureHelp> {
        let snapshot = {
            let state = self.state.read().expect("Tolk workspace lock poisoned");
            state.latest_snapshot.clone()
        }?;
        let file_id = snapshot.find_document_file(document)?;
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position);

        snapshot.signature_help(file_id, offset)
    }
}

impl TolkResolveSnapshot {
    fn signature_help(&self, file_id: FileId, offset: usize) -> Option<SignatureHelp> {
        let file = self.file_db.get_by_id(file_id)?;
        let call = find_call_at_offset(file.source(), offset)?;
        let arguments = call.argument_list()?;
        let callee = call.callee_identifier()?;

        let Resolved::Global(symbol_id) = self.resolved_at(file_id, callee.start_byte())? else {
            return None;
        };

        let symbol = self.project_index.resolve_symbol(symbol_id)?;
        let declaration_file = self.file_db.get_by_id(symbol.id.file_id)?;
        let declaration = declaration_file.find_syntax_declaration(symbol.id)?;
        let function = BaseFunction::try_from_node(declaration.syntax()).ok()?;

        let skip_self = matches!(
            symbol.kind,
            SymbolKind::Method {
                is_instance: true,
                ..
            }
        );
        let parameters = function
            .parameters()
            .skip(usize::from(skip_self))
            .map(|parameter| declaration_file.text(&parameter).to_owned())
            .collect::<Vec<_>>();

        let label = format!("fun {}({})", signature_name(symbol), parameters.join(", "));
        let active_parameter = u32::try_from(arguments.active_parameter(offset)).ok()?;

        let signature =
            SignatureInformation::new(label, parameters).with_active_parameter(active_parameter);

        Some(SignatureHelp::new(signature))
    }
}

fn signature_name(symbol: &Symbol) -> &str {
    match symbol.kind {
        SymbolKind::Method { .. } => &symbol.fqn,
        _ => &symbol.name,
    }
}
