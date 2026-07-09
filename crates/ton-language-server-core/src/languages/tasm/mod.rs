use crate::language::{
    CodeLensRequest, FeatureSet, HoverRequest, LanguagePlugin, ParseRequest, ParsedDocument,
};
use crate::logging;
use crate::{CodeLens, Command, Hover, LanguageId};
use anyhow::Context;
use serde::Deserialize;
use std::any::Any;
use std::collections::HashMap;
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TasmSpec {
    instructions: HashMap<String, TasmInstructionSpec>,
}

impl TasmSpec {
    pub fn from_json(spec_json: &str) -> serde_json::Result<Self> {
        let raw = serde_json::from_str::<RawSpecification>(spec_json)?;
        let instructions = raw
            .instructions
            .into_iter()
            .map(TasmInstructionSpec::from)
            .map(|instruction| (instruction.name.to_ascii_uppercase(), instruction))
            .collect();
        Ok(Self { instructions })
    }

    fn instruction(&self, name: &str) -> Option<&TasmInstructionSpec> {
        self.instructions.get(&name.to_ascii_uppercase())
    }

    fn stack_effect_title(&self, name: &str) -> String {
        self.instruction(name)
            .and_then(|instruction| instruction.stack.as_deref())
            .filter(|stack| !stack.is_empty())
            .unwrap_or("N/A")
            .replace(":Any", "")
            .replace(':', ": ")
    }
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
struct TasmInstructionSpec {
    name: String,
    category: Option<String>,
    sub_category: Option<String>,
    short: String,
    long: String,
    operands: Vec<String>,
    gas: Vec<TasmGasSpec>,
    prefix: Option<String>,
    tlb: Option<String>,
    stack: Option<String>,
}

impl TasmInstructionSpec {
    fn render_hover(&self) -> String {
        let stack_info = self
            .stack
            .as_ref()
            .filter(|stack| !stack.is_empty())
            .map(|stack| {
                format!(
                    "- Stack (top is on the right): `{}`",
                    format_stack_effect(stack)
                )
            });
        let gas = format_gas_ranges(&self.gas);
        let operands = format_operands(&self.operands);

        let raw_short = self.short.trim();
        let raw_long = self.long.trim();
        let short = if raw_short.is_empty() {
            raw_long
        } else {
            raw_short
        };
        let details = if raw_long.is_empty() || short == raw_long {
            ""
        } else {
            raw_long
        };

        let mut lines = Vec::new();
        lines.push("```".to_owned());
        if operands.is_empty() {
            lines.push(self.name.clone());
        } else {
            lines.push(format!("{} {operands}", self.name));
        }
        lines.push("```".to_owned());

        if let Some(stack_line) = stack_info {
            lines.push(stack_line);
        }
        lines.push(format!("- Gas: `{gas}`"));
        if let Some(prefix) = self.prefix.as_deref().filter(|value| !value.is_empty()) {
            lines.push(format!("- Opcode: `{prefix}`"));
        }
        if let Some(tlb) = self.tlb.as_deref().filter(|value| !value.is_empty()) {
            lines.push(format!("- TL-B: `{tlb}`"));
        }
        if let Some(category) = self.category.as_deref().filter(|value| !value.is_empty()) {
            lines.push(format!("- Category: `{category}`"));
        }
        if let Some(sub_category) = self
            .sub_category
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            lines.push(format!("- Subcategory: `{sub_category}`"));
        }
        lines.push(String::new());

        if !short.is_empty() {
            lines.push(short.to_owned());
            lines.push(String::new());
        }

        if !details.is_empty() {
            lines.push("**Details:**".to_owned());
            lines.push(String::new());
            lines.push(details.to_owned());
            lines.push(String::new());
        }

        lines.join("\n")
    }
}

impl From<RawInstruction> for TasmInstructionSpec {
    fn from(raw: RawInstruction) -> Self {
        Self {
            name: raw.name,
            category: raw.category,
            sub_category: raw.sub_category,
            short: raw.description.short,
            long: raw.description.long,
            operands: raw.description.operands,
            gas: raw
                .description
                .gas
                .into_iter()
                .map(TasmGasSpec::from)
                .collect(),
            prefix: raw
                .layout
                .as_ref()
                .and_then(|layout| layout.prefix_str.clone()),
            tlb: raw.layout.and_then(|layout| layout.tlb),
            stack: raw.signature.and_then(|signature| signature.stack_string),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TasmGasSpec {
    value: i64,
    description: Option<String>,
    formula: Option<String>,
}

impl From<RawGas> for TasmGasSpec {
    fn from(raw: RawGas) -> Self {
        Self {
            value: raw.value,
            description: raw.description,
            formula: raw.formula,
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawSpecification {
    instructions: Vec<RawInstruction>,
}

#[derive(Debug, Deserialize)]
struct RawInstruction {
    name: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    sub_category: Option<String>,
    #[serde(default)]
    description: RawDescription,
    #[serde(default)]
    layout: Option<RawLayout>,
    #[serde(default)]
    signature: Option<RawSignature>,
}

#[derive(Debug, Default, Deserialize)]
struct RawDescription {
    #[serde(default)]
    short: String,
    #[serde(default)]
    long: String,
    #[serde(default)]
    operands: Vec<String>,
    #[serde(default)]
    gas: Vec<RawGas>,
}

#[derive(Debug, Deserialize)]
struct RawGas {
    value: i64,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    formula: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawLayout {
    #[serde(default)]
    prefix_str: Option<String>,
    #[serde(default)]
    tlb: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSignature {
    #[serde(default)]
    stack_string: Option<String>,
}

fn format_stack_effect(effect: &str) -> String {
    effect.replace("->", "\u{2192}")
}

fn format_operands(operands: &[String]) -> String {
    operands
        .iter()
        .map(|operand| format!("[{operand}]"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_gas_ranges(entries: &[TasmGasSpec]) -> String {
    if entries.is_empty() {
        return "N/A".to_owned();
    }

    let formula = entries.iter().find(|entry| entry.formula.is_some());
    let non_formula_values = entries
        .iter()
        .filter(|entry| entry.formula.is_none())
        .map(|entry| entry.value)
        .collect::<Vec<_>>();

    if non_formula_values.is_empty()
        && let Some(value) = formula.and_then(|entry| entry.formula.as_ref())
    {
        return value.clone();
    }

    let mut sorted_values = non_formula_values;
    sorted_values.sort_unstable();

    let mut result_parts = Vec::new();
    let mut start_index = 0usize;

    for index in 0..sorted_values.len() {
        let is_last = index + 1 == sorted_values.len();
        let breaks_range = !is_last && sorted_values[index + 1] != sorted_values[index] + 1;
        if is_last || breaks_range {
            if start_index == index {
                result_parts.push(sorted_values[index].to_string());
            } else {
                result_parts.push(format!(
                    "{}-{}",
                    sorted_values[start_index], sorted_values[index]
                ));
            }
            start_index = index + 1;
        }
    }

    let base_gas = result_parts
        .into_iter()
        .filter(|part| part != "36")
        .collect::<Vec<_>>()
        .join(" | ");

    if let Some(value) = formula.and_then(|entry| entry.formula.as_ref()) {
        return format!("{base_gas} + {value}");
    }

    base_gas
}
