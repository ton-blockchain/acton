#![cfg(test)]

use crate::common::test_parser::{TestCase, TestParser};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tolk_resolver::file_db::{FileDb, FileInfo};
use tolk_resolver::project_index::ProjectIndex;
use tolk_resolver::{Resolved, SymbolId, resolve};
use tolk_ty::{InferenceResult, TypeDb, TypeInterner, infer};

#[cfg(test)]
pub mod common;

fn find_caret_positions(input: &str) -> (String, Vec<usize>) {
    let mut clean_input = String::with_capacity(input.len());
    let mut positions = Vec::new();
    let mut cursor = 0;

    while let Some(found) = input[cursor..].find("<caret>") {
        let absolute = cursor + found;
        clean_input.push_str(&input[cursor..absolute]);
        positions.push(clean_input.len());
        cursor = absolute + "<caret>".len();
    }

    clean_input.push_str(&input[cursor..]);
    (clean_input, positions)
}

fn offset_to_line_col(file_info: &FileInfo, offset: usize) -> (usize, usize) {
    let line_offsets = file_info.line_offsets();
    let line = line_offsets
        .binary_search(&offset)
        .unwrap_or_else(|idx| idx.saturating_sub(1));
    let line_start = line_offsets.get(line).copied().unwrap_or(0);
    let source: &str = file_info.source().source.as_ref();
    let col = source[line_start..offset].chars().count();
    (line, col)
}

fn resolve_symbol_at_offset(
    file_info: &FileInfo,
    project_index: &ProjectIndex,
    inferences: &HashMap<SymbolId, InferenceResult>,
    offset: usize,
) -> Option<Resolved> {
    let file_id = file_info.id();
    let resolved_uses = project_index.get_resolved_uses(file_id)?;

    if let Some(usage) = resolved_uses.find_use(offset)
        && !matches!(&usage.resolved, Resolved::Unresolved)
    {
        return Some(usage.resolved.clone());
    }

    if let Some(global_symbol) = project_index.find_symbol_at(file_id, offset) {
        return Some(Resolved::Global(global_symbol.id));
    }

    if let Some(local_def) = resolved_uses.find_local_at(offset) {
        return Some(Resolved::Local(local_def.id));
    }

    let symbol = file_info.find_symbol_at(offset)?;
    let inference = inferences.get(&symbol.id)?;

    if let Some(resolved) = inference.resolve(tolk_resolver::Span::from_offset(offset)) {
        return Some(resolved.resolved.clone());
    }

    inference
        .resolved_refs
        .iter()
        .find(|name_use| name_use.span.contains(offset))
        .map(|resolved| resolved.resolved.clone())
}

fn format_resolved_result(
    file_db: &FileDb,
    source_file: &FileInfo,
    project_index: &ProjectIndex,
    source_offset: usize,
    resolved: Option<Resolved>,
) -> String {
    let (from_line, from_col) = offset_to_line_col(source_file, source_offset);
    let unresolved = || format!("{from_line}:{from_col} unresolved");

    match resolved {
        Some(Resolved::Global(symbol_id)) => {
            let Some(symbol) = project_index.resolve_symbol(symbol_id) else {
                return unresolved();
            };
            let Some(target_file) = file_db.get_by_id(symbol_id.file_id) else {
                return unresolved();
            };
            let (to_line, to_col) = offset_to_line_col(&target_file, symbol.name_span.start());
            format!("{from_line}:{from_col} -> {to_line}:{to_col} resolved")
        }
        Some(Resolved::Local(local_id)) => {
            let Some(resolved_uses) = project_index.get_resolved_uses(local_id.file_id) else {
                return unresolved();
            };
            let Some(local) = resolved_uses.find_local(local_id) else {
                return unresolved();
            };
            let Some(target_file) = file_db.get_by_id(local_id.file_id) else {
                return unresolved();
            };
            let (to_line, to_col) = offset_to_line_col(&target_file, local.def_span.start());
            format!("{from_line}:{from_col} -> {to_line}:{to_col} resolved")
        }
        _ => unresolved(),
    }
}

fn run_resolve_test(test_case: &TestCase) -> String {
    let temp_dir = tempfile::tempdir().unwrap();
    let root_path = temp_dir.path().join("main.tolk");
    let (clean_input, carets) = find_caret_positions(&test_case.input);
    fs::write(&root_path, clean_input).unwrap();
    let root_path = dunce::canonicalize(root_path).unwrap();

    for (file_path, content) in &test_case.files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }

        let (clean_content, _) = find_caret_positions(content);
        fs::write(full_path, clean_content).unwrap();
    }

    let stdlib_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tolkc/assets/tolk-stdlib");
    let file_db = FileDb::new(stdlib_path.clone(), None);
    let stdlib_path = dunce::canonicalize(stdlib_path).unwrap();

    let common_tolk = stdlib_path.join("common.tolk");
    if common_tolk.exists() {
        file_db.process(&common_tolk).unwrap();
    }

    let mut index = ProjectIndex::builder(&file_db, root_path.clone())
        .with_stdlib(file_db.stdlib_path().to_owned())
        .build()
        .expect("Failed to build index");

    let file_info = file_db
        .get_by_path(&root_path)
        .expect("Failed to process file");

    resolve(&file_db, &mut index);

    let mut interner = TypeInterner::new();
    let mut type_db = TypeDb::new(&mut interner, &file_db, &index);

    let mut inferences: HashMap<SymbolId, InferenceResult> = HashMap::new();
    for decl in file_info.source().top_levels() {
        let Some(index_decl) = file_info.find_declaration(&decl) else {
            continue;
        };
        let result = infer(&mut type_db, file_info.id(), index_decl.id, &decl);
        inferences.insert(index_decl.id, result);
    }

    let mut rows = Vec::new();
    for offset in carets {
        let resolved = resolve_symbol_at_offset(&file_info, &index, &inferences, offset);
        rows.push(format_resolved_result(
            &file_db, &file_info, &index, offset, resolved,
        ));
    }

    if rows.is_empty() {
        "ok".to_string()
    } else {
        rows.join("\n")
    }
}

fn run_resolve_tests_from_file(path: &Path) {
    let content = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("Failed to read test file at {:?}: {}", path, e));
    let tests = TestParser::parse_all(&content);

    let has_only = tests.iter().any(|t| t.properties.contains_key("only"));
    let tests_to_run: Vec<&TestCase> = if has_only {
        tests
            .iter()
            .filter(|t| t.properties.contains_key("only"))
            .collect()
    } else {
        tests.iter().collect()
    };

    let mut updates = Vec::new();
    let update_snapshots = std::env::var("UPDATE_SNAPSHOTS")
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    for test in tests_to_run {
        let actual = run_resolve_test(test);
        if update_snapshots {
            updates.push((test.name.clone(), actual));
        } else {
            assert_eq!(actual, test.expected, "Test failed: {}", test.name);
        }
    }

    if update_snapshots && !updates.is_empty() {
        let update_refs: Vec<(&str, &str)> = updates
            .iter()
            .map(|(n, a)| (n.as_str(), a.as_str()))
            .collect();
        TestParser::update_expected_batch(path, update_refs);
    }
}

fn get_test_path(relative_path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testcases/resolving")
        .join(relative_path)
}

#[test]
fn test_basics() {
    run_resolve_tests_from_file(&get_test_path("basic.test"));
}

#[test]
fn test_aliases() {
    run_resolve_tests_from_file(&get_test_path("aliases.test"));
}

#[test]
fn test_enums() {
    run_resolve_tests_from_file(&get_test_path("enums.test"));
}

#[test]
fn test_instance_methods() {
    run_resolve_tests_from_file(&get_test_path("instance-methods.test"));
}

#[test]
fn test_lambdas() {
    run_resolve_tests_from_file(&get_test_path("lambdas.test"));
}

#[test]
fn test_static_methods() {
    run_resolve_tests_from_file(&get_test_path("static-methods.test"));
}

#[test]
fn test_struct_fields() {
    run_resolve_tests_from_file(&get_test_path("struct-fields.test"));
}

#[test]
fn test_type_parameters() {
    run_resolve_tests_from_file(&get_test_path("type-parameters.test"));
}

#[test]
fn test_array_methods() {
    run_resolve_tests_from_file(&get_test_path("array-methods.test"));
}
