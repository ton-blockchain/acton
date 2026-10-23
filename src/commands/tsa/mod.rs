use crate::commands::build::{BuildCommandOptions, build_cmd};
use crate::commands::common::error_fmt;
use crate::commands::compile::compile_cmd;
use crate::stdlib;
use acton_config::config::{ActonConfig, project_root as configured_project_root};
use anyhow::Context;
use clap::Subcommand;
use rston::boc::Boc;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

use abi::load_contract_contexts;
use install::resolve_tsa_jar;
use report::print_tsa_summary;

mod abi;
mod install;
mod report;

/// Custom checker commands that compile project contracts and invoke an external TSA CLI.
/// A user-level TSA JAR is installed on demand; checker intrinsics are executed by TSA only.
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
            help = "Use a TSA JAR instead of the Acton-managed version (or set ACTON_TSA_JAR)"
        )]
        tsa_cli: Option<String>,
        #[arg(long, help = "Download the Acton-managed TSA JAR without prompting")]
        yes: bool,
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
            yes,
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
            yes,
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
    yes: bool,
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
    let checker_path = resolve_existing_path(checker)?;
    let scheme_path = scheme.as_deref().map(resolve_existing_path).transpose()?;
    let output_path = output.map(PathBuf::from);
    let requested_exported_inputs_path = exported_inputs.map(PathBuf::from);

    ensure_contracts_exist(&config, &contracts)?;
    let tsa_cli_path = resolve_tsa_jar(tsa_cli.as_deref(), yes)?;
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
