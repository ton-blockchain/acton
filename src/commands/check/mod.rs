use crate::commands::common::error_fmt;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, CheckOutputFormat, ContractConfig, LintLevel};
use anyhow::anyhow;
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;
use std::{fs, io};
use tolk_linter::diagnostic::{Annotation, Applicability, Diagnostic, Severity};
use tolk_linter::{Checker, Rule};
use tolk_resolver::file_db::FileDb;
use tolk_resolver::file_index::{FileId, Span};
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
mod output;
mod pos;
mod render;

pub(super) struct LintExcludes {
    project_root: PathBuf,
    patterns: Vec<String>,
    excludes: GlobSet,
}

impl LintExcludes {
    fn from_config(project_root: &Path, config: &ActonConfig) -> anyhow::Result<Self> {
        let patterns = config
            .lint
            .as_ref()
            .and_then(|lint| lint.exclude.clone())
            .unwrap_or_default();

        let mut exclude_builder = GlobSetBuilder::new();
        for pattern in &patterns {
            exclude_builder.add(Glob::new(pattern)?);
        }

        Ok(Self {
            project_root: project_root.to_path_buf(),
            patterns,
            excludes: exclude_builder.build()?,
        })
    }

    fn is_match(&self, path: &Path) -> bool {
        if self.patterns.is_empty() {
            return false;
        }

        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project_root.join(path)
        };
        let relative =
            pathdiff::diff_paths(&absolute, &self.project_root).unwrap_or_else(|| absolute.clone());

        self.excludes.is_match(&relative) || self.excludes.is_match(&absolute)
    }

    fn is_match_file_id(&self, file_db: &FileDb, file_id: FileId) -> bool {
        let Some(info) = file_db.get_by_id(file_id) else {
            return false;
        };
        self.is_match(info.path())
    }
}

fn diagnostics_summary(diagnostics: &[Diagnostic]) -> (usize, usize) {
    let mut error_count = 0;
    let mut warning_count = 0;

    for diagnostic in diagnostics {
        match diagnostic.severity {
            Severity::Warning => warning_count += 1,
            Severity::Error | Severity::Fatal => error_count += 1,
            Severity::Info | Severity::Help => {}
        }
    }

    (error_count, warning_count)
}

pub fn check_cmd(
    fix: bool,
    cli_output_format: Option<CheckOutputFormat>,
    output_file: Option<PathBuf>,
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
    let output_format = cli_output_format
        .or_else(|| {
            config
                .lint
                .as_ref()
                .and_then(|lint| lint.output_format.clone())
        })
        .unwrap_or(CheckOutputFormat::Plain);
    let is_plain_report = output_format == CheckOutputFormat::Plain;
    if is_plain_report && output_file.is_some() {
        anyhow::bail!("output_file cannot be used with plain output format")
    }

    let max_warnings = config
        .lint
        .as_ref()
        .map_or(usize::MAX, |lint| lint.max_warnings);

    let cwd = std::env::current_dir()?;
    let excludes = LintExcludes::from_config(&cwd, &config)?;

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
            let contract_diagnostics = check_test_file(
                Path::new(&target),
                &file_db,
                fix,
                is_plain_report,
                &config,
                &excludes,
            )?;
            all_diagnostics.extend(contract_diagnostics);
        } else {
            let contract = config
                .get_contract(&target)
                .ok_or_else(|| anyhow!(error_fmt::contract_not_found(&config, &target)))?;
            let contract_diagnostics = check_contract(
                &target,
                contract,
                &file_db,
                fix,
                is_plain_report,
                &config,
                &excludes,
            )?;
            all_diagnostics.extend(contract_diagnostics);
        }
    } else {
        let contracts = config.contracts().cloned().unwrap_or_default();
        for (contract_id, contract) in contracts {
            if excludes.is_match(Path::new(&contract.src)) {
                continue;
            }
            let contract_diagnostics = check_contract(
                &contract_id,
                &contract,
                &file_db,
                fix,
                is_plain_report,
                &config,
                &excludes,
            )?;
            all_diagnostics.extend(contract_diagnostics);
        }

        for file in files {
            let Some(name) = file.file_name() else {
                continue;
            };
            if name.to_string_lossy().ends_with(".test.tolk") && !excludes.is_match(&file) {
                let contract_diagnostics =
                    check_test_file(&file, &file_db, fix, is_plain_report, &config, &excludes)?;
                all_diagnostics.extend(contract_diagnostics);
            }
        }
    }

    // Deduplicate all diagnostic for JSON output to avoid duplicate errors in IDEs
    let all_diagnostics = all_diagnostics
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let mut writer: Box<dyn Write> = match output_file {
        Some(path) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }

            let file = fs::File::create(path)?;
            Box::new(BufWriter::new(file))
        }
        None => Box::new(BufWriter::new(io::stdout())),
    };

    match output_format {
        CheckOutputFormat::Plain => {
            show_plain_report(fix, max_warnings, &all_diagnostics, &file_db)?;
        }
        CheckOutputFormat::Json => {
            output::json::write_report(&mut writer, &all_diagnostics, &file_db)?;
        }
        CheckOutputFormat::Sarif => {
            output::sarif::write_report(&mut writer, &all_diagnostics, &file_db, &cwd)?;
        }
    }

    if output_format != CheckOutputFormat::Plain {
        writer.flush()?;

        let (error_count, warning_count) = diagnostics_summary(&all_diagnostics);
        let warning_limit_exceeded = warning_count > max_warnings;
        let is_success = error_count == 0 && !warning_limit_exceeded;

        if !is_success {
            std::process::exit(1);
        }
    }

    Ok(())
}

fn show_plain_report(
    fix: bool,
    max_warnings: usize,
    all_diagnostics: &[Diagnostic],
    file_db: &FileDb,
) -> anyhow::Result<()> {
    if fix {
        fix::apply_fixes(file_db, all_diagnostics)?;
    }

    let mut shown_diagnostics = if fix {
        fix::filter_fixed_diagnostics(all_diagnostics)
    } else {
        Vec::from(all_diagnostics)
    };
    let (error_count, warning_count) = diagnostics_summary(&shown_diagnostics);
    let warning_limit_exceeded = warning_count > max_warnings;

    if !shown_diagnostics.is_empty() {
        shown_diagnostics.sort();
        let first_code = shown_diagnostics
            .iter()
            .find(|d| d.code.is_some())
            .and_then(|d| d.code.clone());

        let mut printed_autofix_notice = false;
        if !fix {
            let count_to_autofix = shown_diagnostics
                .iter()
                .filter(|d| {
                    d.fixes
                        .iter()
                        .any(|f| f.applicability == Applicability::Auto)
                })
                .count();

            if count_to_autofix > 0 {
                let issue_word = if count_to_autofix == 1 {
                    "issue"
                } else {
                    "issues"
                };

                eprintln!();
                eprintln!(
                    "{count_to_autofix} {issue_word} can be fixed automatically, rerun with {} flag.",
                    "--fix".yellow()
                );
                printed_autofix_notice = true;
            }
        }

        if warning_limit_exceeded {
            if !printed_autofix_notice {
                eprintln!();
            }
            eprintln!(
                "Warning limit exceeded: {} {} (max-warnings = {}).",
                warning_count,
                if warning_count == 1 {
                    "warning"
                } else {
                    "warnings"
                },
                max_warnings
            );
        }

        if let Some(code) = first_code {
            eprintln!();
            eprintln!(
                "Use {} to get detailed explanation of a rule.",
                "acton check --explain <CODE>".yellow()
            );
            eprintln!("For example: acton check --explain {}", code);
        }
    }

    if error_count > 0 || warning_limit_exceeded {
        std::process::exit(1);
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

    Ok(dunce::canonicalize(path_to_stdlib)?)
}

fn find_acton_stdlib() -> anyhow::Result<PathBuf> {
    let path_to_acton = PathBuf::from(".acton");
    if !path_to_acton.exists() {
        anyhow::bail!(
            "cannot find Acton in .acton/, did you run {}?",
            "acton init".yellow()
        );
    }

    Ok(dunce::canonicalize(path_to_acton)?)
}

fn check_contract(
    contract_id: &str,
    config: &ContractConfig,
    file_db: &FileDb,
    fix: bool,
    is_plain_report: bool,
    acton_config: &ActonConfig,
    excludes: &LintExcludes,
) -> anyhow::Result<Vec<Diagnostic>> {
    if !config.src.ends_with(".tolk") {
        // skip contracts with .boc sources
        return Ok(vec![]);
    }

    if is_plain_report {
        println!("    {} {}", "Checking".green().bold(), config.name,);
    }

    let root = dunce::canonicalize(PathBuf::from(&config.src))?;
    let lint_settings = Checker::build_settings(acton_config, Some(contract_id));

    check_root_file(
        &root,
        file_db,
        fix,
        is_plain_report,
        lint_settings,
        acton_config,
        excludes,
    )
}

fn check_test_file(
    file: &Path,
    file_db: &FileDb,
    fix: bool,
    is_plain_report: bool,
    acton_config: &ActonConfig,
    excludes: &LintExcludes,
) -> anyhow::Result<Vec<Diagnostic>> {
    let root = dunce::canonicalize(file)?;
    let current_dir = std::env::current_dir().unwrap_or_default();
    let relative_root = pathdiff::diff_paths(&root, &current_dir).unwrap_or_else(|| root.clone());

    if is_plain_report {
        println!(
            "    {} {}",
            "Checking".green().bold(),
            relative_root.display()
        );
    }

    let mut lint_settings = Checker::build_settings(acton_config, None);
    // we can import any files in tests
    lint_settings.insert(Rule::ActonImportInContract, LintLevel::Allow);
    // random is not so important in tests
    lint_settings.insert(Rule::RandomRequiresInitialization, LintLevel::Allow);
    // division is not so important in tests
    lint_settings.insert(Rule::DivideBeforeMultiply, LintLevel::Allow);

    check_root_file(
        &root,
        file_db,
        fix,
        is_plain_report,
        lint_settings,
        acton_config,
        excludes,
    )
}

fn check_root_file(
    root: &Path,
    file_db: &FileDb,
    fix: bool,
    is_plain_report: bool,
    lint_settings: HashMap<Rule, LintLevel>,
    acton_config: &ActonConfig,
    excludes: &LintExcludes,
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
                rule: Rule::CompilerError,
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
        .with_mappings(&acton_config.mappings)
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
        if info.id() != file_info.id() && excludes.is_match(info.path()) {
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
    diagnostics.retain(|diagnostic| {
        diagnostic.rule == Rule::CompilerError
            || diagnostic.file_id == file_info.id()
            || !excludes.is_match_file_id(file_db, diagnostic.file_id)
    });

    if is_plain_report {
        let diagnostics_to_show = if fix {
            fix::filter_fixed_diagnostics(&diagnostics)
        } else {
            diagnostics.clone()
        };
        let _ = render::emit_diagnostics(file_db, &diagnostics_to_show);
    }

    Ok(diagnostics)
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
