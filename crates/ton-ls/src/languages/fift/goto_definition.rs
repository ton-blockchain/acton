use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
use crate::languages::fift::psi::FiftReference;
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Position, Range};

impl Backend {
    pub async fn handle_fift_goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Option<GotoDefinitionResponse> {
        crate::profile!(self, "fift: goto_definition");
        let uri = params.text_document_position_params.text_document.uri;
        let position = params.text_document_position_params.position;
        let file = self.registry.find_fift_file(&uri)?;

        let range = find_definition_range(&file, position)?;

        Some(GotoDefinitionResponse::Scalar(Location::new(uri, range)))
    }
}

fn find_definition_range(
    file: &ParsedSnapshot<fift_syntax::SourceFile>,
    position: Position,
) -> Option<Range> {
    let node = file.node_at(position)?;
    let reference = FiftReference::new(node, file.syntax())?;
    let definition = reference.resolve()?;
    let name_node = definition.child_by_field_name("name").unwrap_or(definition);

    Some(file.range_of(name_node))
}
