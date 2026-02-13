use crate::support::project::ProjectBuilder;

mod deprecated_tests;
mod method_can_be_static_tests;
mod mutable_variable_can_be_immutable_tests;
mod success_tests;
mod used_ignored_identifier_tests;

pub(crate) fn run_fix_test(before: &str, after: &str, name: &str) {
    let project = ProjectBuilder::new(&format!("check-fix-{name}"))
        .contract("main", before)
        .build();

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
