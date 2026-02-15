use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

mod acton_import_tests;
mod asm_safety_comment_tests;
mod deprecated_tests;
mod message_entity_naming_tests;
mod method_can_be_static_tests;
mod mutable_parameter_can_be_immutable_tests;
mod mutable_variable_can_be_immutable_tests;
mod name_case_checker_tests;
mod send_mode_literal_tests;
mod several_not_null_assertions_tests;
mod unauthorized_access_tests;
mod unused_import_tests;
mod used_ignored_identifier_tests;

pub(crate) fn run_fix_test(before: &str, after: &str, name: &str) {
    run_fix_test_with_files(before, after, &[], name);
}

pub(crate) fn run_fix_test_with_files(
    before: &str,
    after: &str,
    files: &[(&str, &str)],
    name: &str,
) {
    let mut builder = ProjectBuilder::new(&format!("check-fix-{name}")).contract("main", before);
    for (path, content) in files {
        builder = builder.file(path, content);
    }

    let project = builder.build();

    project.acton().init().run().success();
    project.acton().check().arg("--fix").run().success();

    let file_path = project.path().join("contracts/main.tolk");
    let actual = std::fs::read_to_string(&file_path)
        .unwrap_or_else(|e| panic!("failed to read fixed file '{}': {}", file_path.display(), e));

    assert_eq!(
        actual.trim(),
        after.trim(),
        "fixed file content mismatch for {}",
        file_path.display()
    );
}

pub(crate) fn run_check_test_with_files(
    group: &str,
    main_content: &str,
    files: &[(&str, &str)],
    name: &str,
) {
    let mut builder = ProjectBuilder::new(&format!("check-{name}")).contract("main", main_content);
    for (path, content) in files {
        builder = builder.file(path, content);
    }

    let project = builder.build();

    project.acton().init().run().success();
    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!("integration/snapshots/check/{group}/{name}.txt"));
}
