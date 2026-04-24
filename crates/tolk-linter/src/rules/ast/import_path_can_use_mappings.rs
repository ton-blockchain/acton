use crate::rules::diagnostic::{Annotation, Applicability, Diagnostic, Edit, Fix};
use crate::rules::violation::Violation;
use crate::{Checker, FixAvailability};
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};
use tolk_macros::ViolationMetadata;
use tolk_resolver::file_index::{FileId, Span};

/// ### What it does
/// Warns when a relative import path can be replaced with a configured `@mapping/...` path.
///
/// ### Why is this bad?
/// Relative imports are harder to maintain in larger projects and become fragile when files move.
///
/// ### Example
/// ```tolk twoslash
/// import "../libs/math.tolk";
/// //      ^^^^^^^^^^^^^^^^^ E018: import path can use mappings
/// ```
///
/// Use instead:
/// ```tolk
/// import "@libs/math";
/// ```
#[derive(ViolationMetadata)]
#[violation_metadata(stable_since = "v0.0.1")]
pub struct ImportPathCanUseMappings;

impl Violation for ImportPathCanUseMappings {
    const FIX_AVAILABILITY: FixAvailability = FixAvailability::Sometimes;

    fn message(&self) -> String {
        "import path can use mappings".to_string()
    }
}

pub fn check_file(checker: &mut Checker, file_id: FileId) -> Option<()> {
    let project_index = checker.type_db.project_index;
    let imports = project_index.imports().get(&file_id)?;
    if imports.is_empty() {
        // fast path for files without imports
        return None;
    }

    let mappings = project_index.mappings();
    if mappings.is_empty() {
        // fast path for projects without mappings
        return None;
    }

    let project_root = checker.project_root()?.to_path_buf();

    for resolved_import in imports {
        let import = resolved_import.import();

        if import.path.starts_with('@') {
            // already uses mappings
            continue;
        }

        if !import.path.starts_with("..") && !import.path.starts_with('.') {
            // don't process path like "types"
            continue;
        }

        let Some(target_id) = resolved_import.target() else {
            continue;
        };

        let Some(target_file) = checker.file_db.get_by_id(target_id) else {
            continue;
        };

        if !target_file.is_workspace_file() && !target_file.is_acton_file() {
            continue;
        }

        let Some(mapped_import) =
            suggest_mapped_import(&target_file.index().path, &project_root, mappings)
        else {
            continue;
        };

        if normalize_import_path(import.path.as_ref()) == normalize_import_path(&mapped_import) {
            continue;
        }

        fire_diagnostic(checker, file_id, import.span, &mapped_import);
    }

    Some(())
}

fn suggest_mapped_import(
    target_abs: &Path,
    project_root: &Path,
    mappings: &FxHashMap<String, String>,
) -> Option<String> {
    let mut best_match: Option<(usize, String)> = None;

    for (mapping_name, mapping_target) in mappings {
        let mapping_abs = normalize_abs_path(project_root, Path::new(mapping_target));
        if !target_abs.starts_with(&mapping_abs) {
            continue;
        }

        let Ok(relative_suffix) = target_abs.strip_prefix(&mapping_abs) else {
            continue;
        };

        let mapping_name = if mapping_name.starts_with('@') {
            mapping_name.clone()
        } else {
            format!("@{mapping_name}")
        };

        let mut import_path = PathBuf::from(mapping_name);
        if !relative_suffix.as_os_str().is_empty() {
            import_path = import_path.join(relative_suffix);
        }

        let score = mapping_abs.components().count();
        let import_path = format_import_path(import_path.as_path());

        if best_match
            .as_ref()
            .is_none_or(|(best_score, _)| score > *best_score)
        {
            best_match = Some((score, import_path));
        }
    }

    best_match.map(|(_, path)| path)
}

fn normalize_abs_path(project_root: &Path, path: &Path) -> PathBuf {
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    };

    dunce::canonicalize(&abs_path).unwrap_or(abs_path)
}

fn format_import_path(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    path.trim_start_matches("./")
        .trim_end_matches(".tolk")
        .to_owned()
}

fn normalize_import_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_end_matches(".tolk")
        .to_owned()
}

#[cold]
fn fire_diagnostic(checker: &mut Checker, file_id: FileId, span: Span, mapped_import: &str) {
    let diagnostic = Diagnostic::warning_for(file_id, ImportPathCanUseMappings)
        .with_annotations(vec![Annotation {
            span,
            message: Some(format!("this import can use mapping `{mapped_import}`")),
            is_primary: true,
            tags: vec![],
        }])
        .with_fixes(vec![Fix {
            message: "replace import path with mapping".to_string(),
            edits: vec![Edit {
                span,
                replacement: format!("import \"{mapped_import}\""),
                file_id,
            }],
            applicability: Applicability::Auto,
        }])
        .with_help(format!(
            "replace this import with `import \"{mapped_import}\";`"
        ));

    checker.emit_diagnostic(diagnostic);
}
