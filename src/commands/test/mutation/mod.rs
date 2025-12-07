use crate::commands::common::error_fmt;
use crate::commands::test::TestConfig;
use crate::commands::test::mutation::rules::{MutationEdit, MutationMatcher, MutationRule, rules};
use crate::config::ActonConfig;
use anyhow::anyhow;
use owo_colors::OwoColorize;
use serde_json::Value;
use std::path::PathBuf;
use std::{fs, process};
use tempfile::TempDir;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

mod rules;

#[derive(Clone)]
struct MutationCandidate<'a> {
    rule: MutationRule,
    node: Node<'a>,
}

struct MutationResult<'a> {
    index: usize,
    rule: MutationRule,
    node: Node<'a>,
    line: usize,
    column: usize,
    survived: bool,
    compile_failed: bool,
    source_path: String,
}

struct MutationSource {
    path: PathBuf,
    relative_path: PathBuf,
    content: String,
    tree: tree_sitter::Tree,
}

struct GlobalMutation<'a> {
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

fn get_code_context(source: &str, result: &MutationResult, context_lines: usize) -> String {
    let node = &result.node;
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
            match &result.rule.edit {
                MutationEdit::Remove => {
                    output.push_str(&format!(
                        "  {} {} {}\n",
                        format!("{:4}", line_num).dimmed(),
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
                        format!("{:4}", line_num).dimmed(),
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
                format!("{:4}", line_num).dimmed(),
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
                let query = Query::new(&tolk_parser::parser::language(), query).map_err(|e| {
                    anyhow!("Failed to create query for rule {}: {:?}", rule.name, e)
                })?;

                let mut cursor = QueryCursor::new();
                let matches = cursor.matches(&query, root, source.as_bytes());

                matches.for_each(|m| {
                    for capture_match in m.captures {
                        let capture_name = query
                            .capture_names()
                            .get(capture_match.index as usize)
                            .map(|s| s.as_ref())
                            .unwrap_or("");

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

pub fn test_mutate_cmd(path: &Option<String>, config: &TestConfig) -> anyhow::Result<()> {
    let Some(mutate_contract) = &config.mutate_contract else {
        anyhow::bail!("Provide --mutate-contract flag to specify a contract to mutate")
    };
    let acton_config = ActonConfig::load()?;
    let contract = acton_config.get_contract(mutate_contract).ok_or_else(|| {
        anyhow!(error_fmt::contract_not_found(
            &acton_config,
            mutate_contract
        ))
    })?;

    let all_disable_rules = &config.disable_rules;
    let project_root = std::env::current_dir()?;

    let mut sources = Vec::new();

    let main_content = match fs::read_to_string(&contract.src) {
        Ok(content) => content,
        Err(err) => {
            anyhow::bail!("Error reading file '{}': {err}", contract.src)
        }
    };
    let main_tree = tolk_parser::parser::parse(&main_content)?;
    let main_path = PathBuf::from(&contract.src);
    let main_path = fs::canonicalize(&main_path).unwrap_or(main_path);

    let main_relative_path = if main_path.starts_with(&project_root) {
        main_path.strip_prefix(&project_root)?.to_path_buf()
    } else {
        main_path.clone()
    };

    sources.push(MutationSource {
        path: main_path,
        relative_path: main_relative_path,
        content: main_content,
        tree: main_tree,
    });

    let dependencies = abi::get_file_dependencies(&contract.src, true)?;
    for dep_path_str in &dependencies {
        let dep_path = PathBuf::from(dep_path_str);
        let dep_path = fs::canonicalize(&dep_path).unwrap_or(dep_path);

        if !dep_path.starts_with(&project_root) {
            continue;
        }

        if sources.iter().any(|s| s.path == dep_path) {
            continue;
        }

        let relative_path = dep_path.strip_prefix(&project_root)?.to_path_buf();
        let content = fs::read_to_string(&dep_path)
            .map_err(|e| anyhow!("Error reading dependency {}: {}", dep_path.display(), e))?;
        let tree = tolk_parser::parser::parse(&content)?;

        sources.push(MutationSource {
            path: dep_path,
            relative_path,
            content,
            tree,
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
        .collect();

    let mut global_mutations = Vec::new();
    for (idx, source) in sources.iter().enumerate() {
        let candidates =
            collect_mutations(source.tree.root_node(), &source.content, &filtered_rules)?;
        for candidate in candidates {
            global_mutations.push(GlobalMutation {
                candidate,
                source_index: idx,
            });
        }
    }

    println!("{}", "Mutation Testing".bold());
    println!("{}", "─".repeat(60).dimmed());
    println!("Contract: {}", contract.name.bright_white());
    println!("Main Source: {}", contract.src.dimmed());
    println!("Files:    {}", sources.len().to_string().bright_cyan());
    println!(
        "Mutants:  {}\n",
        global_mutations.len().to_string().bright_cyan()
    );

    let mut results = Vec::new();

    for (index, global_mutation) in global_mutations.iter().enumerate() {
        let mutation = &global_mutation.candidate;
        let source_idx = global_mutation.source_index;
        let source = &sources[source_idx];
        let pos = mutation.node.start_position();

        print!(
            "  {} Mutation {}/{} ",
            "◉".cyan(),
            (index + 1).to_string().bright_white(),
            global_mutations.len()
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

        let code_b64 = compile_file(&main_contract_dest_path.to_string_lossy())?;
        if code_b64.is_empty() {
            println!("{}", "COMPILE ERROR".yellow().bold());

            results.push(MutationResult {
                index,
                rule: mutation.rule.clone(),
                node: mutation.node,
                line: pos.row + 1,
                column: pos.column + 1,
                survived: false,
                compile_failed: true,
                source_path: source.relative_path.to_string_lossy().to_string(),
            });

            fs::write(&dest_path, &source.content)?;
            continue;
        }

        let exe = std::env::current_exe().unwrap_or(PathBuf::from("acton"));
        let mut cmd = process::Command::new(exe);
        let cmd = cmd
            .arg("test")
            .arg(path.as_ref().unwrap_or(&".".to_owned()))
            .arg("--fail-fast")
            .arg("--mutate-overrides")
            .arg(format!("{mutate_contract}:{code_b64}"));
        let output = cmd.output()?;

        let survived = output.status.success();

        if survived {
            println!("{}", "SURVIVED".red().bold());
        } else {
            println!("{}", "KILLED".green());
        }

        results.push(MutationResult {
            index,
            rule: mutation.rule.clone(),
            node: mutation.node,
            line: pos.row + 1,
            column: pos.column + 1,
            survived,
            compile_failed: false,
            source_path: source.relative_path.to_string_lossy().to_string(),
        });

        fs::write(&dest_path, &source.content)?;
    }

    let compile_failed_count = results.iter().filter(|r| r.compile_failed).count();
    let killed_count = results
        .iter()
        .filter(|r| !r.survived && !r.compile_failed)
        .count();
    let survived_count = results.iter().filter(|r| r.survived).count();
    let executed_total = results.len().saturating_sub(compile_failed_count);
    let mutation_score = if executed_total > 0 {
        ((killed_count + compile_failed_count) as f64 / executed_total as f64) * 100.0
    } else {
        0.0
    };

    println!();

    println!(
        "  {} {:<20} {}",
        " ".dimmed(),
        "Total mutants",
        results.len()
    );

    println!(
        "  {} {:<20} {}",
        "✓".green(),
        "Killed".green(),
        killed_count.to_string().green()
    );

    println!(
        "  {} {:<20} {}",
        "✗".red(),
        "Survived".red(),
        survived_count.to_string().red()
    );

    println!(
        "  {} {:<20} {}",
        "!".yellow(),
        "Compile errors".yellow(),
        compile_failed_count.to_string().yellow()
    );

    let score_str = format!("{:.1}%", mutation_score);
    let (score_icon, score_label) = match mutation_score as u32 {
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
        if mutation_score <= 50.0 {
            score_str.red().bold().to_string()
        } else if mutation_score <= 80.0 {
            score_str.yellow().bold().to_string()
        } else {
            score_str.green().bold().to_string()
        }
    );

    if survived_count > 0 {
        println!("\n{}", "Survived Mutants".yellow());
        println!("{}", "─".repeat(60).dimmed());

        for result in results.iter().filter(|r| r.survived) {
            println!("\n  {} Mutation #{}", "✗".red().bold(), (result.index + 1));
            println!(
                "  {} {}{}{}",
                "Rule:".dimmed(),
                result.rule.description,
                " ",
                format!("[{}]", result.rule.name).dimmed()
            );
            println!(
                "  {} {}",
                "Level".dimmed(),
                result.rule.level.colorize(result.rule.level.label())
            );
            println!(
                "  {} {}:{}:{}",
                "at".dimmed(),
                result.source_path.bright_white(),
                result.line.to_string().bright_white(),
                result.column
            );

            let content = sources
                .iter()
                .find(|s| s.relative_path.to_string_lossy() == result.source_path)
                .map(|s| s.content.as_str())
                .unwrap_or("");

            println!("{}", get_code_context(content, result, 2));
            println!(
                "  {} {}",
                "Why it's bad:".dimmed(),
                result.rule.explanation.dimmed()
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

    Ok(())
}

fn compile_file(path: &str) -> anyhow::Result<String> {
    let exe = std::env::current_exe().unwrap_or(PathBuf::from("acton"));
    let mut cmd = process::Command::new(exe);
    let cmd = cmd.arg("compile").arg("--json").arg(path);

    let compilation_result = cmd.output()?;
    let compilation_result = String::from_utf8_lossy(&compilation_result.stdout);
    let compilation_result: Value = serde_json::from_str(compilation_result.as_ref())?;
    let Some(success) = compilation_result.get("success") else {
        anyhow::bail!("Compilation returned invalid result without `success` flag");
    };
    let success = success.as_bool().unwrap_or(false);
    if !success {
        return Ok("".to_owned());
    }
    let Some(code_b64) = compilation_result.get("code_boc64") else {
        anyhow::bail!("No code boc64 found in compilation result")
    };
    let Value::String(code_b64) = code_b64 else {
        anyhow::bail!("No code boc64 found in compilation result")
    };
    Ok(code_b64.clone())
}
