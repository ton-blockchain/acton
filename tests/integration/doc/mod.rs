use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
fn test_doc_tvm_add_text() {
    let project = ProjectBuilder::new("doc-tvm-add-text").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("ADD")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_add.stdout.txt");
}

#[test]
fn test_doc_tvm_add_json() {
    let project = ProjectBuilder::new("doc-tvm-add-json").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("add")
        .arg("--json")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_add_json.stdout.json.txt");
}

#[test]
fn test_doc_tvm_multi_text() {
    let project = ProjectBuilder::new("doc-tvm-multi-text").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("ADD")
        .arg("SUB")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_multi.stdout.txt");
}

#[test]
fn test_doc_tvm_multi_json() {
    let project = ProjectBuilder::new("doc-tvm-multi-json").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("ADD")
        .arg("SUB")
        .arg("--json")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_multi.stdout.json.txt");
}

#[test]
fn test_doc_tvm_unknown_instruction() {
    let project = ProjectBuilder::new("doc-tvm-unknown").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("ADDD")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/doc/test_doc_tvm_unknown.stderr.txt",
        );
}

#[test]
fn test_doc_tvm_find_text() {
    let project = ProjectBuilder::new("doc-tvm-find-text").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("SENRAWMSG")
        .arg("--find")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_find.stdout.txt");
}

#[test]
fn test_doc_tvm_find_json() {
    let project = ProjectBuilder::new("doc-tvm-find-json").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("SENRAWMSG")
        .arg("--find")
        .arg("--json")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/doc/test_doc_tvm_find_json.stdout.json.txt",
        );
}

#[test]
fn test_doc_tvm_find_multi_text() {
    let project = ProjectBuilder::new("doc-tvm-find-multi-text").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("SENRAWMSG")
        .arg("outcomng")
        .arg("--find")
        .arg("--description")
        .run()
        .success()
        .assert_snapshot_matches("integration/snapshots/doc/test_doc_tvm_find_multi.stdout.txt");
}

#[test]
fn test_doc_tvm_find_multi_json() {
    let project = ProjectBuilder::new("doc-tvm-find-multi-json").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("SENRAWMSG")
        .arg("outcomng")
        .arg("--find")
        .arg("--description")
        .arg("--json")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/doc/test_doc_tvm_find_multi.stdout.json.txt",
        );
}

#[test]
fn test_doc_tvm_find_description_text() {
    let project = ProjectBuilder::new("doc-tvm-find-description-text").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("outcomng")
        .arg("--find")
        .arg("--description")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/doc/test_doc_tvm_find_description.stdout.txt",
        );
}

#[test]
fn test_doc_tvm_find_without_description_flag() {
    let project = ProjectBuilder::new("doc-tvm-find-description-missing-flag").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("outcomng")
        .arg("--find")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/doc/test_doc_tvm_find_without_description.stderr.txt",
        );
}

#[test]
fn test_doc_tvm_description_requires_find_flag() {
    let project = ProjectBuilder::new("doc-tvm-description-requires-find").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("ADD")
        .arg("--description")
        .run()
        .failure()
        .assert_stderr_contains("--find");
}

#[test]
fn test_doc_tvm_empty_sub_category_does_not_print_separator() {
    let project = ProjectBuilder::new("doc-tvm-empty-sub-category").build();

    project
        .acton()
        .arg("doc")
        .arg("tvm")
        .arg("DICTIADDGETREF")
        .run()
        .success()
        .assert_contains("Category:")
        .assert_contains("dictionary")
        .assert_not_contains("Category:      dictionary /");
}
