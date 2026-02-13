use crate::commands::common::error_fmt;
use acton_config::config::{ActonConfig, ContractConfig};
use anyhow::anyhow;
use globset::{Glob, GlobSetBuilder};
use owo_colors::OwoColorize;
use serde_json;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tolk_linter::Checker;
use tolk_linter::diagnostic::{Annotation, Diagnostic, Severity};
use tolk_resolver::file_db::FileDb;
use tolk_resolver::file_index::Span;
use tolk_resolver::project_index::ProjectIndex;
use tolk_resolver::symbol_resolver::resolve;
use tolk_ty::TypeDb;
use tolk_ty::TypeInterner;
use tolk_ty::infer;
use walkdir::WalkDir;

mod check_explain;
mod check_list;
mod compiler;
mod fix;
mod json;
mod pos;

pub fn check_cmd(
    fix: bool,
    json: bool,
    explain: Option<String>,
    list_lint_rules: bool,
    target: Option<String>,
) -> anyhow::Result<()> {
    if list_lint_rules {
        return check_list::check_list_cmd();
    }
    if let Some(code) = explain {
        return check_explain::check_explain_cmd(&code);
    }

    let config = ActonConfig::load()?;

    let cwd = std::env::current_dir()?;

    let now = Instant::now();
    let files = find_files(&cwd)?;
    log::info!("found {} files in {:?}", files.len(), now.elapsed());

    let stdlib = find_stdlib()?;
    let acton_stdlib = find_acton_stdlib()?;
    let common_tolk = stdlib.join("common.tolk");

    let file_db = FileDb::new(stdlib, Some(acton_stdlib));

    // We need stdlib for all targets so preprocess it before all.
    if common_tolk.exists() {
        file_db.process(&common_tolk)?;
    }

    let mut all_diagnostics = Vec::new();

    if let Some(target) = target {
        if target.ends_with(".tolk") {
            let contract_diagnostics =
                check_test_file(Path::new(&target), &file_db, fix, json, &config)?;
            all_diagnostics.extend(contract_diagnostics);
        } else {
            let contract = config
                .get_contract(&target)
                .ok_or_else(|| anyhow!(error_fmt::contract_not_found(&config, &target)))?;
            let contract_diagnostics =
                check_contract(&target, contract, &file_db, fix, json, &config)?;
            all_diagnostics.extend(contract_diagnostics);
        }
    } else {
        let contracts = config.contracts().cloned().unwrap_or_default();
        for (contract_id, contract) in contracts {
            let contract_diagnostics =
                check_contract(&contract_id, &contract, &file_db, fix, json, &config)?;
            all_diagnostics.extend(contract_diagnostics);
        }

        for file in files {
            let Some(name) = file.file_name() else {
                continue;
            };
            if name.to_string_lossy().ends_with(".test.tolk") {
                let contract_diagnostics = check_test_file(&file, &file_db, fix, json, &config)?;
                all_diagnostics.extend(contract_diagnostics);
            }
        }
    }

    if json {
        let json_output = serde_json::json!({
            "success": true,
            "diagnostics": all_diagnostics.iter().map(|d| json::diagnostic_to_json(d, &file_db)).collect::<Vec<_>>()
        });
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        let shown_diagnostics = if fix {
            fix::filter_fixed_diagnostics(&all_diagnostics)
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
    let path_to_stdlib = PathBuf::from(".acton/tolk-stdlib");
    if !path_to_stdlib.exists() {
        anyhow::bail!(
            "cannot find Tolk stdlib in .acton/, did you run {}?",
            "acton init".yellow()
        );
    }

    Ok(path_to_stdlib.canonicalize()?)
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

    if !json {
        println!("    {} {}", "Checking".green().bold(), config.name,);
    }

    let root = PathBuf::from(&config.src).canonicalize()?;
    let lint_settings = Checker::build_settings(acton_config, Some(contract_id));

    check_root_file(&root, file_db, fix, json, lint_settings, acton_config)
}

fn check_test_file(
    file: &Path,
    file_db: &FileDb,
    fix: bool,
    json: bool,
    acton_config: &ActonConfig,
) -> anyhow::Result<Vec<Diagnostic>> {
    let root = file.canonicalize()?;
    let current_dir = std::env::current_dir().unwrap_or_default();
    let relative_root = pathdiff::diff_paths(&root, &current_dir).unwrap_or_else(|| root.clone());

    if !json {
        println!(
            "    {} {}",
            "Checking".green().bold(),
            relative_root.display()
        );
    }

    let lint_settings = Checker::build_settings(acton_config, None);

    check_root_file(&root, file_db, fix, json, lint_settings, acton_config)
}

fn check_root_file(
    root: &Path,
    file_db: &FileDb,
    fix: bool,
    json: bool,
    lint_settings: HashMap<tolk_linter::Rule, acton_config::config::LintLevel>,
    acton_config: &ActonConfig,
) -> anyhow::Result<Vec<Diagnostic>> {
    let file_info = file_db.process(root)?;
    let file_source = file_info.source().source.clone();

    let mut all_diagnostics = vec![];

    let has_compiler_errors =
        compiler::check_with_compiler(root, file_db, acton_config, &mut all_diagnostics)?;

    let parse_errors = file_info.source().errors();

    if has_compiler_errors {
        // don't possibly duplicate parsing errors if we have compiler errors
        for parse_error in parse_errors {
            let start_byte = pos::byte_offset_from_point(&parse_error.span.start, &file_source);
            let end_byte = pos::byte_offset_from_point(&parse_error.span.end, &file_source);

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
        let Some(file_to_infer) = file_db.get_by_id(*file_to_check) else {
            continue;
        };
        let mut file_body_types = HashMap::new();

        for decl in file_to_infer.source().top_levels() {
            let Some(index_decl) = file_to_infer.find_declaration(&decl) else {
                continue;
            };

            let res = infer(&mut type_db, file_to_infer.id(), index_decl.id, &decl);
            file_body_types.insert(index_decl.id, res);
        }

        body_types.insert(*file_to_check, file_body_types);
    }
    log::debug!("Infer types took {:?}", now.elapsed());

    // And finally run all inspections provided by checker
    let now = Instant::now();
    let mut checker = Checker::new(file_db, &mut type_db, &body_types).with_settings(lint_settings);

    // locals by file -> file_db -> project_index -> by usage
    // globals one time
    checker.run_once();

    for file_to_check in files_to_check {
        let Some(info) = file_db.get_by_id(file_to_check) else {
            continue;
        };
        if !info.is_workspace_file() {
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
            fix::filter_fixed_diagnostics(&diagnostics)
        } else {
            diagnostics.clone()
        };
        let _ = emit_diagnostics(file_db, &diagnostics_to_show);
    }

    if fix && !json {
        fix::apply_fixes(file_db, &diagnostics)?;
    }

    Ok(diagnostics)
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
            info.index().path.to_string_lossy().to_string(),
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

        // this is diagnostic header printed, with yellow
        term::emit(&mut writer.lock(), &config, &files, &cs_diag)?;

        for fix in &diag.fixes {
            let mut labels = vec![];
            // print edit message (help in green) per edit
            for edit in &fix.edits {
                let edit_file_id = edit.file_id;
                let cs_file_id = *file_id_map.get(&edit_file_id).unwrap_or(&0);
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

fn find_files(root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    const EXCLUDED_DIRS: &[&str] = &[
        ".git",
        ".github",
        ".idea",
        ".acton",
        "node_modules",
        "target",
        "tolk-stdlib",
    ];

    let mut exclude_builder = GlobSetBuilder::new();
    for p in [
        // ... for future ignoring via flags
    ] {
        exclude_builder.add(Glob::new(p)?);
    }
    let excludes = exclude_builder.build()?;

    let it = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if !entry.file_type().is_dir() {
                return true;
            }
            let name = entry.file_name();
            if EXCLUDED_DIRS.iter().any(|d| name == OsStr::new(d)) {
                // fast path
                return false;
            }

            let p = entry.path();
            let rel = p.strip_prefix(root).unwrap_or(p);
            !excludes.is_match(rel)
        });

    let mut out: Vec<PathBuf> = Vec::with_capacity(32);

    for entry in it {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                log::warn!("walk dir error: {err}");
                continue;
            }
        };

        if entry.file_type().is_file() {
            let path = entry.path();

            if let Some(ext) = path.extension() {
                if ext != "tolk" {
                    continue;
                }
            } else {
                continue;
            }

            let rel = path.strip_prefix(root).unwrap_or(path);
            if excludes.is_match(rel) {
                continue;
            }

            out.push(path.to_path_buf());
        }
    }

    out.sort_unstable();
    Ok(out)
}
