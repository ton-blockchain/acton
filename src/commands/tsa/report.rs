use super::abi::{TsaContractContext, try_decode_input_body, try_decode_storage_cell};
use acton_config::color::OwoColorize;
use anyhow::Context;
use rston::boc::Boc;
use rston::cell::Cell;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

struct TsaExportedExecution {
    index: usize,
    storages: BTreeMap<usize, TsaExportedCell>,
    inputs: BTreeMap<usize, TsaExportedCell>,
    fetched_cells: BTreeMap<usize, TsaExportedCell>,
}

struct TsaExportedCell {
    cell: Cell,
    raw_summary: Option<String>,
}

pub(super) fn print_tsa_summary(
    report_path: &Path,
    keep_report: bool,
    exported_inputs_path: &Path,
    keep_exported_inputs: bool,
    contract_contexts: &[TsaContractContext],
) -> anyhow::Result<()> {
    let report_contents = fs::read_to_string(report_path)
        .with_context(|| format!("Failed to read TSA SARIF report {}", report_path.display()))?;
    let report: TsaSarifReport = serde_json::from_str(&report_contents)
        .with_context(|| format!("Failed to parse TSA SARIF report {}", report_path.display()))?;
    let exported_executions = match load_exported_executions(exported_inputs_path) {
        Ok(executions) => executions,
        Err(err) => {
            eprintln!(
                "{} Failed to read TSA exported inputs from {}: {}",
                "Warning:".yellow(),
                exported_inputs_path.display(),
                err
            );
            Vec::new()
        }
    };

    let mut results = Vec::new();
    for run in report.runs {
        results.extend(run.results);
    }

    if results.is_empty() {
        println!("{} TSA reported no findings", "✓".green().bold());
    } else {
        println!(
            "{} TSA found {} finding(s)",
            "✗".red().bold(),
            results.len().to_string().bold()
        );

        for (index, result) in results.iter().enumerate() {
            let execution = exported_executions
                .iter()
                .find(|execution| execution.index == index);
            print_tsa_result(index + 1, result, execution, contract_contexts);
        }
    }

    if keep_report {
        println!("{} {}", "SARIF:".dimmed(), report_path.display());
    }
    if keep_exported_inputs {
        println!(
            "{} {}",
            "Exported inputs:".dimmed(),
            exported_inputs_path.display()
        );
    }

    Ok(())
}

fn print_tsa_result(
    index: usize,
    result: &TsaSarifResult,
    execution: Option<&TsaExportedExecution>,
    contract_contexts: &[TsaContractContext],
) {
    let title = result
        .message
        .as_ref()
        .and_then(|message| message.text.as_deref())
        .unwrap_or("<no TSA message>");
    let rule_id = result.rule_id.as_deref().unwrap_or("<unknown-rule>");

    println!();
    println!(
        "{} {} {}",
        format!("{index}.").bold(),
        rule_id.red().bold(),
        title
    );

    if let Some(execution) = execution {
        print_execution_summary(execution, result.properties.as_ref(), contract_contexts);
    }

    let fetched_values = result
        .properties
        .as_ref()
        .map(|properties| &properties.fetched_values);
    match fetched_values {
        Some(values) if !values.is_empty() => {
            println!("   {}", "saved values:".dimmed());
            for (key, value) in sort_fetched_values(values) {
                println!("   - {} = {}", key.cyan(), format_json_value(value));
            }
        }
        _ => {}
    }
}

fn print_execution_summary(
    execution: &TsaExportedExecution,
    properties: Option<&TsaSarifProperties>,
    contract_contexts: &[TsaContractContext],
) {
    println!("   {}", format!("execution {}:", execution.index).dimmed());

    if !execution.storages.is_empty() {
        println!("   {}", "initial storage:".dimmed());
        for (contract_id, cell) in &execution.storages {
            let contract_label = contract_contexts
                .iter()
                .find(|contract| contract.tsa_id == *contract_id)
                .map_or_else(
                    || format!("contract {contract_id}"),
                    TsaContractContext::label,
                );
            println!("   - {}:", contract_label.cyan());

            if let Some(contract) = contract_contexts
                .iter()
                .find(|contract| contract.tsa_id == *contract_id)
                && let Ok(decoded) = try_decode_storage_cell(&cell.cell, contract)
            {
                print_json_block("      ", &decoded);
            } else {
                print_raw_cell_block("      ", cell);
            }
        }
    }

    let additional_inputs = properties.map(|properties| &properties.additional_inputs);
    let input_ids = collect_execution_input_ids(execution, additional_inputs);
    if !input_ids.is_empty() {
        println!("   {}", "inputs:".dimmed());
        for input_id in input_ids {
            let input_summary =
                additional_inputs.and_then(|inputs| inputs.get(&input_id.to_string()));
            let input_kind = input_summary.and_then(|input| input.input_type.as_deref());
            let accepted_suffix = input_summary
                .map(|input| format!(", accepted={}", input.was_accepted))
                .unwrap_or_default();
            let input_type = human_input_type(input_kind);

            println!(
                "   - input {} ({input_type}{accepted_suffix})",
                input_id.to_string().cyan()
            );

            if let Some(cell) = execution.inputs.get(&input_id) {
                let decoded_messages =
                    try_decode_input_body(&cell.cell, input_kind, contract_contexts);
                match decoded_messages.len() {
                    0 => print_raw_cell_block("      ", cell),
                    1 => {
                        let decoded = &decoded_messages[0];
                        println!(
                            "      {}",
                            format!(
                                "decoded as {} for {}",
                                decoded.body_name, decoded.contract_label
                            )
                            .dimmed()
                        );
                        print_json_block("      ", &decoded.data);
                    }
                    _ => {
                        println!("      {}", "decoded candidates:".dimmed());
                        for decoded in decoded_messages {
                            println!(
                                "      {}",
                                format!("{} for {}", decoded.body_name, decoded.contract_label)
                                    .cyan()
                            );
                            print_json_block("        ", &decoded.data);
                        }
                    }
                }
            }
        }
    }

    if !execution.fetched_cells.is_empty() {
        println!("   {}", "saved cells:".dimmed());
        for (value_id, cell) in &execution.fetched_cells {
            println!("   - value {}:", value_id.to_string().cyan());
            print_raw_cell_block("      ", cell);
        }
    }
}

fn collect_execution_input_ids(
    execution: &TsaExportedExecution,
    additional_inputs: Option<&BTreeMap<String, TsaSarifAdditionalInput>>,
) -> Vec<usize> {
    let mut input_ids = execution.inputs.keys().copied().collect::<Vec<_>>();
    if let Some(additional_inputs) = additional_inputs {
        for input_id in additional_inputs.keys() {
            if let Ok(input_id) = input_id.parse::<usize>()
                && !input_ids.contains(&input_id)
            {
                input_ids.push(input_id);
            }
        }
    }
    input_ids.sort_unstable();
    input_ids
}

fn human_input_type(input_type: Option<&str>) -> &'static str {
    match input_type {
        Some("recvExternalInput") => "external",
        Some("recvInternalInput") => "internal",
        _ => "unknown",
    }
}

fn print_json_block(indent: &str, value: &Value) {
    let rendered = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    for line in rendered.lines() {
        println!("{indent}{line}");
    }
}

fn print_raw_cell_block(indent: &str, cell: &TsaExportedCell) {
    if let Some(raw_summary) = &cell.raw_summary {
        for line in raw_summary.lines() {
            println!("{indent}{}", truncate_line(line, 120));
        }
    } else {
        println!("{indent}<raw cell summary unavailable>");
    }
}

fn truncate_line(line: &str, max_len: usize) -> String {
    if line.chars().count() <= max_len {
        return line.to_string();
    }

    let mut result = String::new();
    for (index, ch) in line.chars().enumerate() {
        if index >= max_len.saturating_sub(1) {
            break;
        }
        result.push(ch);
    }
    result.push('…');
    result
}

fn load_exported_executions(
    exported_inputs_path: &Path,
) -> anyhow::Result<Vec<TsaExportedExecution>> {
    if !exported_inputs_path.exists() {
        return Ok(Vec::new());
    }

    let mut executions = Vec::new();
    for entry in fs::read_dir(exported_inputs_path)
        .with_context(|| format!("Failed to read {}", exported_inputs_path.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
            continue;
        };
        let Some(index) = parse_prefixed_index(file_name, "execution_") else {
            continue;
        };
        executions.push(load_exported_execution(index, &path)?);
    }

    executions.sort_by_key(|execution| execution.index);
    Ok(executions)
}

fn load_exported_execution(index: usize, path: &Path) -> anyhow::Result<TsaExportedExecution> {
    let mut storages = BTreeMap::new();
    let mut inputs = BTreeMap::new();
    let mut fetched_cells = BTreeMap::new();

    for entry in fs::read_dir(path).with_context(|| format!("Failed to read {}", path.display()))? {
        let entry = entry?;
        let child_path = entry.path();
        if !child_path.is_dir() {
            continue;
        }

        let Some(file_name) = child_path
            .file_name()
            .and_then(|file_name| file_name.to_str())
        else {
            continue;
        };

        if let Some(contract_id) = parse_prefixed_index(file_name, "c4_") {
            storages.insert(contract_id, load_exported_cell(&child_path)?);
            continue;
        }
        if let Some(input_id) = parse_prefixed_index(file_name, "msgBody_") {
            inputs.insert(input_id, load_exported_cell(&child_path)?);
            continue;
        }
        if let Some(value_id) = parse_prefixed_index(file_name, "fetched_") {
            fetched_cells.insert(value_id, load_exported_cell(&child_path)?);
        }
    }

    Ok(TsaExportedExecution {
        index,
        storages,
        inputs,
        fetched_cells,
    })
}

fn load_exported_cell(path: &Path) -> anyhow::Result<TsaExportedCell> {
    let boc_path = path.join("cell.boc");
    let cell = Boc::decode(
        &fs::read(&boc_path).with_context(|| format!("Failed to read {}", boc_path.display()))?,
    )
    .with_context(|| format!("Failed to decode {}", boc_path.display()))?;
    let raw_summary = fs::read_to_string(path.join("cell-types.yaml")).ok();

    Ok(TsaExportedCell { cell, raw_summary })
}

fn parse_prefixed_index(file_name: &str, prefix: &str) -> Option<usize> {
    file_name.strip_prefix(prefix)?.parse().ok()
}

fn sort_fetched_values(values: &BTreeMap<String, Value>) -> Vec<(&str, &Value)> {
    let mut items = values
        .iter()
        .map(|(key, value)| (key.as_str(), value))
        .collect::<Vec<_>>();
    items.sort_by(|(left_key, _), (right_key, _)| compare_fetched_value_keys(left_key, right_key));
    items
}

fn compare_fetched_value_keys(left: &str, right: &str) -> std::cmp::Ordering {
    match (left.parse::<u64>(), right.parse::<u64>()) {
        (Ok(left_num), Ok(right_num)) => left_num.cmp(&right_num),
        _ => left.cmp(right),
    }
}

fn format_json_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

#[derive(Deserialize, Default)]
struct TsaSarifReport {
    #[serde(default)]
    runs: Vec<TsaSarifRun>,
}

#[derive(Deserialize, Default)]
struct TsaSarifRun {
    #[serde(default)]
    results: Vec<TsaSarifResult>,
}

#[derive(Deserialize, Default)]
struct TsaSarifResult {
    #[serde(default, rename = "ruleId")]
    rule_id: Option<String>,
    #[serde(default)]
    message: Option<TsaSarifMessage>,
    #[serde(default)]
    properties: Option<TsaSarifProperties>,
}

#[derive(Deserialize, Default)]
struct TsaSarifMessage {
    #[serde(default)]
    text: Option<String>,
}

#[derive(Deserialize, Default)]
struct TsaSarifProperties {
    #[serde(default, rename = "fetchedValues")]
    fetched_values: BTreeMap<String, Value>,
    #[serde(default, rename = "additionalInputs")]
    additional_inputs: BTreeMap<String, TsaSarifAdditionalInput>,
}

#[derive(Deserialize, Default)]
struct TsaSarifAdditionalInput {
    #[serde(default, rename = "type")]
    input_type: Option<String>,
    #[serde(default, rename = "wasAccepted")]
    was_accepted: bool,
}
