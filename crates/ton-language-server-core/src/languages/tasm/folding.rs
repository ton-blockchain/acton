use crate::FoldingRange;
use tasm_syntax::{Argument, AstNode, Code, Dictionary, Expr, TopLevel};

pub(super) fn folding_ranges(parsed: &super::TasmParsedDocument) -> Vec<FoldingRange> {
    let mut ranges = Vec::new();
    for top_level in parsed.source_file.top_levels() {
        collect_top_level(top_level, &mut ranges);
    }
    ranges.sort_by_key(|range| {
        (
            range.start_line,
            range.start_character,
            range.end_line,
            range.end_character,
        )
    });
    ranges
}

fn collect_top_level(top_level: TopLevel<'_>, ranges: &mut Vec<FoldingRange>) {
    match top_level {
        TopLevel::Instruction(instruction) => {
            for argument in instruction.args() {
                collect_argument(argument, ranges);
            }
        }
        TopLevel::ExplicitRef(explicit_ref) => {
            if let Some(code) = explicit_ref.code() {
                collect_code(code, ranges);
            }
        }
        TopLevel::EmbedSlice(_) | TopLevel::Exotic(_) | TopLevel::Unmapped(_) => {}
    }
}

fn collect_argument(argument: Argument<'_>, ranges: &mut Vec<FoldingRange>) {
    if let Some(expression) = argument.expr() {
        collect_expression(expression, ranges);
    }
}

fn collect_expression(expression: Expr<'_>, ranges: &mut Vec<FoldingRange>) {
    match expression {
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
    push_range(code.syntax(), ranges);

    if let Some(instructions) = code.instructions() {
        for top_level in instructions.items() {
            collect_top_level(top_level, ranges);
        }
    }
}

fn collect_dictionary(dictionary: Dictionary<'_>, ranges: &mut Vec<FoldingRange>) {
    push_range(dictionary.syntax(), ranges);

    for entry in dictionary.entries() {
        if let Some(code) = entry.code() {
            collect_code(code, ranges);
        }
    }
}

fn push_range(node: tree_sitter::Node<'_>, ranges: &mut Vec<FoldingRange>) {
    let Ok(start_line) = u32::try_from(node.start_position().row) else {
        return;
    };
    let Ok(end_line) = u32::try_from(node.end_position().row) else {
        return;
    };
    if end_line <= start_line {
        return;
    }

    ranges.push(FoldingRange::line_range(start_line, end_line));
}
