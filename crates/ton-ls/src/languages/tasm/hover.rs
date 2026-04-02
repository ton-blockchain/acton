use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
use crate::languages::instruction_docs::{build_hover_markdown, get_tasm_spec};
use lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Range};

struct HoverTarget<'a> {
    name: &'a str,
    range: Range,
}

impl Backend {
    pub async fn handle_tasm_hover(&self, params: HoverParams) -> Option<Hover> {
        crate::profile!(self, "tasm: hover");
        let uri = params.text_document_position_params.text_document.uri;
        let file = self.registry.find_tasm_file(&uri)?;

        let position = params.text_document_position_params.position;
        let target = find_target(&file, position)?;

        let tasm_spec = get_tasm_spec()?;

        let markdown = build_hover_markdown(target.name, tasm_spec)?;
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: markdown,
            }),
            range: Some(target.range),
        })
    }
}

fn find_target(
    file: &'_ ParsedSnapshot<tasm_syntax::SourceFile>,
    position: lsp_types::Position,
) -> Option<HoverTarget<'_>> {
    let node = file.node_at(position)?;

    let name = file.text_of(node);
    if name.is_empty() {
        return None;
    }

    let range = file.range_of(node);

    Some(HoverTarget { name, range })
}
