mod completion;
mod folding;

use crate::language::{
    CodeLensRequest, CompletionRequest, FeatureSet, FoldingRangeRequest, HoverRequest,
    LanguagePlugin, ParseRequest, ParsedDocument,
};
use crate::logging;
use crate::{CodeLens, Command, Hover, LanguageId};
use anyhow::Context;
use std::any::Any;
use tasm_syntax::{Argument, Code, Dictionary, Expr, Instruction, TopLevel};
use tree_sitter::{Node, Tree};

pub const LANGUAGE_ID: &str = "tasm";
pub const STACK_EFFECT_CODE_LENS_COMMAND: &str = "tonls.tasm.stackEffect";

#[derive(Clone, Debug, Default)]
pub struct TasmLanguage {
    spec: Option<TasmSpec>,
}

impl TasmLanguage {
    #[must_use]
    pub const fn new() -> Self {
        Self { spec: None }
    }

    #[must_use]
    pub const fn with_spec(spec: TasmSpec) -> Self {
        Self { spec: Some(spec) }
    }

    /// Creates a TASM language plugin from a caller-provided TVM specification JSON.
    ///
    /// The language server intentionally does not bundle the specification. Native users can read
    /// it from disk, and browser users can fetch it as a static asset before constructing the
    /// plugin.
    pub fn with_spec_json(spec_json: &str) -> serde_json::Result<Self> {
        TasmSpec::from_json(spec_json).map(Self::with_spec)
    }
}

impl LanguagePlugin for TasmLanguage {
    fn language_id(&self) -> LanguageId {
        LanguageId::from(LANGUAGE_ID)
    }

    fn file_extensions(&self) -> &'static [&'static str] {
        &["tasm"]
    }

    fn capabilities(&self) -> FeatureSet {
        FeatureSet {
            hover: self.spec.is_some(),
            code_lens: self.spec.is_some(),
            folding_ranges: true,
            completion: self.spec.is_some(),
            ..FeatureSet::default()
        }
    }

    fn parse(&self, request: ParseRequest<'_>) -> anyhow::Result<Box<dyn ParsedDocument>> {
        tracing::debug!(
            target: logging::TASM_TARGET,
            operation = "tasm.parse",
            uri = request.document.uri().as_str(),
            version = request.document.version(),
            incremental = request.old_tree.is_some(),
            text_len = request.document.text().len(),
            "parsing TASM document"
        );
        let parse_started_at = request.profiler.start();
        let source_file =
            match tasm_syntax::parse_with_old_tree(request.document.text(), request.old_tree) {
                Ok(source_file) => source_file,
                Err(error) => {
                    tracing::debug!(
                        target: logging::TASM_TARGET,
                        operation = "tasm.parse",
                        uri = request.document.uri().as_str(),
                        version = request.document.version(),
                        incremental = request.old_tree.is_some(),
                        error = %error,
                        "TASM parse failed"
                    );
                    return Err(error);
                }
            };
        request.profiler.finish("tasm.parse", parse_started_at);
        tracing::debug!(
            target: logging::TASM_TARGET,
            operation = "tasm.parse",
            uri = request.document.uri().as_str(),
            version = request.document.version(),
            incremental = request.old_tree.is_some(),
            has_error = source_file.tree.root_node().has_error(),
            "parsed TASM document"
        );

        Ok(Box::new(TasmParsedDocument { source_file }))
    }

    fn hover(&self, request: HoverRequest<'_>) -> anyhow::Result<Option<Hover>> {
        let Some(spec) = &self.spec else {
            return Ok(None);
        };
        let parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TasmParsedDocument>()
            .context("TASM parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let hover = hover_at(spec, request.context.document, parsed, request.position);
        request.context.profiler.finish("tasm.hover", started_at);
        tracing::debug!(
            target: logging::TASM_TARGET,
            operation = "tasm.hover",
            uri = request.context.document.uri().as_str(),
            version = request.context.document.version(),
            line = request.position.line,
            character = request.position.character,
            has_result = hover.is_some(),
            "resolved TASM hover"
        );

        Ok(hover)
    }

    fn code_lens(&self, request: CodeLensRequest<'_>) -> anyhow::Result<Vec<CodeLens>> {
        let Some(spec) = &self.spec else {
            return Ok(Vec::new());
        };
        let parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TasmParsedDocument>()
            .context("TASM parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let mut lenses = Vec::new();
        for top_level in parsed.source_file.top_levels() {
            collect_top_level(top_level, request.context.document, spec, &mut lenses);
        }
        lenses.sort_by_key(|lens| lens.range.start);
        request
            .context
            .profiler
            .finish("tasm.code_lens", started_at);
        tracing::debug!(
            target: logging::TASM_TARGET,
            operation = "tasm.code_lens",
            uri = request.context.document.uri().as_str(),
            version = request.context.document.version(),
            result_count = lenses.len(),
            "resolved TASM code lenses"
        );

        Ok(lenses)
    }

    fn completion(&self, request: CompletionRequest<'_>) -> anyhow::Result<crate::CompletionList> {
        let Some(spec) = &self.spec else {
            return Ok(crate::CompletionList::default());
        };
        let started_at = request.context.profiler.start();
        let result = completion::completion(spec, request.context.document, request.position);
        request
            .context
            .profiler
            .finish("tasm.completion", started_at);
        Ok(result)
    }

    fn folding_ranges(
        &self,
        request: FoldingRangeRequest<'_>,
    ) -> anyhow::Result<Vec<crate::FoldingRange>> {
        let parsed = request
            .context
            .parsed
            .as_any()
            .downcast_ref::<TasmParsedDocument>()
            .context("TASM parsed document has an unexpected type")?;
        let started_at = request.context.profiler.start();
        let result = folding::folding_ranges(parsed);
        request
            .context
            .profiler
            .finish("tasm.folding_ranges", started_at);

        Ok(result)
    }
}

#[derive(Debug)]
pub struct TasmParsedDocument {
    source_file: tasm_syntax::SourceFile,
}

impl ParsedDocument for TasmParsedDocument {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn tree(&self) -> &Tree {
        &self.source_file.tree
    }
}

pub type TasmSpec = super::instruction_docs::InstructionSpec;

fn hover_at(
    spec: &TasmSpec,
    document: &crate::DocumentSnapshot,
    parsed: &TasmParsedDocument,
    position: crate::Position,
) -> Option<Hover> {
    let text = document.text();
    let point = document.text_index().position_to_point(text, position);
    let node = parsed
        .source_file
        .tree
        .root_node()
        .descendant_for_point_range(point, point)?;
    let instruction_node = instruction_ancestor(node)?;
    let name_node = Instruction(instruction_node).name()?.0;
    let offset = document.text_index().position_to_offset(text, position);
    if !contains_offset(name_node, offset) {
        return None;
    }

    let name = name_node.utf8_text(text.as_bytes()).ok()?;
    let instruction = spec.instruction(name)?;
    Some(Hover::new(
        instruction.render_hover(),
        Some(document.text_index().range_of_node(text, name_node)),
    ))
}

fn instruction_ancestor(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        if node.kind() == "instruction" {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn contains_offset(node: Node<'_>, offset: usize) -> bool {
    node.start_byte() <= offset && offset < node.end_byte()
}

fn collect_top_level(
    top_level: TopLevel<'_>,
    document: &crate::DocumentSnapshot,
    spec: &TasmSpec,
    lenses: &mut Vec<CodeLens>,
) {
    match top_level {
        TopLevel::Instruction(instruction) => {
            push_instruction_code_lens(instruction, document, spec, lenses);
            for argument in instruction.args() {
                collect_argument(argument, document, spec, lenses);
            }
        }
        TopLevel::ExplicitRef(explicit_ref) => {
            if let Some(code) = explicit_ref.code() {
                collect_code(code, document, spec, lenses);
            }
        }
        TopLevel::EmbedSlice(_) | TopLevel::Exotic(_) | TopLevel::Unmapped(_) => {}
    }
}

fn collect_argument(
    argument: Argument<'_>,
    document: &crate::DocumentSnapshot,
    spec: &TasmSpec,
    lenses: &mut Vec<CodeLens>,
) {
    if let Some(expr) = argument.expr() {
        collect_expr(expr, document, spec, lenses);
    }
}

fn collect_expr(
    expr: Expr<'_>,
    document: &crate::DocumentSnapshot,
    spec: &TasmSpec,
    lenses: &mut Vec<CodeLens>,
) {
    match expr {
        Expr::Code(code) => collect_code(code, document, spec, lenses),
        Expr::Dictionary(dictionary) => collect_dictionary(dictionary, document, spec, lenses),
        Expr::IntegerLit(_)
        | Expr::DataLiteral(_)
        | Expr::StackElement(_)
        | Expr::ControlRegister(_)
        | Expr::Unmapped(_) => {}
    }
}

fn collect_code(
    code: Code<'_>,
    document: &crate::DocumentSnapshot,
    spec: &TasmSpec,
    lenses: &mut Vec<CodeLens>,
) {
    if let Some(instructions) = code.instructions() {
        for top_level in instructions.items() {
            collect_top_level(top_level, document, spec, lenses);
        }
    }
}

fn collect_dictionary(
    dictionary: Dictionary<'_>,
    document: &crate::DocumentSnapshot,
    spec: &TasmSpec,
    lenses: &mut Vec<CodeLens>,
) {
    for entry in dictionary.entries() {
        if let Some(code) = entry.code() {
            collect_code(code, document, spec, lenses);
        }
    }
}

fn push_instruction_code_lens(
    instruction: Instruction<'_>,
    document: &crate::DocumentSnapshot,
    spec: &TasmSpec,
    lenses: &mut Vec<CodeLens>,
) {
    let text = document.text();
    let Some(name_node) = instruction.name().map(|name| name.0) else {
        return;
    };
    let Ok(name) = name_node.utf8_text(text.as_bytes()) else {
        return;
    };
    if name.is_empty() {
        return;
    }

    lenses.push(CodeLens::new(
        document.text_index().range_of_node(text, name_node),
        Some(Command::new(
            spec.stack_effect_title(name),
            STACK_EFFECT_CODE_LENS_COMMAND,
        )),
    ));
}
