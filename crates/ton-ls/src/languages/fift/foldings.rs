use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
use fift_syntax::SourceFile;
use lsp_types::{FoldingRange, FoldingRangeKind, FoldingRangeParams};
use tree_sitter::Node;

impl Backend {
    pub async fn handle_fift_folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> Option<Vec<FoldingRange>> {
        crate::profile!(self, "fift: folding_range");
        let uri = params.text_document.uri;
        let file = self.registry.find_fift_file(&uri)?;

        Some(collect_ranges(file))
    }
}

fn collect_ranges(source_file: ParsedSnapshot<SourceFile>) -> Vec<FoldingRange> {
    let mut result = Vec::new();

    for node in source_file.traverse() {
        if !node.is_named() {
            continue;
        }
        if !is_foldable(node.kind()) {
            continue;
        }
        push_generic_folding(node, &mut result);
    }

    result.sort_by_key(|range| (range.start_line, range.end_line));
    result
}

fn is_foldable(kind: &str) -> bool {
    matches!(
        kind,
        "program"
            | "proc_definition"
            | "proc_inline_definition"
            | "proc_ref_definition"
            | "method_definition"
            | "block_instruction"
            | "instruction_block"
            | "if_statement"
            | "ifjmp_statement"
            | "while_statement"
            | "repeat_statement"
            | "until_statement"
    )
}

fn push_generic_folding(node: Node<'_>, result: &mut Vec<FoldingRange>) {
    let child_count = node.child_count();
    if child_count == 0 {
        return;
    }

    let Some(open_brace) = node.child(0) else {
        return;
    };
    let Some(close_brace) = node.child(child_count - 1) else {
        return;
    };

    let start_line = open_brace.end_position().row as u32;
    let end_line = close_brace.start_position().row as u32;

    if end_line <= start_line {
        return;
    }

    result.push(FoldingRange {
        start_line,
        start_character: None,
        end_line,
        end_character: None,
        kind: Some(FoldingRangeKind::Region),
        collapsed_text: None,
    });
}
