use super::GITHUB_SOURCE_BASE;
use std::fs;
use std::path::Path;
use tolk_linter::{FixAvailability, Linter, RuleGroup};

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

pub(super) fn generate_linter_docs(out_dir: &Path) -> anyhow::Result<()> {
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

fn clear_generated_linter_rule_pages(out_dir: &Path) -> anyhow::Result<()> {
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

fn write_linter_index(out_dir: &Path, rules: &[LinterRuleDoc]) -> anyhow::Result<()> {
    let mut mdx_content = String::new();
    mdx_content.push_str("---\n");
    mdx_content.push_str("title: \"Linting rules\"\n");
    mdx_content.push_str("description: \"Reference for all Tolk linter checks\"\n");
    mdx_content.push_str("icon: \"FileCheck\"\n");
    mdx_content.push_str("---\n\n");
    mdx_content.push_str(
        "The `acton check` command validates your Tolk code and reports diagnostics for lint rules.\n\n",
    );
    mdx_content.push_str(
        "Use `acton check --explain <CODE>` to read a detailed explanation for any specific rule right in the terminal.\n\n",
    );
    mdx_content.push_str(
        "`acton check --list-lint-rules` also exists as a hidden machine-readable helper, but it only prints rule names and markdown descriptions. This index is the human-readable catalog with rule codes, lifecycle status, and quick-fix availability.\n\n",
    );
    mdx_content.push_str(
        "Lifecycle states currently used in the catalog are mainly `Stable` and `Preview`. The generator also supports future `Deprecated` and `Removed` statuses when rules eventually transition.\n\n",
    );
    mdx_content.push_str(
        "For setup, configuration, and CI usage, start with [Linting](/docs/linting).\n\n",
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

fn write_linter_meta(out_dir: &Path, rules: &[LinterRuleDoc]) -> anyhow::Result<()> {
    let pages = rules
        .iter()
        .map(|rule| rule.slug.clone())
        .collect::<Vec<_>>();
    let content = serde_json::to_string_pretty(&serde_json::json!({ "pages": pages }))?;
    fs::write(out_dir.join("meta.json"), format!("{content}\n"))?;
    Ok(())
}

fn write_linter_rule_page(out_dir: &Path, rule: &LinterRuleDoc) -> anyhow::Result<()> {
    let mut mdx_content = String::new();

    mdx_content.push_str("---\n");
    mdx_content.push_str(&format!(
        "title: \"{}\"\n",
        escape_frontmatter(&format!("{}: {}", rule.code, rule.rule_name))
    ));
    mdx_content.push_str(&format!(
        "description: \"{}\"\n",
        escape_frontmatter(&rule.summary.clone())
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
