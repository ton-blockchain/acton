use crate::commands::common::error_fmt;
use crate::config::ActonConfig;
use anyhow::anyhow;
use owo_colors::OwoColorize;
use serde_json::Value;
use std::path::PathBuf;
use std::{fs, process};
use tempfile::TempDir;
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator};

#[derive(Clone)]
enum MutationEdit {
    Remove,
    Replace { replacement: &'static str },
}

#[derive(Clone)]
struct MutationRule {
    name: &'static str,
    description: &'static str,
    query: &'static str,
    capture: &'static str,
    edit: MutationEdit,
    predicate: Option<fn(&str) -> bool>,
}

impl MutationRule {
    fn remove(
        name: &'static str,
        description: &'static str,
        query: &'static str,
        capture: &'static str,
    ) -> Self {
        Self {
            name,
            description,
            query,
            capture,
            edit: MutationEdit::Remove,
            predicate: None,
        }
    }

    fn replace(
        name: &'static str,
        description: &'static str,
        query: &'static str,
        capture: &'static str,
        replacement: &'static str,
    ) -> Self {
        Self {
            name,
            description,
            query,
            capture,
            edit: MutationEdit::Replace { replacement },
            predicate: None,
        }
    }

    fn with_predicate(mut self, predicate: fn(&str) -> bool) -> Self {
        self.predicate = Some(predicate);
        self
    }
}

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

fn get_code_context(source: &str, node: &Node, context_lines: usize) -> String {
    let start_line = node.start_position().row;
    let end_line = node.end_position().row;

    let lines: Vec<&str> = source.lines().collect();
    let context_start = start_line.saturating_sub(context_lines);
    let context_end = (end_line + context_lines + 1).min(lines.len());

    let mut result = String::new();
    for line_idx in context_start..context_end {
        let line = lines.get(line_idx).unwrap_or(&"");
        let line_num = line_idx + 1;

        if line_idx >= start_line && line_idx <= end_line {
            result.push_str(&format!(
                "  {} {} {}\n",
                format!("{:4}", line_num).dimmed(),
                "│".red(),
                line.red().strikethrough()
            ));
        } else {
            result.push_str(&format!(
                "  {} {} {}\n",
                format!("{:4}", line_num).dimmed(),
                "│".dimmed(),
                line.dimmed()
            ));
        }
    }
    result
}

fn rules() -> Vec<MutationRule> {
    vec![
        MutationRule::remove(
            "remove_assert",
            "Remove assert statements",
            "(assert_statement) @assert",
            "assert",
        ),
        MutationRule::remove(
            "remove_throw",
            "Remove throw keyword",
            "(\"throw\") @throw_kw",
            "throw_kw",
        ),
        MutationRule::replace(
            "flip_plus_assign",
            "Replace += with -=",
            "(\"+=\") @plus_assign",
            "plus_assign",
            "-=",
        ),
        MutationRule::replace(
            "flip_minus_assign",
            "Replace -= with +=",
            "(\"-=\") @minus_assign",
            "minus_assign",
            "+=",
        ),
    ]
}

fn collect_mutations<'a>(
    root: Node<'a>,
    source: &str,
    rules: &[MutationRule],
) -> anyhow::Result<Vec<MutationCandidate<'a>>> {
    let mut candidates = Vec::new();

    for rule in rules {
        let query = Query::new(&tolk_parser::parser::language(), rule.query)
            .map_err(|e| anyhow!("Failed to create query for rule {}: {:?}", rule.name, e))?;

        let mut cursor = QueryCursor::new();
        let matches = cursor.matches(&query, root, source.as_bytes());

        matches.for_each(|m| {
            for capture in m.captures {
                let capture_name = query
                    .capture_names()
                    .get(capture.index as usize)
                    .map(|s| s.as_ref())
                    .unwrap_or("");

                if capture_name != rule.capture {
                    continue;
                }

                let text = capture
                    .node
                    .utf8_text(source.as_bytes())
                    .unwrap_or("")
                    .to_string();

                if let Some(predicate) = rule.predicate
                    && !predicate(&text)
                {
                    continue;
                }

                candidates.push(MutationCandidate {
                    rule: rule.clone(),
                    node: capture.node,
                });
            }
        });
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

pub fn test_mutate_cmd(
    path: &Option<String>,
    mutate_contract: Option<String>,
) -> anyhow::Result<()> {
    let Some(mutate_contract) = mutate_contract else {
        anyhow::bail!("Provide --mutate-contract flag to specify a contract to mutate")
    };
    let config = ActonConfig::load()?;
    let contract = config
        .get_contract(&mutate_contract)
        .ok_or_else(|| anyhow!(error_fmt::contract_not_found(&config, &mutate_contract)))?;

    let content = match fs::read_to_string(&contract.src) {
        Ok(content) => content,
        Err(err) => {
            anyhow::bail!("Error reading file '{}': {err}", contract.src)
        }
    };
    let tree = tolk_parser::parser::parse(&content)?;
    let root_node = tree.root_node();

    let dependencies = abi::get_file_dependencies(&contract.src, true)?;
    let project_root = PathBuf::from(".");

    let mutation_dir = TempDir::new()?;

    for dep_path_str in &dependencies {
        let dep_path = PathBuf::from(dep_path_str);
        let relative_path = if dep_path.is_absolute() {
            dep_path
                .strip_prefix(&project_root)
                .map(|p| p.to_path_buf())
                .unwrap_or(dep_path.clone())
        } else {
            dep_path.clone()
        };
        let dest_path = mutation_dir.path().join(relative_path);

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&dep_path, &dest_path).map_err(|e| {
            anyhow!(
                "Failed to copy {} to {}: {}",
                dep_path.display(),
                dest_path.display(),
                e
            )
        })?;
    }

    let mutation_rules = rules();
    let mutations = collect_mutations(root_node, &content, &mutation_rules)?;

    println!("{}", "Mutation Testing".bold());
    println!("{}", "─".repeat(60).dimmed());
    println!("Contract: {}", contract.name.bright_white());
    println!("Source:   {}", contract.src.dimmed());
    println!("Mutants:  {}\n", mutations.len().to_string().bright_cyan());

    let mut results = Vec::new();

    for (index, mutation) in mutations.iter().enumerate() {
        let pos = mutation.node.start_position();

        print!(
            "  {} Mutation {}/{} ",
            "◉".cyan(),
            (index + 1).to_string().bright_white(),
            mutations.len()
        );
        print!(
            "{}:{}:{} ",
            contract.src.dimmed(),
            pos.row + 1,
            pos.column + 1
        );
        print!(
            "[{}] {} ",
            mutation.rule.name.bright_white(),
            mutation.rule.description.dimmed()
        );

        let new_content = apply_mutation(&content, mutation);
        let contract_src_path = PathBuf::from(&contract.src);
        let relative_contract_path = if contract_src_path.is_absolute() {
            contract_src_path
                .strip_prefix(&project_root)
                .map(|p| p.to_path_buf())
                .unwrap_or(contract_src_path.clone())
        } else {
            contract_src_path.clone()
        };
        let dest_contract_path = mutation_dir.path().join(relative_contract_path);
        fs::write(&dest_contract_path, new_content)?;

        let code_b64 = compile_file(&dest_contract_path.to_string_lossy())?;
        if code_b64.is_empty() {
            println!("{}", "KILLED".green());

            results.push(MutationResult {
                index,
                rule: mutation.rule.clone(),
                node: mutation.node,
                line: pos.row + 1,
                column: pos.column + 1,
                survived: false, // TODO: separate flag for compilation error
            });
            continue;
        }

        let exe = std::env::current_exe().unwrap_or(PathBuf::from("acton"));
        let mut cmd = process::Command::new(exe);
        let cmd = cmd
            .arg("test")
            .arg(path.as_ref().unwrap_or(&".".to_owned()))
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
        });
    }

    let survived_count = results.iter().filter(|r| r.survived).count();
    let killed_count = results.len() - survived_count;
    let mutation_score = if !results.is_empty() {
        (killed_count as f64 / results.len() as f64) * 100.0
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
            println!(
                "\n  {} Mutation #{}",
                "✗".red().bold(),
                (result.index + 1).to_string()
            );
            println!(
                "  {} {}:{}:{}",
                "at".dimmed(),
                contract.src.bright_white(),
                result.line.to_string().bright_white(),
                result.column
            );
            println!(
                "  {} {} - {}",
                "rule".dimmed(),
                result.rule.name.bright_white(),
                result.rule.description.dimmed()
            );
            println!("{}", get_code_context(&content, &result.node, 2));
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
