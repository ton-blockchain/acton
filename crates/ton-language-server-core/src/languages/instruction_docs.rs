use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstructionSpec {
    instructions: HashMap<String, InstructionDoc>,
}

impl InstructionSpec {
    pub fn from_json(spec_json: &str) -> serde_json::Result<Self> {
        let raw = serde_json::from_str::<RawSpecification>(spec_json)?;
        let instructions = raw
            .instructions
            .into_iter()
            .map(InstructionDoc::from)
            .map(|instruction| (instruction.name.to_ascii_uppercase(), instruction))
            .collect();
        Ok(Self { instructions })
    }

    pub(crate) fn instruction(&self, name: &str) -> Option<&InstructionDoc> {
        self.instructions.get(&name.to_ascii_uppercase())
    }

    pub(crate) fn instructions(&self) -> impl Iterator<Item = &InstructionDoc> {
        self.instructions.values()
    }

    pub(crate) fn stack_effect_title(&self, name: &str) -> String {
        self.instruction(name)
            .and_then(InstructionDoc::stack)
            .filter(|stack| !stack.is_empty())
            .unwrap_or("N/A")
            .replace(":Any", "")
            .replace(':', ": ")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InstructionDoc {
    name: String,
    category: Option<String>,
    sub_category: Option<String>,
    short: String,
    long: String,
    operands: Vec<String>,
    gas: Vec<GasSpec>,
    prefix: Option<String>,
    tlb: Option<String>,
    stack: Option<String>,
}

impl InstructionDoc {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn operands(&self) -> &[String] {
        &self.operands
    }

    pub(crate) fn stack(&self) -> Option<&str> {
        self.stack.as_deref()
    }

    pub(crate) fn gas(&self) -> String {
        format_gas_ranges(&self.gas)
    }

    pub(crate) fn render_hover(&self) -> String {
        let stack_info = self.stack().filter(|stack| !stack.is_empty()).map(|stack| {
            format!(
                "- Stack (top is on the right): `{}`",
                format_stack_effect(stack)
            )
        });
        let gas = self.gas();
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

impl From<RawInstruction> for InstructionDoc {
    fn from(raw: RawInstruction) -> Self {
        Self {
            name: raw.name,
            category: raw.category,
            sub_category: raw.sub_category,
            short: raw.description.short,
            long: raw.description.long,
            operands: raw.description.operands,
            gas: raw.description.gas.into_iter().map(GasSpec::from).collect(),
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
struct GasSpec {
    value: i64,
    description: Option<String>,
    formula: Option<String>,
}

impl From<RawGas> for GasSpec {
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

fn format_gas_ranges(entries: &[GasSpec]) -> String {
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
    let mut start_index = 0;
    for index in 0..sorted_values.len() {
        let is_last = index + 1 == sorted_values.len();
        let breaks_range = !is_last && sorted_values[index + 1] != sorted_values[index] + 1;
        if !is_last && !breaks_range {
            continue;
        }

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
