use crate::backend::Backend;
use lsp_types::{FoldingRange, FoldingRangeKind, FoldingRangeParams};
use tasm_syntax::{Argument, AstNode, Code, Dictionary, Expr, TopLevel};

impl Backend {
    pub async fn handle_tasm_folding_range(
        &self,
        params: FoldingRangeParams,
    ) -> Option<Vec<FoldingRange>> {
        crate::profile!(self, "tasm: folding_range");
        let uri = params.text_document.uri;
        let file = self.registry.find_tasm_file(&uri)?;

        let mut ranges = Vec::new();
        for top_level in file.syntax().top_levels() {
            collect_top_level(top_level, &mut ranges);
        }
        ranges.sort_by_key(|range| (range.start_line, range.end_line));

        Some(ranges)
    }
}

fn collect_top_level(top_level: TopLevel<'_>, ranges: &mut Vec<FoldingRange>) {
    match top_level {
        TopLevel::Instruction(node) => {
            for arg in node.args() {
                collect_argument(arg, ranges);
            }
        }
        TopLevel::ExplicitRef(node) => {
            if let Some(code) = node.code() {
                collect_code(code, ranges);
            }
        }
        TopLevel::EmbedSlice(_) => {}
        TopLevel::Exotic(_) => {}
        TopLevel::Unmapped(_) => {}
    }
}

fn collect_argument(argument: Argument<'_>, ranges: &mut Vec<FoldingRange>) {
    if let Some(expr) = argument.expr() {
        collect_expr(expr, ranges);
    }
}

fn collect_expr(expr: Expr<'_>, ranges: &mut Vec<FoldingRange>) {
    match expr {
        Expr::Code(code) => collect_code(code, ranges),
        Expr::Dictionary(dictionary) => collect_dictionary(dictionary, ranges),
        Expr::IntegerLit(_)
        | Expr::DataLiteral(_)
        | Expr::StackElement(_)
        | Expr::ControlRegister(_)
        | Expr::Unmapped(_) => {}
    }
}

fn collect_code(code: Code<'_>, ranges: &mut Vec<FoldingRange>) {
    push_folding_range(code.syntax(), ranges);

    if let Some(instructions) = code.instructions() {
        for top_level in instructions.items() {
            collect_top_level(top_level, ranges);
        }
    }
}

fn collect_dictionary(dictionary: Dictionary<'_>, ranges: &mut Vec<FoldingRange>) {
    push_folding_range(dictionary.syntax(), ranges);

    for entry in dictionary.entries() {
        if let Some(code) = entry.code() {
            collect_code(code, ranges);
        }
    }
}

fn push_folding_range(node: tree_sitter::Node<'_>, ranges: &mut Vec<FoldingRange>) {
    let start_line = node.start_position().row as u32;
    let end_line = node.end_position().row as u32;

    if end_line <= start_line {
        return;
    }

    ranges.push(FoldingRange {
        start_line,
        start_character: None,
        end_line,
        end_character: None,
        kind: Some(FoldingRangeKind::Region),
        collapsed_text: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn collect_ranges(source: &str) -> anyhow::Result<Vec<FoldingRange>> {
        let source_file = tasm_syntax::parse(source)?;
        let mut ranges = Vec::new();
        for top_level in source_file.top_levels() {
            collect_top_level(top_level, &mut ranges);
        }
        ranges.sort_by_key(|range| (range.start_line, range.end_line));
        Ok(ranges)
    }

    #[test]
    fn folds_code_block() -> anyhow::Result<()> {
        let source = "ref {\n  PUSHINT_4 1\n}\n";
        let ranges = collect_ranges(source)?;

        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 0);
        assert_eq!(ranges[0].end_line, 2);
        Ok(())
    }

    #[test]
    fn folds_nested_code_and_dictionary() -> anyhow::Result<()> {
        let source = "PUSHCONT {\n  PUSHDICT [\n    1 => {\n      SWAP\n    }\n  ]\n}\n";
        let ranges = collect_ranges(source)?;

        let pairs = ranges
            .iter()
            .map(|r| (r.start_line, r.end_line))
            .collect::<Vec<_>>();

        assert_eq!(pairs, vec![(0, 6), (1, 5), (2, 4)]);
        Ok(())
    }
}
