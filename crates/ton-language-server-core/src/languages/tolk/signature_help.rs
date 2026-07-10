use super::{TolkResolveSnapshot, TolkWorkspaceEngine, logical_path_for_uri};
use crate::{
    DocumentSnapshot, ParameterInformation, Position, SignatureHelp, SignatureInformation,
};
use tolk_resolver::{FileId, Resolved, Symbol, SymbolKind};
use tolk_syntax::{AstNode, BaseFunction, Call, TryFromNode};

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
        let path = logical_path_for_uri(document.uri());
        let file_id = snapshot.project_index.get_file_by_path(&path)?;
        let offset = document
            .text_index()
            .position_to_offset(document.text(), position);

        snapshot.signature_help(file_id, offset)
    }
}

impl TolkResolveSnapshot {
    fn signature_help(&self, file_id: FileId, offset: usize) -> Option<SignatureHelp> {
        let file = self.file_db.get_by_id(file_id)?;
        let call = call_at_offset(file.source(), offset)?;
        let arguments = call.argument_list()?;
        let callee = call.callee_identifier()?;
        let Resolved::Global(symbol_id) = self.resolved_at(file_id, callee.start_byte())? else {
            return None;
        };
        let symbol = self.project_index.resolve_symbol(symbol_id)?;
        let declaration_file = self.file_db.get_by_id(symbol.id.file_id)?;
        let declaration = declaration_file.find_syntax_declaration(symbol.id)?;
        let function = BaseFunction::try_from_node(declaration.syntax()).ok()?;
        let source = declaration_file.source().source.as_ref();
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
            .map(|parameter| ParameterInformation::new(parameter.syntax().text(source).trim()))
            .collect::<Vec<_>>();
        let parameter_list = parameters
            .iter()
            .map(|parameter| parameter.label.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let label = format!("fun {}({parameter_list})", signature_name(symbol));
        let active_parameter = u32::try_from(arguments.active_parameter(offset)).ok()?;
        let signature =
            SignatureInformation::new(label, parameters).with_active_parameter(active_parameter);

        Some(SignatureHelp::new(signature))
    }
}

fn call_at_offset(source_file: &tolk_syntax::SourceFile, offset: usize) -> Option<Call<'_>> {
    let offset = offset.min(source_file.source.len());
    let mut node = source_file
        .tree
        .root_node()
        .descendant_for_byte_range(offset, offset)?;

    loop {
        if let Ok(call) = Call::try_from_node(node)
            && call.argument_list()?.contains_offset(offset)
        {
            return Some(call);
        }

        node = node.parent()?;
    }
}

fn signature_name(symbol: &Symbol) -> &str {
    match symbol.kind {
        SymbolKind::Method { .. } => &symbol.fqn,
        _ => &symbol.name,
    }
}
