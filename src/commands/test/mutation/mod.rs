use crate::commands::common::error_fmt;
use crate::commands::test::TestConfig;
use crate::commands::test::mutation::diff::collect_mutation_diff_scope;
use crate::commands::test::mutation::rules::{MutationEdit, MutationMatcher, MutationRule, rules};
use crate::commands::test::mutation::session::{
    MutationRecord, MutationSessionEvent, MutationStatus, append_mutation_session_event,
    load_or_create_mutation_session, mutation_summary,
};
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, project_root as configured_project_root};
use anyhow::anyhow;
use path_absolutize::Absolutize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use std::{fs, process};
use tempfile::TempDir;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

mod diff;
mod rules;
mod session;

static MUTATION_INTERRUPTED: AtomicBool = AtomicBool::new(false);
static MUTATION_INTERRUPT_HANDLER: OnceLock<Result<(), String>> = OnceLock::new();

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

struct GlobalMutation<'a> {
    id: usize,
    candidate: MutationCandidate<'a>,
    source_index: usize,
}

fn remove_node_from_source(source: &str, node_to_remove: &Node) -> String {
    let start_byte = node_to_remove.start_byte();
    let end_byte = node_to_remove.end_byte();

    let mut new_content = String::new();

    let mut line_start_byte = start_byte;
    while line_start_byte > 0 && source.as_bytes()[line_start_byte - 1] != b'\n' {
        line_start_byte -= 1;
    }

    let mut line_end_byte = end_byte;
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

fn replace_node_in_source(source: &str, target: &Node, replacement: &str) -> String {
    let start = target.start_byte();
    let end = target.end_byte();

    let mut new_content = String::with_capacity(source.len() + replacement.len());
    new_content.push_str(&source[..start]);
    new_content.push_str(replacement);
    new_content.push_str(&source[end..]);
    new_content
}

fn get_code_context(
    source: &str,
    node: &Node,
    rule: &MutationRule,
    context_lines: usize,
) -> String {
    let start_line = node.start_position().row;
    let end_line = node.end_position().row;

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
                    output.push_str(&format!(
                        "  {} {} {}\n",
                        format!("{line_num:4}").dimmed(),
                        "│".red(),
                        line.red().strikethrough()
                    ));
                }
                MutationEdit::Replace { replacement } => {
                    let start_col = if line_idx == start_line {
                        node.start_position().column
                    } else {
                        0
                    };
                    let end_col = if line_idx == end_line {
                        node.end_position().column
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

                    output.push_str(&format!(
                        "  {} {} {}\n",
                        format!("{line_num:4}").dimmed(),
                        "│".dimmed(),
                        line_content
                    ));

                    if line_idx == end_line {
                        let padding: String = prefix
                            .chars()
                            .map(|c| if c.is_whitespace() { c } else { ' ' })
                            .collect();

                        output.push_str(&format!(
                            "  {} {} {}{}\n",
                            "    ",
                            "│".dimmed(),
                            padding,
                            replacement.green().bold()
                        ));
                    }
                }
            }
        } else {
            output.push_str(&format!(
                "  {} {} {}\n",
                format!("{line_num:4}").dimmed(),
                "│".dimmed(),
                line.dimmed()
            ));
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

fn apply_mutation(source: &str, candidate: &MutationCandidate) -> String {
    match candidate.rule.edit {
        MutationEdit::Remove => remove_node_from_source(source, &candidate.node),
        MutationEdit::Replace { replacement } => {
            replace_node_in_source(source, &candidate.node, replacement)
        }
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
    let result = MUTATION_INTERRUPT_HANDLER.get_or_init(|| {
        ctrlc::set_handler(|| {
            MUTATION_INTERRUPTED.store(true, Ordering::SeqCst);
        })
        .map_err(|err| err.to_string())
    });

    if let Err(err) = result {
        anyhow::bail!("Failed to install Ctrl+C handler: {err}");
    }

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

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | ':' | ','))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

fn mutation_resume_command(path: &Option<String>, config: &TestConfig, session_id: &str) -> String {
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

    if let Some(diff) = config.mutation_diff {
        args.push("--mutation-diff".to_owned());
        args.push(diff.to_string());
    }

    if let Some(diff_ref) = &config.mutation_diff_ref {
        args.push("--mutation-diff-ref".to_owned());
        args.push(shell_quote(diff_ref));
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
        args.push("--disable-rule".to_owned());
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

fn exit_mutation_interrupted(
    path: &Option<String>,
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

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let details = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("exit status {}", output.status)
    };

    anyhow::bail!("Failed to prepare project for mutation testing: {details}");
}

pub fn test_mutate_cmd(path: &Option<String>, config: &TestConfig) -> anyhow::Result<()> {
    install_mutation_interrupt_handler()?;

    let Some(mutate_contract) = &config.mutate_contract else {
        anyhow::bail!(
            "Provide {} {} to choose which contract to mutate",
            "--mutate-contract".yellow(),
            "<CONTRACT_ID>".yellow()
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

    let project_root = dunce::canonicalize(configured_project_root())
        .unwrap_or_else(|_| configured_project_root().to_path_buf());
    let mutation_diff_scope = collect_mutation_diff_scope(&project_root, config)?;

    prepare_project_for_mutation(config)?;
    if mutation_interrupted() {
        exit_mutation_interrupted(path, config, None);
    }

    let all_disable_rules = &config.disable_rules;
    let selected_mutation_levels = &config.mutation_levels;

    let mut sources = Vec::new();

    let main_path = Path::new(&contract.src)
        .absolutize_from(&project_root)
        .unwrap_or_else(|_| Path::new(&contract.src).into())
        .to_path_buf();
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
    let main_path_str = main_path.to_string_lossy().to_string();

    sources.push(MutationSource {
        path: main_path,
        relative_path: main_relative_path,
        content: main_content,
        tree: main_tree.tree,
    });

    let mappings = acton_config.mappings();
    let dependencies = ton_abi::get_file_dependencies(&main_path_str, true, &mappings)?;
    for dep_path_str in &dependencies {
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

    let mutation_dir = TempDir::new()?;

    for source in &sources {
        let dest_path = mutation_dir.path().join(&source.relative_path);
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&dest_path, &source.content)?;
    }

    let mutation_rules = rules();
    let filtered_rules: Vec<MutationRule> = mutation_rules
        .into_iter()
        .filter(|rule| !all_disable_rules.contains(&rule.name.to_string()))
        .filter(|rule| {
            selected_mutation_levels.is_empty()
                || selected_mutation_levels
                    .iter()
                    .any(|level| level.as_str() == rule.level.label())
        })
        .collect();

    let mut global_mutations = Vec::new();
    for (idx, source) in sources.iter().enumerate() {
        let candidates =
            collect_mutations(source.tree.root_node(), &source.content, &filtered_rules)?;
        for candidate in candidates {
            if let Some(diff_scope) = &mutation_diff_scope
                && !diff_scope.matches_candidate(source, &candidate)
            {
                continue;
            }
            global_mutations.push(GlobalMutation {
                id: 0,
                candidate,
                source_index: idx,
            });
        }
    }

    for (index, mutation) in global_mutations.iter_mut().enumerate() {
        mutation.id = index + 1;
    }

    let available_mutation_count = global_mutations.len();

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

        global_mutations.retain(|mutation| requested_ids.contains(&mutation.id));
    }

    let selected_ids = global_mutations
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
    global_mutations.retain(|mutation| !completed_ids.contains(&mutation.id));

    if session.resumed {
        eprintln!(
            "Resuming mutation session {}: {} completed, {} remaining",
            session.session_id,
            session.completed_records.len(),
            global_mutations.len()
        );
    }

    println!("{}", "Mutation Testing".bold());
    println!("{}", "─".repeat(60).dimmed());
    println!("Session:  {}", session.session_id.bright_cyan());
    println!("Contract: {}", contract.name.bright_white());
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

    let mut current_records = Vec::new();

    for global_mutation in &global_mutations {
        if mutation_interrupted() {
            exit_mutation_interrupted(path, config, Some(&session.session_id));
        }

        let mutation = &global_mutation.candidate;
        let source_idx = global_mutation.source_index;
        let mutation_id = global_mutation.id;
        let source = &sources[source_idx];
        let pos = mutation.node.start_position();

        print!(
            "  {} Mutation {}/{} ",
            "◉".cyan(),
            mutation_id.to_string().bright_white(),
            available_mutation_count
        );
        print!(
            "{} ",
            format!(
                "{}:{}:{}",
                source.relative_path.display(),
                pos.row + 1,
                pos.column + 1
            )
            .dimmed(),
        );
        print!("{} ", mutation.rule.description.dimmed());

        let new_content = apply_mutation(&source.content, mutation);
        let dest_path = mutation_dir.path().join(&source.relative_path);

        fs::write(&dest_path, &new_content)?;

        // main contract file is always at sources[0]
        let main_contract_relative_path = &sources[0].relative_path;
        let main_contract_dest_path = mutation_dir.path().join(main_contract_relative_path);

        let code_b64 = match compile_file(&main_contract_dest_path.to_string_lossy())? {
            Some(code_b64) => code_b64,
            None => {
                fs::write(&dest_path, &source.content)?;
                exit_mutation_interrupted(path, config, Some(&session.session_id));
            }
        };
        if code_b64.is_empty() {
            println!("{}", "COMPILE ERROR".yellow().bold());

            let record = MutationRecord {
                id: mutation_id,
                rule_name: mutation.rule.name.to_string(),
                rule_description: mutation.rule.description.to_owned(),
                rule_level: mutation.rule.level.label().to_owned(),
                rule_group: mutation.rule.group.to_owned(),
                rule_explanation: mutation.rule.explanation.to_owned(),
                line: pos.row + 1,
                column: pos.column + 1,
                source_path: source.relative_path.to_string_lossy().to_string(),
                code_context: get_code_context(&source.content, &mutation.node, &mutation.rule, 2),
                status: MutationStatus::CompileError,
            };
            append_mutation_session_event(
                &session.progress_path,
                &MutationSessionEvent::MutationCompleted {
                    session_id: session.session_id.clone(),
                    record: record.clone(),
                    completed_at: session::now_rfc3339(),
                },
            )?;
            current_records.push(record);

            fs::write(&dest_path, &source.content)?;
            continue;
        }

        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("acton"));
        let mut cmd = process::Command::new(exe);
        cmd.arg("test")
            .arg(path.as_deref().unwrap_or("."))
            .arg("--fail-fast")
            .arg("--mutate-overrides")
            .arg(format!("{mutate_contract}:{code_b64}"));
        if skip_build_for_child_tests {
            cmd.env("ACTON_INTERNAL_SKIP_BUILD", "1");
        }

        if let Some(filter) = &config.filter {
            cmd.arg("--filter").arg(filter);
        }

        for exclude in &config.exclude_patterns {
            cmd.arg("--exclude").arg(exclude);
        }

        for include in &config.include_patterns {
            cmd.arg("--include").arg(include);
        }

        let output = match run_command_output_interruptible(&mut cmd)? {
            InterruptibleOutput::Completed(output) => output,
            InterruptibleOutput::Interrupted => {
                fs::write(&dest_path, &source.content)?;
                exit_mutation_interrupted(path, config, Some(&session.session_id));
            }
        };

        let survived = output.status.success();
        let status = if survived {
            MutationStatus::Survived
        } else {
            MutationStatus::Killed
        };

        if survived {
            println!("{}", "SURVIVED".red().bold());
        } else {
            println!("{}", "KILLED".green());
        }

        let record = MutationRecord {
            id: mutation_id,
            rule_name: mutation.rule.name.to_string(),
            rule_description: mutation.rule.description.to_owned(),
            rule_level: mutation.rule.level.label().to_owned(),
            rule_group: mutation.rule.group.to_owned(),
            rule_explanation: mutation.rule.explanation.to_owned(),
            line: pos.row + 1,
            column: pos.column + 1,
            source_path: source.relative_path.to_string_lossy().to_string(),
            code_context: get_code_context(&source.content, &mutation.node, &mutation.rule, 2),
            status,
        };
        append_mutation_session_event(
            &session.progress_path,
            &MutationSessionEvent::MutationCompleted {
                session_id: session.session_id.clone(),
                record: record.clone(),
                completed_at: session::now_rfc3339(),
            },
        )?;
        current_records.push(record);

        fs::write(&dest_path, &source.content)?;
    }

    let mut all_records = session.completed_records.clone();
    all_records.extend(current_records);
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

fn compile_file(path: &str) -> anyhow::Result<Option<String>> {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("acton"));
    let mut cmd = process::Command::new(exe);
    let cmd = cmd.arg("compile").arg("--json").arg(path);

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
