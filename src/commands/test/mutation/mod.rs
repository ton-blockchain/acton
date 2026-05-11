use crate::commands::build::{
    contract_compilation_order, generate_dependency_files, resolve_build_output_dir,
};
use crate::commands::common::{error_fmt, shell_quote};
use crate::commands::test::mutation::diff::collect_mutation_diff_scope;
use crate::commands::test::mutation::rules::{
    MutationEdit, MutationMatcher, MutationRule, load_custom_rules, merge_rules, rules,
};
use crate::commands::test::mutation::session::{
    MutationRecord, MutationSessionEvent, MutationStatus, append_mutation_session_event,
    load_or_create_mutation_session, mutation_summary,
};
use crate::commands::test::{INTERNAL_REQUIRE_TESTS_ENV, INTERNAL_SKIP_BUILD_ENV, TestConfig};
use acton_config::color::{OwoColorize, colors_enabled};
use acton_config::config::{
    ActonConfig, ContractConfig, manifest_path as configured_manifest_path,
    project_root as configured_project_root,
};
use anyhow::anyhow;
use crossbeam_channel::{Receiver, Sender, unbounded};
use path_absolutize::Absolutize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use std::{fs, process};
use tempfile::TempDir;
use tree_sitter::{Node, Point, Query, QueryCursor, StreamingIterator};
use walkdir::WalkDir;

mod diff;
mod rules;
mod session;

static MUTATION_INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone)]
struct MutationCandidate<'a> {
    rule: MutationRule,
    node: Node<'a>,
}

struct MutationSource {
    path: PathBuf,
    relative_path: PathBuf,
    content: String,
    tree: tree_sitter::Tree,
}

#[derive(Clone)]
struct MutationSourceSnapshot {
    relative_path: PathBuf,
    content: String,
}

#[derive(Clone, Copy)]
struct MutationSpan {
    start_byte: usize,
    end_byte: usize,
    start_position: Point,
    end_position: Point,
}

impl MutationSpan {
    fn from_node(node: &Node) -> Self {
        Self {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start_position: node.start_position(),
            end_position: node.end_position(),
        }
    }
}

struct GlobalMutation {
    id: usize,
    rule: MutationRule,
    span: MutationSpan,
    source_index: usize,
}

fn remove_span_from_source(source: &str, span: MutationSpan) -> String {
    let mut new_content = String::new();

    let mut line_start_byte = span.start_byte;
    while line_start_byte > 0 && source.as_bytes()[line_start_byte - 1] != b'\n' {
        line_start_byte -= 1;
    }

    let mut line_end_byte = span.end_byte;
    while line_end_byte < source.len() && source.as_bytes()[line_end_byte] != b'\n' {
        line_end_byte += 1;
    }
    if line_end_byte < source.len() && source.as_bytes()[line_end_byte] == b'\n' {
        line_end_byte += 1;
    }

    new_content.push_str(&source[..line_start_byte]);
    new_content.push_str(&source[line_end_byte..]);

    new_content
}

fn replace_span_in_source(source: &str, span: MutationSpan, replacement: &str) -> String {
    let mut new_content = String::with_capacity(source.len() + replacement.len());
    new_content.push_str(&source[..span.start_byte]);
    new_content.push_str(replacement);
    new_content.push_str(&source[span.end_byte..]);
    new_content
}

fn get_code_context(
    source: &str,
    span: MutationSpan,
    rule: &MutationRule,
    context_lines: usize,
) -> String {
    let start_line = span.start_position.row;
    let end_line = span.end_position.row;

    let lines: Vec<&str> = source.lines().collect();
    let context_start = start_line.saturating_sub(context_lines);
    let context_end = (end_line + context_lines + 1).min(lines.len());

    let mut output = String::new();
    for line_idx in context_start..context_end {
        let line = lines.get(line_idx).unwrap_or(&"");
        let line_num = line_idx + 1;

        if line_idx >= start_line && line_idx <= end_line {
            match &rule.edit {
                MutationEdit::Remove => {
                    let _ = writeln!(
                        output,
                        "  {} {} {}",
                        format!("{line_num:4}").dimmed(),
                        "│".red(),
                        line.red().strikethrough()
                    );
                }
                MutationEdit::Replace { replacement } => {
                    let start_col = if line_idx == start_line {
                        span.start_position.column
                    } else {
                        0
                    };
                    let end_col = if line_idx == end_line {
                        span.end_position.column
                    } else {
                        line.len()
                    };

                    // Clamp indices to line length to be safe
                    let start_col = start_col.min(line.len());
                    let end_col = end_col.min(line.len());

                    let prefix = &line[..start_col];
                    let matched = &line[start_col..end_col];
                    let suffix = &line[end_col..];

                    let mut line_content = String::new();
                    line_content.push_str(&prefix.dimmed().to_string());
                    line_content.push_str(&matched.red().strikethrough().to_string());
                    line_content.push_str(&suffix.dimmed().to_string());

                    let _ = writeln!(
                        output,
                        "  {} {} {}",
                        format!("{line_num:4}").dimmed(),
                        "│".dimmed(),
                        line_content
                    );

                    if line_idx == end_line {
                        let padding: String = prefix
                            .chars()
                            .map(|c| if c.is_whitespace() { c } else { ' ' })
                            .collect();

                        let _ = writeln!(
                            output,
                            "       {} {}{}",
                            "│".dimmed(),
                            padding,
                            replacement.green().bold()
                        );
                    }
                }
            }
        } else {
            let _ = writeln!(
                output,
                "  {} {} {}",
                format!("{line_num:4}").dimmed(),
                "│".dimmed(),
                line.dimmed()
            );
        }
    }
    output
}

fn collect_mutations<'a>(
    root: Node<'a>,
    source: &str,
    rules: &[MutationRule],
) -> anyhow::Result<Vec<MutationCandidate<'a>>> {
    let mut candidates = Vec::new();

    for rule in rules {
        match &rule.matcher {
            MutationMatcher::Query { query, capture } => {
                let query = Query::new(&tolk_syntax::language(), query).map_err(|e| {
                    anyhow!("Failed to create query for rule {}: {:?}", rule.name, e)
                })?;

                let mut cursor = QueryCursor::new();
                let matches = cursor.matches(&query, root, source.as_bytes());

                matches.for_each(|m| {
                    for capture_match in m.captures {
                        let capture_name = query
                            .capture_names()
                            .get(capture_match.index as usize)
                            .map_or("", AsRef::as_ref);

                        if capture_name != *capture {
                            continue;
                        }

                        candidates.push(MutationCandidate {
                            rule: rule.clone(),
                            node: capture_match.node,
                        });
                    }
                });
            }
            MutationMatcher::Callback { predicate } => {
                let mut stack = vec![root];
                while let Some(node) = stack.pop() {
                    if predicate(node, source)? {
                        candidates.push(MutationCandidate {
                            rule: rule.clone(),
                            node,
                        });
                    }

                    for idx in 0..node.child_count() {
                        if let Some(child) = node.child(idx) {
                            stack.push(child);
                        }
                    }
                }
            }
        }
    }

    Ok(candidates)
}

fn mutation_worker_count(config: &TestConfig, total_mutations: usize) -> usize {
    let available = thread::available_parallelism()
        .map(std::num::NonZero::get)
        .unwrap_or(1);
    let configured = config.mutation_workers.unwrap_or(available);

    configured.max(1).min(total_mutations.max(1))
}

fn should_copy_project_source_dir(entry_name: &str) -> bool {
    !matches!(entry_name, ".git" | "build" | "node_modules" | "target")
}

fn copy_project_tolk_sources(project_root: &Path, workspace_path: &Path) -> anyhow::Result<()> {
    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|entry| {
            entry
                .file_name()
                .to_str()
                .is_none_or(should_copy_project_source_dir)
        })
    {
        let entry = entry?;
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|ext| ext != "tolk")
        {
            continue;
        }

        let relative_path = entry.path().strip_prefix(project_root)?;
        let dest_path = workspace_path.join(relative_path);
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(entry.path(), dest_path)?;
    }

    Ok(())
}

fn create_mutation_workspace(
    project_root: &Path,
    sources: &[MutationSourceSnapshot],
) -> anyhow::Result<TempDir> {
    let mutation_dir = TempDir::new()?;
    fs::copy(
        configured_manifest_path(),
        mutation_dir.path().join("Acton.toml"),
    )?;
    copy_project_tolk_sources(project_root, mutation_dir.path())?;

    for source in sources {
        let dest_path = mutation_dir.path().join(&source.relative_path);
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest_path, &source.content)?;
    }

    Ok(mutation_dir)
}

fn dependent_override_order(
    contracts: &BTreeMap<String, ContractConfig>,
    compilation_order: &[String],
    mutate_contract: &str,
) -> Vec<String> {
    let mut affected = HashSet::<String>::from([mutate_contract.to_owned()]);
    let mut dependents = Vec::new();

    for contract_id in compilation_order {
        if contract_id == mutate_contract {
            continue;
        }

        let Some(contract) = contracts.get(contract_id) else {
            continue;
        };
        if contract
            .dependency_names()
            .iter()
            .any(|dependency| affected.contains(*dependency))
        {
            affected.insert(contract_id.clone());
            dependents.push(contract_id.clone());
        }
    }

    dependents
}

fn build_artifacts_dir(acton_config: &ActonConfig, project_root: &Path) -> PathBuf {
    let out_dir = acton_config
        .build
        .as_ref()
        .and_then(|build| build.out_dir.clone());

    resolve_build_output_dir(None, out_dir, "build", project_root)
}

fn generated_dependencies_dir(acton_config: &ActonConfig, project_root: &Path) -> PathBuf {
    let gen_dir = acton_config
        .build
        .as_ref()
        .and_then(|build| build.gen_dir.clone());

    resolve_build_output_dir(None, gen_dir, "gen", project_root)
}

fn load_original_contract_bocs(
    acton_config: &ActonConfig,
    project_root: &Path,
    compilation_order: &[String],
) -> anyhow::Result<HashMap<String, String>> {
    let build_dir = build_artifacts_dir(acton_config, project_root);
    let mut compiled_contracts = HashMap::new();

    for contract_id in compilation_order {
        let artifact_path = build_dir.join(format!("{contract_id}.json"));
        let artifact = fs::read_to_string(&artifact_path)
            .map_err(|err| anyhow!("failed to read {}: {err}", artifact_path.display()))?;
        let artifact: Value = serde_json::from_str(&artifact)
            .map_err(|err| anyhow!("failed to parse {}: {err}", artifact_path.display()))?;
        let code_boc64 = artifact
            .get("code_boc64")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("{} does not contain code_boc64", artifact_path.display()))?;

        compiled_contracts.insert(contract_id.clone(), code_boc64.to_owned());
    }

    Ok(compiled_contracts)
}

fn workspace_contract_source_path(
    workspace_path: &Path,
    project_root: &Path,
    contract: &ContractConfig,
) -> PathBuf {
    let source_path = contract.absolute_source_path(project_root);
    if let Ok(relative_path) = source_path.strip_prefix(project_root) {
        workspace_path.join(relative_path)
    } else {
        source_path
    }
}

fn format_mutation_overrides_arg(overrides: &BTreeMap<String, String>) -> String {
    overrides
        .iter()
        .map(|(contract, code_boc64)| format!("{contract}:{code_boc64}"))
        .collect::<Vec<_>>()
        .join(",")
}

type MutationExecutionResult = anyhow::Result<MutationExecution>;

enum MutationExecution {
    Completed { record: MutationRecord },
    Interrupted,
}

struct MutationRunContext<'a> {
    project_root: &'a Path,
    sources: &'a [MutationSourceSnapshot],
    mutate_contract: &'a str,
    contracts: &'a BTreeMap<String, ContractConfig>,
    dependent_override_order: &'a [String],
    original_contract_bocs: &'a HashMap<String, String>,
    acton_config: &'a ActonConfig,
    path: Option<&'a str>,
    config: &'a TestConfig,
    skip_build_for_child_tests: bool,
}

fn mutation_record(
    mutation: &GlobalMutation,
    source: &MutationSourceSnapshot,
    status: MutationStatus,
) -> MutationRecord {
    let pos = mutation.span.start_position;
    MutationRecord {
        id: mutation.id,
        rule_name: mutation.rule.name.clone(),
        rule_description: mutation.rule.description.clone(),
        rule_level: mutation.rule.level.as_str().to_owned(),
        rule_group: mutation.rule.group.clone(),
        rule_explanation: mutation.rule.explanation.clone(),
        line: pos.row + 1,
        column: pos.column + 1,
        source_path: source.relative_path.to_string_lossy().to_string(),
        code_context: get_code_context(&source.content, mutation.span, &mutation.rule, 2),
        status,
    }
}

fn run_single_mutation(
    workspace_path: &Path,
    gen_dir: &Path,
    mutation: &GlobalMutation,
    context: &MutationRunContext<'_>,
) -> anyhow::Result<MutationExecution> {
    if mutation_interrupted() {
        return Ok(MutationExecution::Interrupted);
    }

    let source = &context.sources[mutation.source_index];
    let dest_path = workspace_path.join(&source.relative_path);

    // apply mutation
    let mutated_content = match &mutation.rule.edit {
        MutationEdit::Remove => remove_span_from_source(&source.content, mutation.span),
        MutationEdit::Replace { replacement } => {
            replace_span_in_source(&source.content, mutation.span, replacement)
        }
    };

    let result = (|| -> anyhow::Result<MutationExecution> {
        fs::write(&dest_path, &mutated_content)?;

        let compile_workspace_contract =
            |contract: &ContractConfig| -> anyhow::Result<Option<String>> {
                let contract_source_path =
                    workspace_contract_source_path(workspace_path, context.project_root, contract);
                compile_file(workspace_path, &contract_source_path.to_string_lossy())
            };

        let mut compiled_contracts = context.original_contract_bocs.clone();
        let mut mutation_overrides = BTreeMap::new();

        let mutate_contract_config =
            context
                .contracts
                .get(context.mutate_contract)
                .ok_or_else(|| {
                    anyhow!(
                        "Contract '{}' not found in Acton.toml",
                        context.mutate_contract
                    )
                })?;
        let Some(code_b64) = compile_workspace_contract(mutate_contract_config)? else {
            return Ok(MutationExecution::Interrupted);
        };
        if code_b64.is_empty() {
            let record = mutation_record(mutation, source, MutationStatus::CompileError);
            return Ok(MutationExecution::Completed { record });
        }
        compiled_contracts.insert(context.mutate_contract.to_owned(), code_b64.clone());
        mutation_overrides.insert(context.mutate_contract.to_owned(), code_b64);

        for dependent_contract in context.dependent_override_order {
            let contract_config = context.contracts.get(dependent_contract).ok_or_else(|| {
                anyhow!("Contract '{dependent_contract}' not found in Acton.toml")
            })?;
            generate_dependency_files(
                dependent_contract,
                contract_config,
                &compiled_contracts,
                &BTreeMap::new(),
                context.acton_config,
                gen_dir,
                workspace_path,
            )?;

            let Some(code_b64) = compile_workspace_contract(contract_config)? else {
                return Ok(MutationExecution::Interrupted);
            };
            if code_b64.is_empty() {
                let record = mutation_record(mutation, source, MutationStatus::CompileError);
                return Ok(MutationExecution::Completed { record });
            }

            compiled_contracts.insert(dependent_contract.clone(), code_b64.clone());
            mutation_overrides.insert(dependent_contract.clone(), code_b64);
        }

        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("acton"));
        let mut cmd = process::Command::new(exe);
        append_mutation_test_command_args(&mut cmd, context.path, context.config);
        cmd.arg("--mutate-overrides")
            .arg(format_mutation_overrides_arg(&mutation_overrides));

        if context.skip_build_for_child_tests {
            cmd.env(INTERNAL_SKIP_BUILD_ENV, "1");
        }

        let output = match run_command_output_interruptible(&mut cmd)? {
            InterruptibleOutput::Completed(output) => output,
            InterruptibleOutput::Interrupted => return Ok(MutationExecution::Interrupted),
        };

        let survived = output.status.success();
        let status = if survived {
            MutationStatus::Survived
        } else {
            MutationStatus::Killed
        };

        let record = mutation_record(mutation, source, status);

        Ok(MutationExecution::Completed { record })
    })();

    let restore_result = fs::write(&dest_path, &source.content);
    match (result, restore_result) {
        (Ok(execution), Ok(())) => Ok(execution),
        (Ok(_), Err(err)) => Err(err.into()),
        (Err(err), Ok(()) | Err(_)) => Err(err),
    }
}

fn mutation_worker_loop(
    job_rx: Receiver<GlobalMutation>,
    result_tx: Sender<MutationExecutionResult>,
    context: MutationRunContext<'_>,
) -> anyhow::Result<()> {
    let workspace = match create_mutation_workspace(context.project_root, context.sources) {
        Ok(workspace) => workspace,
        Err(err) => {
            let _ = result_tx.send(Err(err));
            return Ok(());
        }
    };

    let gen_dir = generated_dependencies_dir(context.acton_config, workspace.path());

    while let Ok(mutation) = job_rx.recv() {
        let execution = match run_single_mutation(workspace.path(), &gen_dir, &mutation, &context) {
            Ok(execution) => execution,
            Err(err) => {
                let _ = result_tx.send(Err(err));
                return Ok(());
            }
        };

        let interrupted = matches!(execution, MutationExecution::Interrupted);
        if result_tx.send(Ok(execution)).is_err() {
            return Ok(());
        }
        if interrupted || mutation_interrupted() {
            return Ok(());
        }
    }

    Ok(())
}

fn print_mutation_status_line(record: &MutationRecord, available_mutation_count: usize) {
    print!(
        "  {} Mutation {}/{} ",
        "◉".cyan(),
        record.id.to_string().bright_white(),
        available_mutation_count
    );
    print!(
        "{} ",
        format!("{}:{}:{}", record.source_path, record.line, record.column).dimmed(),
    );
    print!("{} ", record.rule_description.dimmed());
    println!("{}", mutation_status_label(record.status));
}

fn flush_pending_mutation_outputs(
    pending_outputs: &mut BTreeMap<usize, MutationRecord>,
    ordered_ids: &[usize],
    next_output_index: &mut usize,
    current_records: &mut Vec<MutationRecord>,
    available_mutation_count: usize,
    flush_all: bool,
) {
    while *next_output_index < ordered_ids.len() {
        let next_id = ordered_ids[*next_output_index];
        if let Some(record) = pending_outputs.remove(&next_id) {
            print_mutation_status_line(&record, available_mutation_count);
            current_records.push(record);
            *next_output_index += 1;
            continue;
        }

        if flush_all {
            *next_output_index += 1;
            continue;
        }

        break;
    }
}

fn mutation_status_label(status: MutationStatus) -> String {
    match status {
        MutationStatus::Killed => "KILLED".green().to_string(),
        MutationStatus::Survived => "SURVIVED".red().bold().to_string(),
        MutationStatus::CompileError => "COMPILE ERROR".yellow().bold().to_string(),
    }
}

fn format_rule_level(level: &str) -> String {
    match level {
        "critical" => level.red().bold().to_string(),
        "major" => level.yellow().bold().to_string(),
        "minor" => level.blue().bold().to_string(),
        _ => level.to_owned(),
    }
}

enum InterruptibleOutput {
    Completed(process::Output),
    Interrupted,
}

fn install_mutation_interrupt_handler() -> anyhow::Result<()> {
    ctrlc::set_handler(|| {
        MUTATION_INTERRUPTED.store(true, Ordering::SeqCst);
    })?;

    MUTATION_INTERRUPTED.store(false, Ordering::SeqCst);
    Ok(())
}

fn mutation_interrupted() -> bool {
    MUTATION_INTERRUPTED.load(Ordering::SeqCst)
}

#[cfg(unix)]
fn send_interrupt(child: &process::Child) {
    let _ = process::Command::new("kill")
        .arg("-INT")
        .arg(child.id().to_string())
        .status();
}

#[cfg(not(unix))]
fn send_interrupt(child: &mut process::Child) {
    let _ = child.kill();
}

fn run_command_output_interruptible(
    cmd: &mut process::Command,
) -> anyhow::Result<InterruptibleOutput> {
    if mutation_interrupted() {
        return Ok(InterruptibleOutput::Interrupted);
    }

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    loop {
        if mutation_interrupted() {
            send_interrupt(&child);

            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                match child.try_wait()? {
                    Some(_) => break,
                    None if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
                    None => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                }
            }

            return Ok(InterruptibleOutput::Interrupted);
        }

        if let Some(status) = child.try_wait()? {
            let mut stdout = Vec::new();
            if let Some(mut pipe) = child.stdout.take() {
                let _ = pipe.read_to_end(&mut stdout);
            }

            let mut stderr = Vec::new();
            if let Some(mut pipe) = child.stderr.take() {
                let _ = pipe.read_to_end(&mut stderr);
            }

            return Ok(InterruptibleOutput::Completed(process::Output {
                status,
                stdout,
                stderr,
            }));
        }

        thread::sleep(Duration::from_millis(25));
    }
}

fn append_mutation_test_command_args(
    cmd: &mut process::Command,
    path: Option<&str>,
    config: &TestConfig,
) {
    let test_path = match path {
        Some(path) => Path::new(path),
        None => configured_project_root(),
    };

    cmd.arg("--project-root")
        .arg(configured_project_root())
        .arg("--color")
        .arg(if colors_enabled() { "always" } else { "never" })
        .arg("test")
        .arg(test_path)
        .arg("--fail-fast")
        .arg("--reporter")
        .arg("console");

    if let Some(filter) = &config.filter {
        cmd.arg("--filter").arg(filter);
    }

    for exclude in &config.exclude_patterns {
        cmd.arg("--exclude").arg(exclude);
    }

    for include in &config.include_patterns {
        cmd.arg("--include").arg(include);
    }

    if let Some(fork_net) = &config.fork_net {
        cmd.arg("--fork-net").arg(fork_net_cli_arg(fork_net));
    }

    if let Some(fork_block_number) = config.fork_block_number {
        cmd.arg("--fork-block-number")
            .arg(fork_block_number.to_string());
    }

    if let Some(fuzz_seed) = config.fuzz_seed {
        cmd.arg("--fuzz-seed").arg(fuzz_seed.to_string());
    }
}

fn command_output_details(output: &process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("exit status {}", output.status)
    }
}

fn mutation_resume_command(path: Option<&str>, config: &TestConfig, session_id: &str) -> String {
    let mut args = vec!["acton".to_owned(), "test".to_owned()];

    if let Some(path) = path {
        args.push(shell_quote(path));
    }

    args.push("--mutate".to_owned());

    if let Some(contract) = &config.mutate_contract {
        args.push("--mutate-contract".to_owned());
        args.push(shell_quote(contract));
    }

    args.push("--mutation-session-id".to_owned());
    args.push(shell_quote(session_id));

    if let Some(workers) = config.mutation_workers {
        args.push("--mutation-workers".to_owned());
        args.push(workers.to_string());
    }

    if let Some(diff) = config.mutation_diff {
        args.push("--mutation-diff".to_owned());
        args.push(diff.to_string());
    }

    if let Some(diff_ref) = &config.mutation_diff_ref {
        args.push("--mutation-diff-ref".to_owned());
        args.push(shell_quote(diff_ref));
    }

    if let Some(fork_net) = &config.fork_net {
        args.push("--fork-net".to_owned());
        args.push(shell_quote(&fork_net_cli_arg(fork_net)));
    }

    if let Some(fork_block_number) = config.fork_block_number {
        args.push("--fork-block-number".to_owned());
        args.push(fork_block_number.to_string());
    }

    if !config.mutation_levels.is_empty() {
        args.push("--mutation-levels".to_owned());
        args.push(
            config
                .mutation_levels
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    if !config.mutation_ids.is_empty() {
        let mut ids = config.mutation_ids.clone();
        ids.sort_unstable();
        ids.dedup();
        args.push("--mutation-id".to_owned());
        args.push(
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    if let Some(minimum_percent) = config.mutation_minimum_percent {
        args.push("--mutation-minimum-percent".to_owned());
        args.push(minimum_percent.to_string());
    }

    for rule in &config.disable_rules {
        args.push("--mutation-disable-rules".to_owned());
        args.push(shell_quote(rule));
    }

    if let Some(filter) = &config.filter {
        args.push("--filter".to_owned());
        args.push(shell_quote(filter));
    }

    for include in &config.include_patterns {
        args.push("--include".to_owned());
        args.push(shell_quote(include));
    }

    for exclude in &config.exclude_patterns {
        args.push("--exclude".to_owned());
        args.push(shell_quote(exclude));
    }

    if config.clear_cache {
        args.push("--clear-cache".to_owned());
    }

    args.join(" ")
}

fn fork_net_cli_arg(network: &acton_config::config::Network) -> String {
    match network {
        acton_config::config::Network::Custom(name) => format!("custom:{name}"),
        _ => network.to_string(),
    }
}

fn exit_mutation_interrupted(
    path: Option<&str>,
    config: &TestConfig,
    session_id: Option<&str>,
) -> ! {
    println!();
    println!();
    println!("{}", "Interrupted by Ctrl+C.".yellow().bold());
    if let Some(session_id) = session_id {
        println!(
            "Mutation session {} was left unfinished and can be resumed.",
            session_id.bright_cyan()
        );
        println!("Resume with:");
        println!(
            "  {}",
            mutation_resume_command(path, config, session_id).bright_white()
        );
    } else {
        println!(
            "Mutation session has not been initialized yet. Re-run the mutation command to start a new session."
        );
    }
    process::exit(130);
}

fn prepare_project_for_mutation(config: &TestConfig) -> anyhow::Result<()> {
    // Ensure generated dependency files (e.g. gen/*_code.tolk) exist before collecting
    // file dependencies and compiling mutants.
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("acton"));
    let mut cmd = process::Command::new(exe);
    cmd.arg("build");
    if config.clear_cache {
        cmd.arg("--clear-cache");
    }

    let output = match run_command_output_interruptible(&mut cmd)? {
        InterruptibleOutput::Completed(output) => output,
        InterruptibleOutput::Interrupted => return Ok(()),
    };
    if output.status.success() {
        return Ok(());
    }

    let details = command_output_details(&output);

    anyhow::bail!("Failed to prepare project for mutation testing: {details}");
}

fn run_mutation_baseline_tests(path: Option<&str>, config: &TestConfig) -> anyhow::Result<()> {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("acton"));
    let mut cmd = process::Command::new(exe);
    append_mutation_test_command_args(&mut cmd, path, config);
    cmd.env(INTERNAL_SKIP_BUILD_ENV, "1");
    cmd.env(INTERNAL_REQUIRE_TESTS_ENV, "1");

    let output = match run_command_output_interruptible(&mut cmd)? {
        InterruptibleOutput::Completed(output) => output,
        InterruptibleOutput::Interrupted => return Ok(()),
    };

    if output.status.success() {
        return Ok(());
    }

    let details = command_output_details(&output);
    anyhow::bail!(
        "Baseline test suite failed before mutation testing. Fix failing tests or adjust the mutation test selection.\n\n{details}"
    );
}

pub fn test_mutate_cmd(path: Option<&str>, config: &TestConfig) -> anyhow::Result<()> {
    install_mutation_interrupt_handler()?;

    let Some(mutate_contract) = &config.mutate_contract else {
        anyhow::bail!(
            "Provide {} {} to choose which contract to mutate",
            "--mutate-contract".yellow(),
            "<CONTRACT_NAME>".yellow()
        )
    };
    if let Some(minimum_percent) = config.mutation_minimum_percent
        && (!minimum_percent.is_finite() || !(0.0..=100.0).contains(&minimum_percent))
    {
        anyhow::bail!("mutation minimum percent must be between 0 and 100, got {minimum_percent}");
    }
    let acton_config = ActonConfig::load()?;
    let contract = acton_config.get_contract(mutate_contract).ok_or_else(|| {
        anyhow!(error_fmt::contract_not_found(
            &acton_config,
            mutate_contract
        ))
    })?;
    let Some(contracts) = acton_config.contracts() else {
        anyhow::bail!("No contracts section found in Acton.toml");
    };
    let compilation_order = contract_compilation_order(contracts)?;
    let dependent_override_order =
        dependent_override_order(contracts, &compilation_order, mutate_contract);

    let project_root = dunce::canonicalize(configured_project_root())
        .unwrap_or_else(|_| configured_project_root().to_path_buf());
    let mutation_diff_scope = collect_mutation_diff_scope(&project_root, config)?;

    prepare_project_for_mutation(config)?;
    if mutation_interrupted() {
        exit_mutation_interrupted(path, config, None);
    }
    run_mutation_baseline_tests(path, config)?;
    if mutation_interrupted() {
        exit_mutation_interrupted(path, config, None);
    }
    let original_contract_bocs =
        load_original_contract_bocs(&acton_config, &project_root, &compilation_order)?;

    let all_disable_rules = &config.disable_rules;
    let selected_mutation_levels = &config.mutation_levels;

    let mut sources = Vec::new();

    let main_path = contract.absolute_source_path(&project_root);
    let main_path = dunce::canonicalize(&main_path).unwrap_or(main_path);

    let main_content = match fs::read_to_string(&main_path) {
        Ok(content) => content,
        Err(err) => {
            anyhow::bail!("Error reading file '{}': {err}", main_path.display())
        }
    };
    let main_tree = tolk_syntax::parse(&main_content)?;

    let main_relative_path = if main_path.starts_with(&project_root) {
        pathdiff::diff_paths(&main_path, &project_root).unwrap_or_else(|| main_path.clone())
    } else {
        main_path.clone()
    };
    sources.push(MutationSource {
        path: main_path,
        relative_path: main_relative_path,
        content: main_content,
        tree: main_tree.tree,
    });

    let mappings = acton_config.mappings();
    let compiler = tolk_compiler::Compiler::new(0).with_mappings(&mappings);
    let source_map = match compiler.compile(&sources[0].path, false) {
        tolk_compiler::CompilerResult::Success(result) => result
            .source_map
            .ok_or_else(|| anyhow!("Compiler did not produce symbol types for mutation testing"))?,
        tolk_compiler::CompilerResult::Error(error) => {
            anyhow::bail!(
                "Failed to collect source files for mutation testing: {}",
                error.message
            )
        }
    };
    for source_file in source_map.files() {
        let dep_path_str = source_file.file_name.as_str();
        if dep_path_str.starts_with("@stdlib/") || dep_path_str.starts_with("@fiftlib/") {
            continue;
        }

        let dep_path = Path::new(dep_path_str)
            .absolutize_from(&project_root)
            .unwrap_or_else(|_| Path::new(dep_path_str).into())
            .to_path_buf();
        let dep_path = dunce::canonicalize(&dep_path).unwrap_or(dep_path);

        if !dep_path.starts_with(&project_root) {
            continue;
        }

        if sources.iter().any(|s| s.path == dep_path) {
            continue;
        }

        let relative_path =
            pathdiff::diff_paths(&dep_path, &project_root).unwrap_or_else(|| dep_path.clone());
        let content = fs::read_to_string(&dep_path)
            .map_err(|e| anyhow!("Error reading dependency {}: {}", dep_path.display(), e))?;
        let file = tolk_syntax::parse(&content)?;

        sources.push(MutationSource {
            path: dep_path,
            relative_path,
            content,
            tree: file.tree,
        });
    }

    let mutation_rules = if let Some(custom_rules_file) = &config.mutation_rules_file {
        merge_rules(
            rules(),
            load_custom_rules(&project_root, custom_rules_file)?,
        )
    } else {
        rules()
    };
    let filtered_rules: Vec<MutationRule> = mutation_rules
        .into_iter()
        .filter(|rule| !all_disable_rules.contains(&rule.name))
        .filter(|rule| {
            selected_mutation_levels.is_empty() || selected_mutation_levels.contains(&rule.level)
        })
        .collect();

    let mut mutations = Vec::new();
    for (idx, source) in sources.iter().enumerate() {
        let candidates =
            collect_mutations(source.tree.root_node(), &source.content, &filtered_rules)?;
        for candidate in candidates {
            if let Some(diff_scope) = &mutation_diff_scope
                && !diff_scope.matches_candidate(source, &candidate)
            {
                continue;
            }
            mutations.push(GlobalMutation {
                id: 0,
                rule: candidate.rule,
                span: MutationSpan::from_node(&candidate.node),
                source_index: idx,
            });
        }
    }

    for (index, mutation) in mutations.iter_mut().enumerate() {
        mutation.id = index + 1;
    }

    let available_mutation_count = mutations.len();

    if !config.mutation_ids.is_empty() {
        let requested_ids: BTreeSet<_> = config.mutation_ids.iter().copied().collect();
        let missing_ids: Vec<_> = requested_ids
            .iter()
            .copied()
            .filter(|id| *id > available_mutation_count)
            .collect();

        if !missing_ids.is_empty() {
            let missing = missing_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::bail!(
                "Unknown mutation ID(s): {missing}. Run the same mutation command without --mutation-id to list available IDs"
            );
        }

        mutations.retain(|mutation| requested_ids.contains(&mutation.id));
    }

    let selected_ids = mutations
        .iter()
        .map(|mutation| mutation.id)
        .collect::<BTreeSet<_>>();
    let session_source_path = sources[0].relative_path.to_string_lossy().to_string();
    let session = load_or_create_mutation_session(
        &project_root,
        mutate_contract,
        &session_source_path,
        &selected_ids,
        config.mutation_session_id.as_deref(),
    )?;

    let completed_ids = session
        .completed_records
        .iter()
        .map(|record| record.id)
        .collect::<BTreeSet<_>>();
    mutations.retain(|mutation| !completed_ids.contains(&mutation.id));

    if session.resumed {
        eprintln!(
            "Resuming mutation session {}: {} completed, {} remaining",
            session.session_id,
            session.completed_records.len(),
            mutations.len()
        );
    }

    println!("{}", "Mutation Testing".bold());
    println!("{}", "─".repeat(60).dimmed());
    println!("Session:  {}", session.session_id.bright_cyan());
    println!(
        "Contract: {}",
        contract.display_name(mutate_contract).bright_white()
    );
    println!("Source:   {}", contract.src.dimmed());
    if let Some(diff_scope) = &mutation_diff_scope {
        println!("Diff:     {}", diff_scope.label.bright_cyan());
        println!(
            "Changed:  {}",
            diff_scope
                .changed_source_count(&sources)
                .to_string()
                .bright_cyan()
        );
    }
    if !selected_mutation_levels.is_empty() {
        let levels = selected_mutation_levels
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        println!("Levels:   {}", levels.bright_cyan());
    }
    if !config.mutation_ids.is_empty() {
        let mut ids = config.mutation_ids.clone();
        ids.sort_unstable();
        ids.dedup();
        let ids = ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        println!("IDs:      {}", ids.bright_cyan());
    }
    println!("Files:    {}", sources.len().to_string().bright_cyan());
    println!(
        "Mutants:  {}\n",
        session.selected_ids.len().to_string().bright_cyan()
    );

    // Default behavior in mutation child test runs is to skip per-mutant rebuilds.
    // Any explicit value other than "1" turns this optimization off.
    let skip_build_for_child_tests = std::env::var("ACTON_INTERNAL_SKIP_BUILD")
        .map(|value| value.trim() == "1")
        .unwrap_or(true);
    let source_snapshots = sources
        .iter()
        .map(|source| MutationSourceSnapshot {
            relative_path: source.relative_path.clone(),
            content: source.content.clone(),
        })
        .collect::<Vec<_>>();
    let ordered_remaining_ids = mutations
        .iter()
        .map(|mutation| mutation.id)
        .collect::<Vec<_>>();

    let worker_count = mutation_worker_count(config, mutations.len());
    let mut current_records = Vec::new();
    let mut pending_outputs = BTreeMap::new();
    let mut next_output_index = 0usize;
    let mut interrupted = false;

    thread::scope(|scope| -> anyhow::Result<()> {
        let (job_tx, job_rx) = unbounded::<GlobalMutation>();
        let (result_tx, result_rx) = unbounded::<MutationExecutionResult>();

        for _ in 0..worker_count {
            let worker_job_rx = job_rx.clone();
            let worker_result_tx = result_tx.clone();
            let worker_context = MutationRunContext {
                project_root: &project_root,
                sources: &source_snapshots,
                mutate_contract,
                contracts,
                dependent_override_order: &dependent_override_order,
                original_contract_bocs: &original_contract_bocs,
                acton_config: &acton_config,
                path,
                config,
                skip_build_for_child_tests,
            };

            scope.spawn(move || {
                let _ = mutation_worker_loop(worker_job_rx, worker_result_tx, worker_context);
            });
        }

        drop(job_rx);
        drop(result_tx);

        for mutation in mutations {
            if job_tx.send(mutation).is_err() {
                anyhow::bail!("Mutation worker pool shut down before all jobs were dispatched");
            }
        }
        drop(job_tx);

        for execution in result_rx {
            match execution? {
                MutationExecution::Completed { record } => {
                    append_mutation_session_event(
                        &session.progress_path,
                        &MutationSessionEvent::MutationCompleted {
                            session_id: session.session_id.clone(),
                            record: record.clone(),
                            completed_at: session::now_rfc3339(),
                        },
                    )?;
                    pending_outputs.insert(record.id, record);
                    flush_pending_mutation_outputs(
                        &mut pending_outputs,
                        &ordered_remaining_ids,
                        &mut next_output_index,
                        &mut current_records,
                        available_mutation_count,
                        false,
                    );
                }
                MutationExecution::Interrupted => {
                    interrupted = true;
                }
            }
        }

        Ok(())
    })?;

    flush_pending_mutation_outputs(
        &mut pending_outputs,
        &ordered_remaining_ids,
        &mut next_output_index,
        &mut current_records,
        available_mutation_count,
        true,
    );

    if interrupted {
        exit_mutation_interrupted(path, config, Some(&session.session_id));
    }

    let mut all_records = session.completed_records.clone();
    all_records.extend(current_records);
    all_records.sort_unstable_by_key(|record| record.id);
    let summary = mutation_summary(&all_records);
    let mut mutation_threshold_failed = false;

    println!();

    println!(
        "  {} {:<20} {}",
        " ".dimmed(),
        "Total mutants",
        summary.total_mutants
    );

    println!(
        "  {} {:<20} {}",
        "✓".green(),
        "Killed".green(),
        summary.killed.to_string().green()
    );

    println!(
        "  {} {:<20} {}",
        "✗".red(),
        "Survived".red(),
        summary.survived.to_string().red()
    );

    println!(
        "  {} {:<20} {}",
        "!".yellow(),
        "Compile errors".yellow(),
        summary.compile_errors.to_string().yellow()
    );

    let score_str = format!("{:.1}%", summary.mutation_score);
    let (score_icon, score_label) = match summary.mutation_score as u32 {
        0..=50 => (
            "◆".red().bold().to_string(),
            "Mutation Score".red().bold().to_string(),
        ),
        51..=80 => (
            "◆".yellow().bold().to_string(),
            "Mutation Score".yellow().bold().to_string(),
        ),
        _ => (
            "◆".green().bold().to_string(),
            "Mutation Score".green().bold().to_string(),
        ),
    };

    println!(
        "\n  {} {:<20} {}",
        score_icon,
        score_label,
        if summary.mutation_score <= 50.0 {
            score_str.red().bold().to_string()
        } else if summary.mutation_score <= 80.0 {
            score_str.yellow().bold().to_string()
        } else {
            score_str.green().bold().to_string()
        }
    );

    if let Some(minimum_percent) = config.mutation_minimum_percent
        && summary.mutation_score < minimum_percent
    {
        mutation_threshold_failed = true;
        println!(
            "\n{}: mutation score {:.2}% is below the required minimum of {:.2}%.",
            "Error".red(),
            summary.mutation_score,
            minimum_percent
        );
    }

    if all_records.is_empty() {
        println!("\n{} No mutation points found.\n", "○".dimmed());
    } else if summary.survived > 0 {
        println!("\n{}", "Survived Mutants".yellow());
        println!("{}", "─".repeat(60).dimmed());

        for result in all_records.iter().filter(|r| r.status.is_survived()) {
            println!("\n  {} Mutation #{}", "✗".red().bold(), result.id);
            println!(
                "  {}  {} {}",
                "Rule:".dimmed(),
                result.rule_description,
                format!("[{}]", result.rule_name).dimmed()
            );
            println!(
                "  {} {}",
                "Level:".dimmed(),
                format_rule_level(&result.rule_level)
            );
            println!("  {} {}", "Group:".dimmed(), result.rule_group);
            println!(
                "  {} {}:{}:{}",
                "at".dimmed(),
                result.source_path.bright_white(),
                result.line.to_string().bright_white(),
                result.column
            );

            println!("{}", result.code_context);
            println!(
                "  {} {}",
                "Why it's bad:".dimmed(),
                result.rule_explanation.dimmed()
            );
        }

        println!("{}", "─".repeat(60).dimmed());
        println!("These mutants were not caught by your tests!",);
        println!("Consider adding more test cases to improve mutation coverage.\n");
    } else {
        println!(
            "\n{} {} All mutants were killed!\n",
            "✓".green().bold(),
            "Excellent!".green().bold()
        );
    }

    if !session.finished {
        append_mutation_session_event(
            &session.progress_path,
            &MutationSessionEvent::SessionFinished {
                session_id: session.session_id.clone(),
                total_mutants: summary.total_mutants,
                killed: summary.killed,
                survived: summary.survived,
                compile_errors: summary.compile_errors,
                mutation_score: summary.mutation_score,
                minimum_percent: config.mutation_minimum_percent,
                threshold_failed: mutation_threshold_failed,
                exit_code: i32::from(mutation_threshold_failed),
                finished_at: session::now_rfc3339(),
            },
        )?;
    }

    let exit_code = i32::from(mutation_threshold_failed);
    if exit_code != 0 {
        process::exit(exit_code);
    }

    Ok(())
}

fn compile_file(project_root: &Path, path: &str) -> anyhow::Result<Option<String>> {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("acton"));
    let mut cmd = process::Command::new(exe);
    let cmd = cmd
        .arg("--project-root")
        .arg(project_root)
        .arg("compile")
        .arg("--json")
        .arg(path);

    let compilation_result = match run_command_output_interruptible(cmd)? {
        InterruptibleOutput::Completed(output) => output,
        InterruptibleOutput::Interrupted => return Ok(None),
    };
    let compilation_result = String::from_utf8_lossy(&compilation_result.stdout);
    let compilation_result: Value = serde_json::from_str(compilation_result.as_ref())?;
    let Some(success) = compilation_result.get("success") else {
        anyhow::bail!("Compilation returned invalid result without `success` flag");
    };
    let success = success.as_bool().unwrap_or(false);
    if !success {
        return Ok(Some(String::new()));
    }
    let Some(code_b64) = compilation_result.get("code_boc64") else {
        anyhow::bail!("No code boc64 found in compilation result")
    };
    let Value::String(code_b64) = code_b64 else {
        anyhow::bail!("No code boc64 found in compilation result")
    };
    Ok(Some(code_b64.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use acton_config::config::Network;
    use std::sync::Arc;

    #[test]
    fn mutation_child_test_command_forwards_fork_flags() {
        let config = TestConfig {
            fork_net: Some(Network::Custom(Arc::from("remote-block"))),
            fork_block_number: Some(123_456),
            ..TestConfig::default()
        };
        let mut cmd = process::Command::new("acton");

        append_mutation_test_command_args(&mut cmd, Some("tests/fork.test.tolk"), &config);

        let args = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(
            args.windows(2)
                .any(|pair| pair[0] == "--fork-net" && pair[1] == "custom:remote-block"),
            "mutation child command must forward --fork-net, got {args:?}"
        );
        assert!(
            args.windows(2)
                .any(|pair| pair[0] == "--fork-block-number" && pair[1] == "123456"),
            "mutation child command must forward --fork-block-number, got {args:?}"
        );
    }

    #[test]
    fn mutation_resume_command_includes_fork_flags() {
        let config = TestConfig {
            fork_net: Some(Network::Custom(Arc::from("remote-block"))),
            fork_block_number: Some(123_456),
            ..TestConfig::default()
        };

        let command = mutation_resume_command(Some("tests/fork.test.tolk"), &config, "session-1");

        assert!(
            command.contains("--fork-net custom:remote-block"),
            "resume command must include parseable --fork-net, got {command}"
        );
        assert!(
            command.contains("--fork-block-number 123456"),
            "resume command must include --fork-block-number, got {command}"
        );
    }
}
