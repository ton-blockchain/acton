use anyhow::Result;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tolk_linter::{FixAvailability, Linter, RuleGroup};
use tolk_syntax::parse;
use tree_sitter::Node;

const DEFAULT_STDLIB_OUT: &str = "docs/content/docs/standard_library";
const DEFAULT_LINTER_OUT: &str = "docs/content/docs/linter";
const GITHUB_SOURCE_BASE: &str = "https://github.com/i582/acton/blob/master";

pub fn docgen_cmd(output: Option<String>) -> Result<()> {
    let stdlib_output = output.unwrap_or_else(|| DEFAULT_STDLIB_OUT.to_string());
    let stdlib_out_dir = PathBuf::from(&stdlib_output);

    generate_stdlib_docs(Path::new("lib"), &stdlib_out_dir)?;

    let linter_out_dir = stdlib_out_dir
        .parent()
        .map(|parent| parent.join("linter"))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_LINTER_OUT));

    generate_linter_docs(&linter_out_dir)?;

    Ok(())
}

fn generate_stdlib_docs(lib_dir: &Path, out_dir: &Path) -> Result<()> {
    if !lib_dir.exists() {
        anyhow::bail!("Directory 'lib' not found");
    }

    fs::create_dir_all(out_dir)?;

    let index_article = out_dir.join("index.mdx");
    fs::write(
        &index_article,
        r#"---
title: "Standard library"
description: "Learn about available functions/struct/constants available in Acton standard library"
icon: FileCode
---

Acton provides a collection of functions for writing scripts and tests in Tolk.
"#,
    )?;

    let mut files: Vec<_> = walkdir::WalkDir::new(lib_dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| {
            e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "tolk")
        })
        .collect();

    files.sort_by_key(|e| e.path().to_string_lossy().to_string());

    struct FileDoc {
        path: PathBuf,
        file_stem: String,
        symbols: Vec<SymbolInfo>,
        file_header: Option<String>,
    }

    let mut docs = Vec::new();
    let mut symbol_map: HashMap<String, PathBuf> = HashMap::new();

    for entry in files {
        let path = entry.path();
        let path_string = path.to_string_lossy();
        if path_string.contains("tests") || path_string.ends_with(".test.tolk") {
            continue;
        }

        let content = fs::read_to_string(path)?;
        let relative_path = path.strip_prefix(lib_dir)?;
        let file_stem = relative_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let tree = parse(&content)?;
        let root_node = tree.root_node();

        let symbols = extract_symbols(root_node, &content);
        let symbols: Vec<_> = symbols.into_iter().filter(|s| !skip_symbol(s)).collect();
        let file_header = extract_file_header_doc(&content);

        if !symbols.is_empty() || file_header.is_some() {
            let target_rel_path = relative_path.with_extension("");
            for symbol in &symbols {
                symbol_map.insert(symbol.name.clone(), target_rel_path.clone());
            }

            docs.push(FileDoc {
                path: path.to_path_buf(),
                file_stem,
                symbols,
                file_header,
            });
        }
    }

    let link_regex = Regex::new(r"\[([a-zA-Z0-9_.]+)]")?;

    for doc in docs {
        let relative_path = doc.path.strip_prefix(lib_dir)?;
        let current_file_stem_path = relative_path.with_extension("");

        let mut output_path = out_dir.to_path_buf();
        if let Some(parent) = relative_path.parent() {
            output_path.push(parent);
            fs::create_dir_all(&output_path)?;
        }
        output_path.push(format!("{}.mdx", doc.file_stem));

        let mut mdx_content = String::new();
        mdx_content.push_str("---\n");
        mdx_content.push_str(&format!("title: \"{}\"\n", doc.file_stem));
        mdx_content.push_str(&format!(
            "description: \"{}.tolk standard library file\"\n",
            doc.file_stem
        ));
        mdx_content.push_str("---\n\n");
        mdx_content.push_str("import { SourceCodeLink } from '@/components/SourceCodeLink';\n\n");

        if let Some(header) = &doc.file_header {
            mdx_content.push_str(header);
            mdx_content.push_str("\n\n");
        }

        if !doc.symbols.is_empty() {
            mdx_content.push_str("## Definitions\n\n");
        }

        for symbol in doc.symbols {
            mdx_content.push_str(&format!("## `{}`\n\n", symbol.name));

            let source_url = format!(
                "{GITHUB_SOURCE_BASE}/{}#L{}",
                doc.path.to_string_lossy(),
                symbol.start_line + 1
            );

            mdx_content.push_str("```tolk\n");
            mdx_content.push_str(&symbol.signature);
            mdx_content.push_str("\n```\n\n");

            if let Some(doc_text) = symbol.doc.as_ref() {
                if Some(doc_text) == doc.file_header.as_ref() {
                    // skip if the symbol doc is exactly the same as the file header
                } else {
                    let processed_doc =
                        link_regex.replace_all(doc_text, |caps: &regex::Captures<'_>| {
                            let name = &caps[1];
                            if let Some(target_path) = symbol_map.get(name) {
                                if target_path == &current_file_stem_path {
                                    format!("[{}](#{})", name, normalize_symbol_link(name))
                                } else {
                                    let relative_link_path =
                                        pathdiff::diff_paths(target_path, &current_file_stem_path)
                                            .unwrap_or_else(|| target_path.clone());

                                    let link = relative_link_path.to_string_lossy().to_string();
                                    format!("[{}]({}/#{})", name, link, normalize_symbol_link(name))
                                }
                            } else {
                                eprintln!("Warning: Symbol '{name}' not found in documentation");
                                name.to_string()
                            }
                        });
                    mdx_content.push_str(&processed_doc);
                    mdx_content.push_str("\n\n");
                }
            }

            mdx_content.push_str(&format!("<SourceCodeLink href=\"{source_url}\" />\n\n"));
        }
        fs::write(output_path, mdx_content)?;
    }

    Ok(())
}

#[derive(Debug)]
struct LinterRuleDoc {
    code: String,
    rule_name: String,
    slug: String,
    group: RuleGroup,
    fix: FixAvailability,
    explanation: String,
    summary: String,
    source_file: String,
    source_line: u32,
}

fn generate_linter_docs(out_dir: &Path) -> Result<()> {
    fs::create_dir_all(out_dir)?;

    let rules = collect_linter_rules();
    clear_generated_linter_rule_pages(out_dir)?;

    write_linter_index(out_dir, &rules)?;
    write_linter_meta(out_dir, &rules)?;

    for rule in &rules {
        write_linter_rule_page(out_dir, rule)?;
    }

    Ok(())
}

fn collect_linter_rules() -> Vec<LinterRuleDoc> {
    let mut rules: Vec<_> = Linter::Tolk
        .all_rules()
        .map(|rule| {
            let code = Linter::Tolk
                .code_for_rule(rule)
                .unwrap_or("UNKNOWN")
                .to_string();
            let rule_name = rule.name().to_string();
            let explanation = rule.explanation().unwrap_or_default().trim().to_string();
            let summary = extract_rule_summary(&explanation);
            let source_file = rule.file().replace('\\', "/");

            let slug = if code == "UNKNOWN" {
                format!("rule-{rule_name}")
            } else {
                format!("{}-{rule_name}", code.to_ascii_lowercase())
            };

            LinterRuleDoc {
                code,
                rule_name,
                slug,
                group: rule.group(),
                fix: rule.fixable(),
                explanation,
                summary,
                source_file,
                source_line: rule.line(),
            }
        })
        .collect();

    rules.sort_by(|a, b| {
        a.code
            .cmp(&b.code)
            .then_with(|| a.rule_name.cmp(&b.rule_name))
    });

    rules
}

fn clear_generated_linter_rule_pages(out_dir: &Path) -> Result<()> {
    for entry in fs::read_dir(out_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if path.extension().is_some_and(|ext| ext == "mdx") && file_name != "index.mdx" {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}

fn write_linter_index(out_dir: &Path, rules: &[LinterRuleDoc]) -> Result<()> {
    let mut mdx_content = String::new();
    mdx_content.push_str("---\n");
    mdx_content.push_str("title: \"Linter Rules\"\n");
    mdx_content.push_str("description: \"Reference for all Tolk linter checks\"\n");
    mdx_content.push_str("icon: FileCheck\n");
    mdx_content.push_str("---\n\n");
    mdx_content.push_str(
        "The `acton check` command validates your Tolk code and reports diagnostics for lint rules.\n\n",
    );
    mdx_content.push_str(
        "Use `acton check --explain <CODE>` to read a detailed explanation for any specific rule right in the terminal.\n\n",
    );

    mdx_content.push_str("| Code | Rule | Status | Quick fix | What it does |\n");
    mdx_content.push_str("|:-----|:-----|:-------|:----------|:-------------|\n");

    for rule in rules {
        mdx_content.push_str(&format!(
            "| [{}](./{}) | [`{}`](./{}) | {} | {} | {} |\n",
            rule.code,
            rule.slug,
            rule.rule_name,
            rule.slug,
            table_cell(&format_rule_group(rule.group)),
            table_cell(fix_availability_label(rule.fix)),
            table_cell(&rule.summary),
        ));
    }

    fs::write(out_dir.join("index.mdx"), mdx_content)?;

    Ok(())
}

fn write_linter_meta(out_dir: &Path, rules: &[LinterRuleDoc]) -> Result<()> {
    let pages = rules
        .iter()
        .map(|rule| rule.slug.clone())
        .collect::<Vec<_>>();
    let content = serde_json::to_string_pretty(&serde_json::json!({ "pages": pages }))?;
    fs::write(out_dir.join("meta.json"), format!("{content}\n"))?;
    Ok(())
}

fn write_linter_rule_page(out_dir: &Path, rule: &LinterRuleDoc) -> Result<()> {
    let mut mdx_content = String::new();

    mdx_content.push_str("---\n");
    mdx_content.push_str(&format!(
        "title: \"{}\"\n",
        escape_frontmatter(&format!("{}: {}", rule.code, rule.rule_name))
    ));
    mdx_content.push_str(&format!(
        "description: \"{}\"\n",
        escape_frontmatter(&rule.summary.to_string())
    ));
    mdx_content.push_str("---\n\n");

    mdx_content.push_str("import { SourceCodeLink } from '@/components/SourceCodeLink';\n\n");

    mdx_content.push_str("## Metadata\n\n");
    mdx_content.push_str(&format!("- `Code`: `{}`\n", rule.code));
    mdx_content.push_str(&format!("- `Rule`: `{}`\n", rule.rule_name));
    mdx_content.push_str(&format!("- `Status`: {}\n", format_rule_group(rule.group)));
    mdx_content.push_str(&format!(
        "- `Quick fix`: {}\n\n",
        fix_availability_label(rule.fix)
    ));

    if !rule.explanation.is_empty() {
        mdx_content.push_str(&rule.explanation);
        mdx_content.push_str("\n\n");
    }

    let source_url = format!(
        "{GITHUB_SOURCE_BASE}/{}#L{}",
        rule.source_file, rule.source_line
    );
    mdx_content.push_str(&format!("<SourceCodeLink href=\"{source_url}\" />\n"));

    fs::write(out_dir.join(format!("{}.mdx", rule.slug)), mdx_content)?;

    Ok(())
}

fn table_cell(value: &str) -> String {
    collapse_whitespace(&value.replace('\n', " ").replace('|', "\\|"))
}

fn format_rule_group(group: RuleGroup) -> String {
    match group {
        RuleGroup::Stable { since } => format!("Stable since `{since}`"),
        RuleGroup::Preview { since } => format!("Preview since `{since}`"),
        RuleGroup::Deprecated { since } => format!("Deprecated since `{since}`"),
        RuleGroup::Removed { since } => format!("Removed since `{since}`"),
    }
}

const fn fix_availability_label(fix: FixAvailability) -> &'static str {
    match fix {
        FixAvailability::Always => "always available",
        FixAvailability::Sometimes => "sometimes available",
        FixAvailability::None => "not available",
    }
}

fn extract_rule_summary(explanation: &str) -> String {
    let mut in_what_it_does = false;
    let mut section_lines = Vec::new();

    for line in explanation.lines() {
        let trimmed = line.trim();

        if trimmed == "### What it does" {
            in_what_it_does = true;
            continue;
        }

        if in_what_it_does && trimmed.starts_with("### ") {
            break;
        }

        if in_what_it_does && !trimmed.is_empty() {
            section_lines.push(trimmed);
        }
    }

    if !section_lines.is_empty() {
        return collapse_whitespace(&section_lines.join(" "));
    }

    if let Some(line) = explanation
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("### ") && !line.starts_with("```"))
    {
        return collapse_whitespace(line);
    }

    "No description provided.".to_string()
}

fn collapse_whitespace(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn escape_frontmatter(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn normalize_symbol_link(link: &str) -> String {
    link.replace('\\', "/")
        .replace('.', "")
        .to_ascii_lowercase()
}

fn skip_symbol(s: &SymbolInfo) -> bool {
    s.name == "ffi"
        || s.name == "impl"
        || s.name == "expect_impl"
        || s.name == "impl_msg"
        || s.name.starts_with("ffi.")
        || s.name.starts_with("impl.")
        || s.name.starts_with("expect_impl.")
        || s.name.starts_with("impl_msg.")
        || s.name.starts_with("never.")
        || s.name.contains("__")
}

enum SymbolKind {
    Function,
    Struct,
    Constant,
}

struct SymbolInfo {
    #[allow(dead_code)]
    kind: SymbolKind,
    name: String,
    signature: String,
    doc: Option<String>,
    start_line: usize,
}

fn extract_symbols(root: Node<'_>, source: &str) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        let kind = child.kind();
        if kind == "function_declaration"
            || kind == "method_declaration"
            || kind == "get_method_declaration"
        {
            if let Some(func) = parse_function(child, source) {
                symbols.push(func);
            }
        } else if kind == "struct_declaration"
            && let Some(s) = parse_struct(child, source)
        {
            symbols.push(s);
        } else if kind == "constant_declaration"
            && let Some(c) = parse_constant(child, source)
        {
            symbols.push(c);
        }
    }

    symbols
}

fn parse_struct(node: Node<'_>, source: &str) -> Option<SymbolInfo> {
    let name_node = node.child_by_field_name("name")?;
    let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

    let signature = node.utf8_text(source.as_bytes()).ok()?;

    let doc = extract_doc_comment(node, source);
    let start_line = node.start_position().row;

    Some(SymbolInfo {
        kind: SymbolKind::Struct,
        name,
        signature: signature.to_owned(),
        doc,
        start_line,
    })
}

fn parse_constant(node: Node<'_>, source: &str) -> Option<SymbolInfo> {
    let name_node = node.child_by_field_name("name")?;
    let name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

    let full_text = node.utf8_text(source.as_bytes()).ok()?;
    let doc = extract_doc_comment(node, source);
    let start_line = node.start_position().row;

    Some(SymbolInfo {
        kind: SymbolKind::Constant,
        name,
        signature: full_text.to_string(),
        doc,
        start_line,
    })
}

fn parse_function(node: Node<'_>, source: &str) -> Option<SymbolInfo> {
    let kind = node.kind();

    let name_node = node.child_by_field_name("name")?;
    let mut name = name_node.utf8_text(source.as_bytes()).ok()?.to_string();

    if kind == "method_declaration"
        && let Some(receiver_node) = node.child_by_field_name("receiver")
        && let Some(type_node) = receiver_node.child_by_field_name("receiver_type")
    {
        let type_name = type_node.utf8_text(source.as_bytes()).ok()?;
        name = format!("{type_name}.{name}");
    }

    let full_text = node.utf8_text(source.as_bytes()).ok()?;

    let signature = if let Some(body) = node.child_by_field_name("body") {
        let cut_idx = body.start_byte() - node.start_byte();
        full_text[..cut_idx].trim().to_string()
    } else {
        full_text.to_string()
    };

    let doc = extract_doc_comment(node, source);
    let start_line = node.start_position().row;

    Some(SymbolInfo {
        kind: SymbolKind::Function,
        name,
        signature,
        doc,
        start_line,
    })
}

fn extract_doc_comment(node: Node<'_>, source: &str) -> Option<String> {
    let start_byte = node.start_byte();
    let prefix = &source[..start_byte];

    let mut lines = Vec::new();

    for line in prefix.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !lines.is_empty() {
                break;
            }
        } else if trimmed.starts_with("///") {
            let content = trimmed.trim_start_matches("///");
            let content = if let Some(stripped) = content.strip_prefix(' ') {
                stripped.to_string()
            } else {
                content.to_string()
            };

            lines.push(content);
        } else {
            break;
        }
    }

    if lines.is_empty() {
        None
    } else {
        lines.reverse();
        Some(lines.join("\n"))
    }
}

fn extract_file_header_doc(source: &str) -> Option<String> {
    let mut lines = Vec::new();
    let mut has_content = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("///") {
            let content = trimmed.trim_start_matches("///");
            let content = if let Some(stripped) = content.strip_prefix(' ') {
                stripped
            } else {
                content
            };
            lines.push(content);
            has_content = true;
        } else if trimmed.is_empty() {
            break;
        } else {
            // Found a non-comment, non-empty line. Stop.
            break;
        }
    }

    // Trim trailing empty lines from the buffer
    while let Some(last) = lines.last() {
        if last.is_empty() {
            lines.pop();
        } else {
            break;
        }
    }

    if has_content && !lines.is_empty() {
        Some(lines.join("\n"))
    } else {
        None
    }
}
