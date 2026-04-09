use crate::commands::build::{BuildCommandOptions, build_cmd};
use crate::commands::common::error_fmt;
use crate::commands::compile::compile_cmd;
use crate::file_build_cache::FileBuildCache;
use crate::stdlib;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, ContractConfig, project_root as configured_project_root};
use anyhow::{Context, anyhow};
use clap::Subcommand;
use rston::boc::Boc;
use rston::cell::Cell;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use tolk_compiler::abi::ContractABI as CompilerContractABI;
use tolk_compiler::dynamic_unpack::{self, UnpackedValue};
use tolk_compiler::types_kernel::render_ty;

/// Custom checker commands that compile project contracts and invoke an external TSA CLI.
/// The caller supplies the TSA JAR; checker intrinsics are executed by TSA only.
///
/// Available since trunk.
#[derive(Subcommand, Clone)]
pub enum TsaCommand {
    #[command(about = "Run a TSA custom checker against Acton contracts")]
    Check {
        #[arg(long, help = "Path to checker source (.tolk) or precompiled .boc")]
        checker: String,
        #[arg(
            long,
            help = "Path to tsa-cli.jar. Falls back to ACTON_TSA_JAR when omitted"
        )]
        tsa_cli: Option<String>,
        #[arg(long, help = "Path to inter-contract communication scheme JSON")]
        scheme: Option<String>,
        #[arg(
            long = "output",
            short = 'o',
            help = "Path to output SARIF report file"
        )]
        output: Option<String>,
        #[arg(
            long,
            help = "Folder where TSA should export execution inputs and fetched values"
        )]
        exported_inputs: Option<String>,
        #[arg(long, help = "Enable verbose TSA logging")]
        verbose: bool,
        #[arg(long, help = "Disable out message analysis")]
        disable_out_message_analysis: bool,
        #[arg(
            long,
            value_name = "OPCODE",
            help = "Specify opcodes for dividing analysis time between them"
        )]
        opcode: Vec<u64>,
        #[arg(
            value_name = "CONTRACT",
            required = true,
            help = "Acton contract ids to analyze"
        )]
        contracts: Vec<String>,
    },
}

/// Run a checker and render its SARIF findings, preserving artifacts only at requested paths.
/// Java process failures are errors; reported findings are printed without changing exit status.
///
/// Available since trunk.
pub fn tsa_cmd(command: TsaCommand) -> anyhow::Result<()> {
    match command {
        TsaCommand::Check {
            checker,
            tsa_cli,
            scheme,
            output,
            exported_inputs,
            verbose,
            disable_out_message_analysis,
            opcode,
            contracts,
        } => tsa_check_cmd(
            &checker,
            tsa_cli,
            scheme,
            output,
            exported_inputs,
            verbose,
            disable_out_message_analysis,
            opcode,
            contracts,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn tsa_check_cmd(
    checker: &str,
    tsa_cli: Option<String>,
    scheme: Option<String>,
    output: Option<String>,
    exported_inputs: Option<String>,
    verbose: bool,
    disable_out_message_analysis: bool,
    opcode: Vec<u64>,
    contracts: Vec<String>,
) -> anyhow::Result<()> {
    if contracts.len() > 1 && scheme.is_none() {
        anyhow::bail!(
            "TSA requires --scheme when analyzing multiple contracts. Pass an inter-contract communication scheme JSON."
        );
    }

    let config = ActonConfig::load()?;
    let tsa_cli_path = resolve_existing_path(
        &tsa_cli
            .or_else(|| env::var("ACTON_TSA_JAR").ok())
            .ok_or_else(|| {
                anyhow!(
                    "Path to tsa-cli.jar is required. Pass --tsa-cli <path> or set ACTON_TSA_JAR."
                )
            })?,
    )?;
    let checker_path = resolve_existing_path(checker)?;
    let scheme_path = scheme.as_deref().map(resolve_existing_path).transpose()?;
    let output_path = output.map(PathBuf::from);
    let requested_exported_inputs_path = exported_inputs.map(PathBuf::from);

    ensure_contracts_exist(&config, &contracts)?;
    prepare_output_path(output_path.as_deref())?;

    let temp_dir =
        TempDir::new().context("Failed to create temporary directory for TSA artifacts")?;
    let artifacts_dir = temp_dir.path().join("build-artifacts");
    fs::create_dir_all(&artifacts_dir)
        .with_context(|| format!("Failed to create {}", artifacts_dir.display()))?;
    let report_path = output_path
        .clone()
        .unwrap_or_else(|| temp_dir.path().join("tsa-report.sarif.json"));
    let exported_inputs_path = requested_exported_inputs_path
        .clone()
        .unwrap_or_else(|| temp_dir.path().join("tsa-exported-inputs"));
    prepare_exported_inputs_dir(Some(&exported_inputs_path))?;

    let checker_boc = prepare_checker_boc(&checker_path, temp_dir.path())?;
    let contract_bocs = prepare_contract_bocs(&contracts, &artifacts_dir, temp_dir.path())?;
    let contract_contexts = load_contract_contexts(&config, &contracts);

    let mut cmd = Command::new("java");
    cmd.arg("-jar")
        .arg(&tsa_cli_path)
        .arg("custom-checker-compiled")
        .arg("--checker")
        .arg(&checker_boc);

    for contract_boc in &contract_bocs {
        cmd.arg("-c").arg(contract_boc);
    }
    if let Some(scheme_path) = &scheme_path {
        cmd.arg("-s").arg(scheme_path);
    }
    cmd.arg("-o").arg(&report_path);
    cmd.arg("-e").arg(&exported_inputs_path);
    if verbose {
        cmd.arg("-v");
    }
    if disable_out_message_analysis {
        cmd.arg("--disable-out-message-analysis");
    }
    for opcode in opcode {
        cmd.arg("--opcode").arg(opcode.to_string());
    }

    let status = cmd.status().with_context(|| {
        format!(
            "Failed to start TSA CLI via `java -jar {}`",
            tsa_cli_path.display()
        )
    })?;
    if !status.success() {
        anyhow::bail!("TSA analysis failed with exit status {status}");
    }

    print_tsa_summary(
        &report_path,
        output_path.is_some(),
        &exported_inputs_path,
        requested_exported_inputs_path.is_some(),
        &contract_contexts,
    )?;

    Ok(())
}

fn ensure_contracts_exist(config: &ActonConfig, contracts: &[String]) -> anyhow::Result<()> {
    for contract in contracts {
        if config.get_contract(contract).is_none() {
            anyhow::bail!(error_fmt::contract_not_found(config, contract));
        }
    }
    Ok(())
}

fn resolve_existing_path(path: &str) -> anyhow::Result<PathBuf> {
    if path.is_empty() {
        anyhow::bail!(error_fmt::file_not_found(path));
    }

    let path = PathBuf::from(path);
    if !path.exists() {
        anyhow::bail!(error_fmt::file_not_found(&path.display().to_string()));
    }

    Ok(path)
}

fn prepare_output_path(path: Option<&Path>) -> anyhow::Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent.as_os_str().is_empty() {
        return Ok(());
    }

    fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create output directory {}", parent.display()))?;
    Ok(())
}

fn prepare_exported_inputs_dir(path: Option<&Path>) -> anyhow::Result<()> {
    let Some(path) = path else {
        return Ok(());
    };

    fs::create_dir_all(path).with_context(|| {
        format!(
            "Failed to create exported inputs directory {}",
            path.display()
        )
    })?;
    Ok(())
}

fn prepare_checker_boc(checker_path: &Path, temp_dir: &Path) -> anyhow::Result<PathBuf> {
    if checker_path.extension().and_then(|ext| ext.to_str()) == Some("boc") {
        return Ok(checker_path.to_path_buf());
    }
    if checker_path.extension().and_then(|ext| ext.to_str()) != Some("tolk") {
        anyhow::bail!(
            "Unsupported checker format {}. Expected .tolk or .boc.",
            checker_path.display()
        );
    }

    stdlib::ensure_latest(configured_project_root())?;

    let checker_boc_path = temp_dir.join("checker.boc");
    let checker_path_string = checker_path.display().to_string();
    let checker_boc_string = checker_boc_path.display().to_string();
    compile_cmd(
        &checker_path_string,
        false,
        false,
        Some(checker_boc_string),
        None,
        None,
        None,
        false,
        false,
    )
    .with_context(|| format!("Failed to compile TSA checker {}", checker_path.display()))?;

    Ok(checker_boc_path)
}

fn prepare_contract_bocs(
    contracts: &[String],
    artifacts_dir: &Path,
    temp_dir: &Path,
) -> anyhow::Result<Vec<PathBuf>> {
    let artifacts_dir_string = artifacts_dir.display().to_string();
    let mut result = Vec::with_capacity(contracts.len());

    for contract in contracts {
        build_cmd(BuildCommandOptions {
            contract_id: Some(contract.clone()),
            out_dir: Some(artifacts_dir_string.clone()),
            ..Default::default()
        })
        .with_context(|| format!("Failed to prepare contract `{contract}` for TSA"))?;

        let artifact_path = artifacts_dir.join(format!("{contract}.json"));
        let artifact_contents = fs::read_to_string(&artifact_path).with_context(|| {
            format!(
                "Failed to read TSA build artifact {}",
                artifact_path.display()
            )
        })?;
        let artifact: BuildArtifact =
            serde_json::from_str(&artifact_contents).with_context(|| {
                format!(
                    "Failed to parse TSA build artifact {}",
                    artifact_path.display()
                )
            })?;

        let cell = Boc::decode_base64(&artifact.code_boc64).with_context(|| {
            format!(
                "Failed to decode contract BoC from {}",
                artifact_path.display()
            )
        })?;
        let contract_boc_path = temp_dir.join(format!("{contract}.boc"));
        fs::write(&contract_boc_path, Boc::encode(cell))
            .with_context(|| format!("Failed to write {}", contract_boc_path.display()))?;
        result.push(contract_boc_path);
    }

    Ok(result)
}

#[derive(Deserialize)]
struct BuildArtifact {
    code_boc64: String,
}

#[derive(Clone)]
struct TsaContractContext {
    tsa_id: usize,
    acton_id: String,
    contract_name: String,
    compiler_abi: Option<CompilerContractABI>,
}

impl TsaContractContext {
    fn label(&self) -> String {
        if self.contract_name == self.acton_id {
            format!("{} [{}]", self.acton_id, self.tsa_id)
        } else {
            format!(
                "{} ({}) [{}]",
                self.acton_id, self.contract_name, self.tsa_id
            )
        }
    }
}

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

struct TsaDecodedMessage {
    contract_label: String,
    body_name: String,
    data: Value,
}

fn load_contract_contexts(config: &ActonConfig, contracts: &[String]) -> Vec<TsaContractContext> {
    let mut file_cache = FileBuildCache::new(None).ok();

    contracts
        .iter()
        .enumerate()
        .map(|(index, contract_id)| {
            let contract_config = config
                .get_contract(contract_id)
                .expect("contract presence already validated");
            let compiler_abi = file_cache.as_mut().and_then(|cache| {
                match try_load_compiler_abi(cache, config, contract_config) {
                    Ok(abi) => abi,
                    Err(err) => {
                        eprintln!(
                            "{} Failed to load ABI for {}: {}",
                            "Warning:".yellow(),
                            contract_id.cyan(),
                            err
                        );
                        None
                    }
                }
            });

            TsaContractContext {
                tsa_id: index + 1,
                acton_id: contract_id.clone(),
                contract_name: contract_config.display_name(contract_id).to_owned(),
                compiler_abi,
            }
        })
        .collect()
}

fn try_load_compiler_abi(
    file_cache: &mut FileBuildCache,
    config: &ActonConfig,
    contract_config: &ContractConfig,
) -> anyhow::Result<Option<CompilerContractABI>> {
    if !contract_config.src.ends_with(".tolk") {
        return Ok(None);
    }

    if let Some(entry) = file_cache.get(&contract_config.src, false, false, false) {
        return Ok(entry.abi);
    }

    let mappings = config.mappings();
    let compiler = tolk_compiler::Compiler::new().with_mappings(&mappings);
    let contract_path = resolve_project_path(&contract_config.src);

    match compiler.compile(&contract_path, false) {
        tolk_compiler::CompilerResult::Success(result) => {
            let abi = result.abi.clone();
            let _ = file_cache.put(&contract_config.src, &result, false, false, false);
            Ok(abi)
        }
        tolk_compiler::CompilerResult::Error(error) => {
            anyhow::bail!(
                "cannot compile {} for ABI extraction: {}",
                contract_path.display(),
                error.message
            )
        }
    }
}

fn resolve_project_path(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        configured_project_root().join(path)
    }
}

fn print_tsa_summary(
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
                && let Ok(decoded) = try_decode_storage_cell(cell, contract)
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
                let decoded_messages = try_decode_input_body(cell, input_kind, contract_contexts);
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

fn try_decode_storage_cell(
    cell: &TsaExportedCell,
    contract: &TsaContractContext,
) -> anyhow::Result<Value> {
    let abi = contract
        .compiler_abi
        .as_ref()
        .ok_or_else(|| anyhow!("compiler ABI is not available"))?;
    let storage_ty = abi
        .storage
        .storage_at_deployment_ty_idx
        .or(abi.storage.storage_ty_idx)
        .ok_or_else(|| anyhow!("contract ABI does not declare storage"))?;
    let mut parser = cell.cell.as_slice_allow_exotic();
    let decoded = dynamic_unpack::unpack_from_slice(&mut parser, abi, storage_ty)
        .context("failed to decode storage with compiler ABI")?;
    if parser.size_bits() != 0 || parser.size_refs() != 0 {
        anyhow::bail!(
            "storage has {} extra bits and {} extra refs after ABI decode",
            parser.size_bits(),
            parser.size_refs()
        );
    }
    Ok(compiler_data_to_json(&decoded))
}

fn try_decode_input_body(
    cell: &TsaExportedCell,
    input_type: Option<&str>,
    contract_contexts: &[TsaContractContext],
) -> Vec<TsaDecodedMessage> {
    let mut decoded_messages = Vec::new();

    for contract in contract_contexts {
        let Some(abi) = &contract.compiler_abi else {
            continue;
        };

        let mut candidates = Vec::new();
        match input_type {
            Some("recvExternalInput") => {
                candidates.extend(
                    abi.incoming_external
                        .iter()
                        .map(|message| message.body_ty_idx),
                );
            }
            Some("recvInternalInput") => {
                candidates.extend(
                    abi.incoming_messages
                        .iter()
                        .map(|message| message.body_ty_idx),
                );
            }
            _ => {
                candidates.extend(
                    abi.incoming_external
                        .iter()
                        .map(|message| message.body_ty_idx),
                );
                candidates.extend(
                    abi.incoming_messages
                        .iter()
                        .map(|message| message.body_ty_idx),
                );
            }
        }

        for body_ty in candidates {
            let mut parser = cell.cell.as_slice_allow_exotic();
            let Ok(data) = dynamic_unpack::unpack_from_slice(&mut parser, abi, body_ty) else {
                continue;
            };
            if parser.size_bits() != 0 || parser.size_refs() != 0 {
                continue;
            }

            decoded_messages.push(TsaDecodedMessage {
                contract_label: contract.label(),
                body_name: render_ty(abi, body_ty),
                data: compiler_data_to_json(&data),
            });
        }
    }

    decoded_messages
}

fn compiler_data_to_json(data: &UnpackedValue) -> Value {
    match data {
        UnpackedValue::Null | UnpackedValue::Void | UnpackedValue::AddressNone => Value::Null,
        UnpackedValue::Number(value) => Value::String(value.to_string()),
        UnpackedValue::Bool(value) => Value::Bool(*value),
        UnpackedValue::String(value) => Value::String(value.clone()),
        UnpackedValue::Address(value) => Value::String(value.to_string()),
        UnpackedValue::ExtAddress(value) => serde_json::json!({
            "bits": value.data_bit_len,
            "hex": hex::encode(&value.data),
        }),
        UnpackedValue::Cell(value) | UnpackedValue::RemainingBitsAndRefs(value) => {
            serde_json::json!({
                "boc64": Boc::encode_base64(value.clone()),
            })
        }
        UnpackedValue::Bits((bytes, bit_len)) => serde_json::json!({
            "bits": bit_len,
            "hex": hex::encode(bytes),
        }),
        UnpackedValue::Array(values) => {
            Value::Array(values.iter().map(compiler_data_to_json).collect())
        }
        UnpackedValue::Map(values) => Value::Array(
            values
                .iter()
                .map(|(key, value)| {
                    serde_json::json!({
                        "key": compiler_data_to_json(key),
                        "value": compiler_data_to_json(value),
                    })
                })
                .collect(),
        ),
        UnpackedValue::Object { fields, .. } => {
            let mut result = serde_json::Map::new();
            for (name, value) in fields {
                result.insert(name.clone(), compiler_data_to_json(value));
            }
            Value::Object(result)
        }
    }
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
