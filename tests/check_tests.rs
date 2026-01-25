use expect_test::{expect, Expect};
use std::collections::HashMap;
use std::path::PathBuf;
use tolk_linter::Checker;
use tolk_resolver::file_db::FileDb;
use tolk_resolver::project_index::ProjectIndex;
use tolk_resolver::symbol_resolver::resolve;
use tolk_ty::{infer, TypeDb, TypeInterner};

fn check_diagnostics_filtered(files: &[(&str, &str)], filter_rule: Option<&str>, expect: Expect) {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_root = temp_dir.path();

    let mut target_abs_path = None;

    for (path, content) in files {
        let full_path = project_root.join(path);
        std::fs::create_dir_all(full_path.parent().unwrap()).unwrap();
        std::fs::write(&full_path, content).unwrap();

        if target_abs_path.is_none() {
            target_abs_path = Some(full_path.canonicalize().unwrap());
        }
    }

    let target_abs_path = target_abs_path.expect("No files in test");

    let stdlib_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("crates/tolkc/assets/tolk-stdlib")
        .canonicalize()
        .unwrap();

    let file_db = FileDb::new(stdlib_path.clone(), None);

    let mut project_index = ProjectIndex::builder(&file_db, target_abs_path.clone())
        .with_stdlib(stdlib_path)
        .build()
        .unwrap();

    resolve(&file_db, &mut project_index);

    let mut interner = TypeInterner::new();
    let mut type_db = TypeDb::new(&mut interner, &file_db, &project_index);
    let mut body_types = HashMap::new();

    let files_to_check: Vec<_> = project_index.files().keys().copied().collect();

    for file_id in &files_to_check {
        let Some(file_info) = file_db.get_by_id(*file_id) else {
            continue;
        };

        let mut file_body_types = HashMap::new();
        for decl in file_info.source().top_levels() {
            let Some(index_decl) = file_info.find_declaration(&decl) else {
                continue;
            };
            let res = infer(&mut type_db, *file_id, index_decl.id, &decl);
            file_body_types.insert(index_decl.id, res);
        }
        body_types.insert(*file_id, file_body_types);
    }

    let mut checker = Checker::new(&file_db, &mut type_db, &body_types);

    for file_id in &files_to_check {
        let Some(file_info) = file_db.get_by_id(*file_id) else {
            continue;
        };
        if !file_info.is_workspace_file() {
            continue;
        }
        checker.process_file(file_info.source(), *file_id);
    }

    checker.check_project();
    checker.apply_suppressions();

    let mut actual = String::new();
    let mut diagnostics: Vec<_> = checker
        .diagnostics
        .iter()
        .filter(|d| filter_rule.is_none_or(|r| d.name == r))
        .collect();
    diagnostics.sort_by_key(|d| (d.file_id, d.annotations.first().map(|a| a.span.start)));

    for diag in diagnostics {
        let file_info = file_db.get_by_id(diag.file_id).unwrap();
        let file_name = file_info
            .index()
            .path
            .file_name()
            .unwrap()
            .to_string_lossy();

        let span = diag
            .annotations
            .first()
            .map(|a| format!("{}:{}", a.span.start, a.span.end))
            .unwrap_or_default();

        actual.push_str(&format!("[{}] {} at {}\n", file_name, diag.name, span));
    }

    if actual.is_empty() {
        actual.push_str("no diagnostics\n");
    }

    expect.assert_eq(&actual);
}

const BOUNCE_RULE: &str = "missing-on-bounce-handler";

#[test]
fn test_missing_on_bounce_handler_with_bounceable_message() {
    check_diagnostics_filtered(
        &[(
            "contract.tolk",
            r#"
fun onInternalMessage(_in: InMessage) {
    val msg = createMessage({
        bounce: BounceMode.Bounce,
        value: ton("0.1"),
        dest: contract.getAddress(),
        body: null,
    });
    msg.send(SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}
"#,
        )],
        Some(BOUNCE_RULE),
        expect![[r#"
            [contract.tolk] missing-on-bounce-handler at 79:104
        "#]],
    );
}

#[test]
fn test_missing_on_bounce_handler_without_bounce_field() {
    check_diagnostics_filtered(
        &[(
            "contract.tolk",
            r#"
fun onInternalMessage(_in: InMessage) {
    val msg = createMessage({
        value: ton("0.1"),
        dest: contract.getAddress(),
        body: null,
    });
    msg.send(SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}
"#,
        )],
        Some(BOUNCE_RULE),
        expect![[r#"
            [contract.tolk] missing-on-bounce-handler at 55:68
        "#]],
    );
}

#[test]
fn test_no_error_with_nobounce() {
    check_diagnostics_filtered(
        &[(
            "contract.tolk",
            r#"
fun onInternalMessage(_in: InMessage) {
    val msg = createMessage({
        bounce: BounceMode.NoBounce,
        value: ton("0.1"),
        dest: contract.getAddress(),
        body: null,
    });
    msg.send(SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}
"#,
        )],
        Some(BOUNCE_RULE),
        expect!["no diagnostics\n"],
    );
}

#[test]
fn test_no_error_with_on_bounced_message() {
    check_diagnostics_filtered(
        &[(
            "contract.tolk",
            r#"
fun onInternalMessage(_in: InMessage) {
    val msg = createMessage({
        bounce: BounceMode.Bounce,
        value: ton("0.1"),
        dest: contract.getAddress(),
        body: null,
    });
    msg.send(SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun onBouncedMessage(_in: InMessageBounced) {}
"#,
        )],
        Some(BOUNCE_RULE),
        expect!["no diagnostics\n"],
    );
}

#[test]
fn test_transitive_call_through_helper() {
    check_diagnostics_filtered(
        &[(
            "contract.tolk",
            r#"
fun sendBounceable() {
    val msg = createMessage({
        bounce: BounceMode.Bounce,
        value: ton("0.1"),
        dest: contract.getAddress(),
        body: null,
    });
    msg.send(SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun onInternalMessage(_in: InMessage) {
    sendBounceable();
}
"#,
        )],
        Some(BOUNCE_RULE),
        expect![[r#"
            [contract.tolk] missing-on-bounce-handler at 62:87
        "#]],
    );
}

#[test]
fn test_transitive_call_with_nobounce() {
    check_diagnostics_filtered(
        &[(
            "contract.tolk",
            r#"
fun sendNoBounce() {
    val msg = createMessage({
        bounce: BounceMode.NoBounce,
        value: ton("0.1"),
        dest: contract.getAddress(),
        body: null,
    });
    msg.send(SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}

fun onInternalMessage(_in: InMessage) {
    sendNoBounce();
}
"#,
        )],
        Some(BOUNCE_RULE),
        expect!["no diagnostics\n"],
    );
}

#[test]
fn test_no_on_internal_message() {
    check_diagnostics_filtered(
        &[(
            "contract.tolk",
            r#"
fun someFunction() {
    val msg = createMessage({
        bounce: BounceMode.Bounce,
        value: ton("0.1"),
        dest: contract.getAddress(),
        body: null,
    });
    msg.send(SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}
"#,
        )],
        Some(BOUNCE_RULE),
        expect!["no diagnostics\n"],
    );
}

#[test]
fn test_multiple_files_with_import() {
    check_diagnostics_filtered(
        &[
            (
                "contract.tolk",
                r#"
import "helpers.tolk"

fun onInternalMessage(_in: InMessage) {
    sendBounceable();
}
"#,
            ),
            (
                "helpers.tolk",
                r#"
fun sendBounceable() {
    val msg = createMessage({
        bounce: BounceMode.Bounce,
        value: ton("0.1"),
        dest: contract.getAddress(),
        body: null,
    });
    msg.send(SEND_MODE_BOUNCE_ON_ACTION_FAIL);
}
"#,
            ),
        ],
        Some(BOUNCE_RULE),
        expect![[r#"
            [helpers.tolk] missing-on-bounce-handler at 62:87
        "#]],
    );
}
