use acton_config::config::{ActonConfig, ContractConfig};
use owo_colors::OwoColorize;
use serde_json;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;
use tolk_linter::diagnostic::{Annotation, Applicability, Diagnostic, Severity};
use tolk_linter::{Checker, Tolk};
use tolk_resolver::file_db::FileDb;
use tolk_resolver::file_index::Span;
use tolk_resolver::project_index::ProjectIndex;
use tolk_resolver::symbol_resolver::resolve;
use tolk_ty::TypeDb;
use tolk_ty::TypeInterner;
use tolk_ty::infer;
use tree_sitter::Point;

pub fn check_cmd(
    fix: bool,
    json: bool,
    explain: Option<String>,
    list_lint_rules: bool,
) -> anyhow::Result<()> {
    if list_lint_rules {
        let rules: Vec<_> = tolk_linter::Linter::Tolk
            .all_rules()
            .map(|r| {
                serde_json::json!({
                    "name": r.name(),
                    "description": r.explanation().unwrap_or_default(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rules)?);
        return Ok(());
    }

    if let Some(code) = explain {
        if let Ok(tolk_rules) = Tolk::from_str(&code)
            && let Some(rule) = tolk_rules.rules().next()
        {
            if let Some(explanation) = rule.explanation() {
                println!("{}", explanation);
            } else {
                println!("No explanation available for rule {}", code);
            }
        } else {
            anyhow::bail!("Unknown rule code: {}", code);
        }
        return Ok(());
    }

    let config = ActonConfig::load()?;

    let contracts = match config.contracts() {
        Some(contracts) => contracts,
        None => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "success": true,
                        "diagnostics": [],
                    })
                );
            } else {
                println!(
                    "No contracts found in Acton.toml. Run {} first or add contracts manually.",
                    "acton init".yellow()
                );
            }
            return Ok(());
        }
    };

    if contracts.is_empty() {
        if json {
            println!(
                "{}",
                serde_json::json!({
                    "success": true,
                    "diagnostics": [],
                })
            );
        } else {
            println!("No contracts to check.");
        }
        return Ok(());
    }

    let stdlib = find_stdlib()?;
    let acton_stdlib = find_acton_stdlib()?;
    let common_tolk = stdlib.join("common.tolk");

    let file_db = FileDb::new(stdlib, Some(acton_stdlib));

    // We need stdlib for all targets so preprocess it before all.
    if common_tolk.exists() {
        file_db.process(&common_tolk)?;
    }

    let mut all_diagnostics = Vec::new();

    for (contract_id, contract) in contracts {
        let contract_diagnostics =
            check_contract(contract_id, contract, &file_db, fix, json, &config)?;
        all_diagnostics.extend(contract_diagnostics);
    }

    if json {
        let json_output = serde_json::json!({
            "success": true,
            "diagnostics": all_diagnostics.iter().map(|d| diagnostic_to_json(d, &file_db)).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        let shown_diagnostics = if fix {
            filter_fixed_diagnostics(&all_diagnostics)
        } else {
            all_diagnostics
        };

        if !shown_diagnostics.is_empty() {
            let first_code = shown_diagnostics
                .iter()
                .find(|d| d.code.is_some())
                .and_then(|d| d.code.clone());
            if let Some(code) = first_code {
                eprintln!();
                eprintln!(
                    "Use {} to get detailed explanation of a rule.",
                    "acton check --explain <CODE>".yellow()
                );
                eprintln!("For example: acton check --explain {}", code);
            }
        }
    }

    Ok(())
}

fn find_stdlib() -> anyhow::Result<PathBuf> {
    let path_to_acton = PathBuf::from(".acton/tolk-stdlib");
    if !path_to_acton.exists() {
        anyhow::bail!(
            "cannot find Tolk stdlib in .acton/, did you run {}?",
            "acton init".yellow()
        );
    }

    Ok(path_to_acton.canonicalize()?)
}

fn find_acton_stdlib() -> anyhow::Result<PathBuf> {
    let path_to_acton = PathBuf::from(".acton");
    if !path_to_acton.exists() {
        anyhow::bail!(
            "cannot find Acton in .acton/, did you run {}?",
            "acton init".yellow()
        );
    }

    Ok(path_to_acton.canonicalize()?)
}

fn check_contract(
    contract_id: &str,
    config: &ContractConfig,
    file_db: &FileDb,
    fix: bool,
    json: bool,
    acton_config: &ActonConfig,
) -> anyhow::Result<Vec<Diagnostic>> {
    if !config.src.ends_with(".tolk") {
        // skip contracts with .boc sources
        return Ok(vec![]);
    }

    let root = PathBuf::from(&config.src).canonicalize()?;
    let current_dir = std::env::current_dir().unwrap_or_default();
    let relative_root = pathdiff::diff_paths(&root, &current_dir).unwrap_or_else(|| root.clone());

    if !json {
        println!(
            "Checking {} ({})",
            config.name,
            relative_root.display().cyan()
        );
    }

    let lint_settings = Checker::build_settings(acton_config, Some(contract_id));

    check_file(&root, file_db, fix, json, lint_settings)
}

fn check_file(
    root: &Path,
    file_db: &FileDb,
    fix: bool,
    json: bool,
    lint_settings: HashMap<tolk_linter::Rule, acton_config::config::LintLevel>,
) -> anyhow::Result<Vec<Diagnostic>> {
    let file_info = file_db.process(root)?;

    let parse_errors = file_info.source().errors();
    let mut all_diagnostics = vec![];

    for parse_error in parse_errors {
        let start_byte =
            byte_offset_from_point(&parse_error.span.start, &file_info.source().source);
        let end_byte = byte_offset_from_point(&parse_error.span.end, &file_info.source().source);

        let diagnostic = Diagnostic {
            file_id: file_info.id(),
            severity: Severity::Error,
            code: None,
            name: "parse-error",
            message: parse_error.message.clone(),
            annotations: vec![Annotation {
                span: Span {
                    start: start_byte as u32,
                    end: end_byte as u32,
                },
                message: None,
                is_primary: true,
                tags: vec![],
            }],
            fixes: vec![],
            help: None,
        };
        all_diagnostics.push(diagnostic);
    }

    // First we need to build project index:
    // - find all reachable files
    // - parse
    // - resolve imports
    let now = Instant::now();
    let mut index = ProjectIndex::builder(file_db, root.to_owned())
        .with_stdlib(file_db.stdlib_path().to_owned())
        .build()?;
    log::debug!("Build project index took {:?}", now.elapsed());
    log::debug!("Index: {:?}", index.files().len());

    // Then we can resolve all symbols across files
    let now = Instant::now();
    resolve(file_db, &mut index);
    log::debug!("Resolve project took {:?}", now.elapsed());

    // Infer types of all top level declarations
    let now = Instant::now();
    let mut interner = TypeInterner::new();
    let mut type_db = TypeDb::new(&mut interner, file_db, &index);

    let mut body_types = HashMap::new();

    let files_to_check = index.reachable_files(file_info.id());

    for file_to_check in &files_to_check {
        let mut file_body_types = HashMap::new();

        for decl in file_info.source().top_levels() {
            let Some(index_decl) = file_info.find_declaration(&decl) else {
                continue;
            };

            let res = infer(&mut type_db, file_info.id(), index_decl.id, &decl);
            file_body_types.insert(index_decl.id, res);
        }

        body_types.insert(*file_to_check, file_body_types);
    }
    log::debug!("Infer types took {:?}", now.elapsed());

    // And finally run all inspections provided by checker
    let now = Instant::now();
    let mut checker = Checker::new(file_db, &mut type_db, &body_types).with_settings(lint_settings);

    for file_to_check in files_to_check {
        let Some(info) = file_db.get_by_id(file_to_check) else {
            continue;
        };
        if !file_info.is_workspace_file() {
            // we don't want to check non-workspace files
            continue;
        }

        checker.process_file(info.source(), info.id());
    }

    checker.apply_suppressions();
    log::debug!("Run diagnostics in {:?}", now.elapsed());

    #[cfg(feature = "profile_rules")]
    {
        checker.print_profiling_results();
    }

    let mut diagnostics = checker.diagnostics.clone();
    diagnostics.extend(all_diagnostics);

    if !json {
        let diagnostics_to_show = if fix {
            filter_fixed_diagnostics(&diagnostics)
        } else {
            diagnostics.clone()
        };
        let _ = emit_diagnostics(file_db, &diagnostics_to_show);
    }

    if fix && !json {
        apply_fixes(file_db, &diagnostics)?;
    }

    Ok(diagnostics)
}

fn filter_fixed_diagnostics(diagnostics: &[Diagnostic]) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .filter(|d| {
            !d.fixes
                .iter()
                .any(|f| f.applicability == Applicability::Auto)
        })
        .cloned()
        .collect()
}

fn byte_offset_from_point(point: &Point, source: &str) -> usize {
    let lines: Vec<&str> = source.lines().collect();
    let mut offset = 0;

    // Add bytes for complete lines before the target row
    for i in 0..point.row {
        if i < lines.len() {
            offset += lines[i].len() + 1; // +1 for newline
        }
    }

    // Add bytes for characters in the current line
    if point.row < lines.len() {
        offset += point.column;
    }

    offset
}

fn emit_diagnostics(file_db: &FileDb, diagnostics: &[Diagnostic]) -> anyhow::Result<()> {
    use codespan_reporting::diagnostic::{Diagnostic, Label, Severity};
    use codespan_reporting::files::SimpleFiles;
    use codespan_reporting::term::{
        self,
        termcolor::{Color, ColorChoice, StandardStream},
    };

    let mut files = SimpleFiles::new();
    let mut file_id_map = HashMap::new();

    for info in file_db.iter() {
        let cs_file_id = files.add(
            info.path().to_string_lossy().to_string(),
            info.source().source.as_ref().to_owned(),
        );
        file_id_map.insert(info.id(), cs_file_id);
    }

    let writer = StandardStream::stderr(ColorChoice::Auto);
    let mut config = term::Config::default();

    let mut styles = term::Styles::default();
    styles.header_bug.set_intense(true);
    styles.header_error.set_intense(true);
    styles.header_warning.set_intense(true);
    styles.header_note.set_intense(true);
    styles
        .header_help
        .set_fg(Some(Color::Green))
        .set_intense(true);
    styles.primary_label_bug.set_intense(true);
    styles.primary_label_error.set_intense(true);
    styles.primary_label_warning.set_intense(true);
    styles.primary_label_note.set_intense(true);
    styles
        .primary_label_help
        .set_fg(Some(Color::Green))
        .set_intense(true);
    styles.secondary_label.set_intense(true);

    config.styles = styles;

    for diag in diagnostics {
        let severity = match diag.severity {
            tolk_linter::diagnostic::Severity::Info => Severity::Note,
            tolk_linter::diagnostic::Severity::Warning => Severity::Warning,
            tolk_linter::diagnostic::Severity::Error => Severity::Error,
            tolk_linter::diagnostic::Severity::Fatal => Severity::Bug,
            tolk_linter::diagnostic::Severity::Help => Severity::Help,
        };

        let mut cs_diag = Diagnostic::new(severity).with_message(&diag.message);
        if let Some(code) = &diag.code {
            cs_diag = cs_diag.with_code(code);
        }

        if let Some(help) = &diag.help {
            cs_diag = cs_diag.with_notes(vec![help.clone()]);
        }

        let mut labels = vec![];
        for anno in &diag.annotations {
            let cs_file_id = *file_id_map.get(&diag.file_id).unwrap_or(&0);
            let mut label = if anno.is_primary {
                Label::primary(cs_file_id, anno.span.start()..anno.span.end())
            } else {
                Label::secondary(cs_file_id, anno.span.start()..anno.span.end())
            };
            if let Some(msg) = &anno.message {
                label = label.with_message(msg);
            }
            labels.push(label);
        }
        cs_diag = cs_diag.with_labels(labels);

        term::emit(&mut writer.lock(), &config, &files, &cs_diag)?;

        for fix in &diag.fixes {
            let mut labels = vec![];
            for edit in &fix.edits {
                let cs_file_id = *file_id_map.get(&diag.file_id).unwrap_or(&0);
                labels.push(
                    Label::primary(cs_file_id, edit.span.start()..edit.span.end())
                        .with_message(&edit.replacement),
                );
            }
            let fix_diag = Diagnostic::new(Severity::Help)
                .with_message(&fix.message)
                .with_labels(labels);
            term::emit(&mut writer.lock(), &config, &files, &fix_diag)?;
        }
    }

    Ok(())
}

fn apply_fixes(file_db: &FileDb, diagnostics: &[Diagnostic]) -> anyhow::Result<()> {
    let mut fixes_by_file: BTreeMap<String, Vec<(usize, usize, String)>> = BTreeMap::new();
    let mut total_diags_by_file: HashMap<String, usize> = HashMap::new();
    let mut fixed_diags_by_file: HashMap<String, usize> = HashMap::new();

    for diag in diagnostics {
        let file_info = file_db
            .get_by_id(diag.file_id)
            .ok_or_else(|| anyhow::anyhow!("File info not found for file_id {}", diag.file_id))?;

        let file_path = file_info.path().to_string_lossy().to_string();

        *total_diags_by_file.entry(file_path.clone()).or_default() += 1;

        if diag.fixes.is_empty() {
            continue;
        }

        // For now, apply only the first fix for each diagnostic
        let fix = &diag.fixes[0];
        *fixed_diags_by_file.entry(file_path.clone()).or_default() += 1;

        for edit in &fix.edits {
            fixes_by_file.entry(file_path.clone()).or_default().push((
                edit.span.start as usize,
                edit.span.end as usize,
                edit.replacement.clone(),
            ));
        }
    }

    let current_dir = std::env::current_dir().unwrap_or_default();

    for (file_path, mut fixes) in fixes_by_file {
        let content = fs::read_to_string(&file_path)?;
        let total_issues = *total_diags_by_file.get(&file_path).unwrap_or(&0);
        let fixed_issues = *fixed_diags_by_file.get(&file_path).unwrap_or(&0);

        // sort fixes by start position in reverse order (to avoid offset issues when multiple fixes)
        fixes.sort_by(|a, b| b.0.cmp(&a.0));

        let mut new_content = content.clone();
        let mut applied_fixes = 0;

        for (start, end, replacement) in fixes {
            let start_char = byte_to_char_index(&content, start);
            let end_char = byte_to_char_index(&content, end);

            if start_char <= content.len() && end_char <= content.len() && start_char <= end_char {
                new_content.replace_range(start_char..end_char, &replacement);
                applied_fixes += 1;
            }
        }

        if applied_fixes > 0 {
            fs::write(&file_path, new_content)?;

            let relative_path = pathdiff::diff_paths(&file_path, &current_dir)
                .unwrap_or_else(|| PathBuf::from(&file_path));

            if fixed_issues == total_issues {
                println!("Fixed all issues in {}", relative_path.display().cyan());
            } else {
                let remaining = total_issues - fixed_issues;
                println!(
                    "Applied {} {} to {}, {} {} remaining",
                    fixed_issues,
                    if fixed_issues == 1 { "fix" } else { "fixes" },
                    relative_path.display().cyan(),
                    remaining,
                    if remaining == 1 { "issue" } else { "issues" }
                );
            }
        }
    }

    Ok(())
}

fn byte_to_char_index(s: &str, byte_index: usize) -> usize {
    s.char_indices()
        .nth(byte_index)
        .map(|(i, _)| i)
        .unwrap_or(byte_index)
}

fn create_range_json(source: &str, span: &Span) -> Option<serde_json::Value> {
    if let (Some((start_line, start_col)), Some((end_line, end_col))) = (
        byte_to_line_col(source, span.start as usize),
        byte_to_line_col(source, span.end as usize),
    ) {
        Some(serde_json::json!({
            "start": {"line": start_line, "character": start_col},
            "end": {"line": end_line, "character": end_col}
        }))
    } else {
        None
    }
}

fn diagnostic_to_json(diag: &Diagnostic, file_db: &FileDb) -> serde_json::Value {
    let file_info = file_db
        .get_by_id(diag.file_id)
        .expect("File info should exist for diagnostic");
    let file_path = file_info.path().to_string_lossy().to_string();
    let source = file_info.source().source.as_ref();

    let severity = match diag.severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
        Severity::Fatal => "error",
        Severity::Help => "info",
    };

    let mut annotations_json = Vec::new();
    for annotation in &diag.annotations {
        if let Some(range) = create_range_json(source, &annotation.span) {
            annotations_json.push(serde_json::json!({
                "range": range,
                "message": annotation.message,
                "is_primary": annotation.is_primary
            }));
        }
    }

    let mut fixes_json = Vec::new();
    for fix in &diag.fixes {
        let mut edits_json = Vec::new();
        for edit in &fix.edits {
            if let Some(range) = create_range_json(source, &edit.span) {
                edits_json.push(serde_json::json!({
                    "range": range,
                    "newText": &edit.replacement
                }));
            }
        }
        let applicability = match fix.applicability {
            Applicability::Auto => "auto",
            Applicability::Manual => "manual",
        };
        fixes_json.push(serde_json::json!({
            "message": &fix.message,
            "edits": edits_json,
            "applicability": applicability
        }));
    }

    serde_json::json!({
        "file": file_path,
        "severity": severity,
        "name": &diag.name,
        "code": &diag.code,
        "message": &diag.message,
        "annotations": annotations_json,
        "fixes": fixes_json,
        "source": "tolk"
    })
}

fn byte_to_line_col(source: &str, byte_offset: usize) -> Option<(u32, u32)> {
    let mut line = 0u32;
    let mut col = 0u32;
    let mut current_byte = 0usize;

    for (i, ch) in source.char_indices() {
        if i >= byte_offset {
            return Some((line, col));
        }

        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
        current_byte = i;
    }

    // If we reach the end, return the last position
    if current_byte < byte_offset && byte_offset <= source.len() {
        Some((line, col + (byte_offset - current_byte) as u32))
    } else {
        None
    }
}
