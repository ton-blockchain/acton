use crate::backend::Backend;
use crate::languages::engine::cache::ParsedSnapshot;
use crate::languages::instruction_docs::{InstructionDocsIndex, get_tasm_spec, stack_effect_title};
use lsp_types::{CodeLens, CodeLensParams, Command};
use tasm_syntax::{Argument, AstNode, Code, Dictionary, Expr, TopLevel};

pub const STACK_EFFECT_CODE_LENS_COMMAND: &str = "tonls.tasm.stackEffect";

impl Backend {
    pub async fn handle_tasm_code_lens(&self, params: CodeLensParams) -> Option<Vec<CodeLens>> {
        crate::profile!(self, "tasm: code_lens");
        let uri = params.text_document.uri;
        let file = self.registry.find_tasm_file(&uri)?;

        let tasm_spec = get_tasm_spec();

        let mut lenses = Vec::new();
        for top_level in file.syntax().top_levels() {
            collect_top_level(top_level, &file, tasm_spec, &mut lenses);
        }
        lenses.sort_by_key(|lens| (lens.range.start.line, lens.range.start.character));

        Some(lenses)
    }
}

fn collect_top_level(
    top_level: TopLevel<'_>,
    file: &ParsedSnapshot<tasm_syntax::SourceFile>,
    tasm_spec: Option<&InstructionDocsIndex>,
    lenses: &mut Vec<CodeLens>,
) {
    match top_level {
        TopLevel::Instruction(node) => {
            push_instruction_code_lens(node, file, tasm_spec, lenses);
            for arg in node.args() {
                collect_argument(arg, file, tasm_spec, lenses);
            }
        }
        TopLevel::ExplicitRef(node) => {
            if let Some(code) = node.code() {
                collect_code(code, file, tasm_spec, lenses);
            }
        }
        TopLevel::EmbedSlice(_) => {}
        TopLevel::Exotic(_) => {}
        TopLevel::Unmapped(_) => {}
    }
}

fn collect_argument(
    argument: Argument<'_>,
    file: &ParsedSnapshot<tasm_syntax::SourceFile>,
    instruction_docs: Option<&InstructionDocsIndex>,
    lenses: &mut Vec<CodeLens>,
) {
    if let Some(expr) = argument.expr() {
        collect_expr(expr, file, instruction_docs, lenses);
    }
}

fn collect_expr(
    expr: Expr<'_>,
    file: &ParsedSnapshot<tasm_syntax::SourceFile>,
    instruction_docs: Option<&InstructionDocsIndex>,
    lenses: &mut Vec<CodeLens>,
) {
    match expr {
        Expr::Code(code) => collect_code(code, file, instruction_docs, lenses),
        Expr::Dictionary(dictionary) => {
            collect_dictionary(dictionary, file, instruction_docs, lenses)
        }
        Expr::IntegerLit(_)
        | Expr::DataLiteral(_)
        | Expr::StackElement(_)
        | Expr::ControlRegister(_)
        | Expr::Unmapped(_) => {}
    }
}

fn collect_code(
    code: Code<'_>,
    file: &ParsedSnapshot<tasm_syntax::SourceFile>,
    instruction_docs: Option<&InstructionDocsIndex>,
    lenses: &mut Vec<CodeLens>,
) {
    if let Some(instructions) = code.instructions() {
        for top_level in instructions.items() {
            collect_top_level(top_level, file, instruction_docs, lenses);
        }
    }
}

fn collect_dictionary(
    dictionary: Dictionary<'_>,
    file: &ParsedSnapshot<tasm_syntax::SourceFile>,
    instruction_docs: Option<&InstructionDocsIndex>,
    lenses: &mut Vec<CodeLens>,
) {
    for entry in dictionary.entries() {
        if let Some(code) = entry.code() {
            collect_code(code, file, instruction_docs, lenses);
        }
    }
}

fn push_instruction_code_lens(
    instruction: tasm_syntax::Instruction<'_>,
    file: &ParsedSnapshot<tasm_syntax::SourceFile>,
    instruction_docs: Option<&InstructionDocsIndex>,
    lenses: &mut Vec<CodeLens>,
) {
    let Some(name_node) = instruction.name() else {
        return;
    };

    let instruction_name = file.text_of(name_node.syntax());
    if instruction_name.is_empty() {
        return;
    }

    let title = stack_effect_title(instruction_name, instruction_docs);
    let range = file.range_of(name_node.syntax());

    lenses.push(CodeLens {
        range,
        command: Some(Command {
            title,
            command: STACK_EFFECT_CODE_LENS_COMMAND.to_string(),
            arguments: None,
        }),
        data: None,
    });
}
