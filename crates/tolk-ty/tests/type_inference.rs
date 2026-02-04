#![cfg(test)]

use crate::common::test_parser::{TestCase, TestParser};
use std::fs;
use std::path::{Path, PathBuf};
use tolk_resolver::file_db::FileDb;
use tolk_resolver::project_index::ProjectIndex;
use tolk_resolver::{AstNodeSpanExt, resolve};
use tolk_ty::TypeDb;
use tolk_ty::TypeInterner;
use tolk_ty::infer;

#[cfg(test)]
pub mod common;

struct TypePosition {
    offset: usize,
    expected_type: String,
}

fn find_type_positions(input: &str) -> Vec<TypePosition> {
    let mut positions = Vec::new();
    let lines: Vec<&str> = input.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        if line.contains("//!")
            && let Some(caret_position) = line.find('^')
        {
            let expected_type = line[caret_position + 1..].trim().to_string();
            if i > 0 {
                // Offset of the character in the previous line
                let prev_line = lines[i - 1];
                let mut prev_line_offset = 0;
                for (idx, (offset, _)) in prev_line.char_indices().enumerate() {
                    if idx == caret_position {
                        prev_line_offset = offset;
                        break;
                    }
                }

                // Calculate absolute offset
                let mut absolute_offset = 0;
                for l in &lines[..i - 1] {
                    absolute_offset += l.len() + 1; // +1 for \n
                }
                absolute_offset += prev_line_offset;

                positions.push(TypePosition {
                    offset: absolute_offset,
                    expected_type,
                });
            }
        }
    }
    positions
}

fn run_type_test(test_case: &TestCase) -> String {
    let temp_dir = tempfile::tempdir().unwrap();
    let root_path = temp_dir.path().join("main.tolk");
    fs::write(&root_path, &test_case.input).unwrap();
    let root_path = root_path.canonicalize().unwrap();

    for (file_path, content) in &test_case.files {
        let full_path = temp_dir.path().join(file_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(full_path, content).unwrap();
    }

    let stdlib_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tolkc/assets/tolk-stdlib");
    let file_db = FileDb::new(stdlib_path.clone(), None);
    let stdlib_path = stdlib_path.canonicalize().unwrap();

    let common_tolk = stdlib_path.join("common.tolk");
    // We need stdlib for all targets so preprocess it before all.
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

    let mut errors = Vec::new();
    let positions = find_type_positions(&test_case.input);

    for decl in file_info.source().top_levels() {
        let Some(index_decl) = file_info.find_declaration(&decl) else {
            continue;
        };

        let result = infer(&mut type_db, file_info.id(), index_decl.id, &decl);

        let decl_span = decl.span();
        for pos in &positions {
            if !decl_span.contains(pos.offset) {
                continue;
            }

            let mut found_type = None;

            for (span, ty_id) in &result.expression_types {
                if span.start() == pos.offset {
                    found_type = Some(*ty_id);
                }
            }

            if let Some(ty_id) = found_type {
                let actual_type = type_db.intrn.display(ty_id).to_string();
                if actual_type != pos.expected_type {
                    errors.push(format!(
                        "type inference error at offset {}: expected {}, got {}",
                        pos.offset, pos.expected_type, actual_type
                    ));
                }
            } else {
                errors.push(format!(
                    "type inference error at offset {}: unknown type",
                    pos.offset,
                ));
            }
        }
    }

    if errors.is_empty() {
        "ok".to_string()
    } else {
        errors.join("\n")
    }
}

fn run_tests_from_file(path: &Path) {
    // #[allow(unsafe_code)]
    // // SAFETY: set ones anyway
    // unsafe {
    //    std::env::set_var("UPDATE_SNAPSHOTS", "1")
    // }

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
        let actual = run_type_test(test);
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
        .join("tests/testcases/types")
        .join(relative_path)
}

#[test]
fn test_types_basic() {
    run_tests_from_file(&get_test_path("basic.test"));
}

#[test]
fn test_types_vars() {
    run_tests_from_file(&get_test_path("vars.test"));
}

#[test]
fn test_types_builtin() {
    run_tests_from_file(&get_test_path("builtin-types.test"));
}

#[test]
fn test_types_funcs() {
    run_tests_from_file(&get_test_path("funcs.test"));
}

#[test]
#[ignore]
fn test_types_map() {
    run_tests_from_file(&get_test_path("map.test"));
}

#[test]
fn test_types_unions() {
    run_tests_from_file(&get_test_path("unions.test"));
}
