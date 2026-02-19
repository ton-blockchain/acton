//! Reserved integration test module for subagent AU.
//!
//! Ownership boundary for agent AU:
//! - tests/integration/test-runner/test_runner_stdlib_au_fs_tests.rs
//! - tests/integration/snapshots/test-runner/test_runner_stdlib_au_fs_tests/**
//! - tests/integration/testdata/test_std_agent_au/**
//! - tests/support/test_std_agent_au/** (optional)
//!
//! Required test name prefix:
//! - au_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const FS_IMPORTS: &str = r#"
import "../../lib/fs"
import "../../lib/testing/expect"
"#;

fn run_fs_case(
    project_name: &str,
    fixture_files: &[(&str, &str)],
    test_body: &str,
    snapshot_path: &str,
) {
    let test_code = format!("{FS_IMPORTS}\n{test_body}\n");
    let mut builder = ProjectBuilder::new(project_name).test_file("fs_behavior", &test_code);

    for (path, content) in fixture_files {
        builder = builder.raw_file(path, content);
    }

    builder
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn fs_read_file_returns_null_for_non_utf8_binary_fixture() {
    let test_code = format!(
        r#"
{FS_IMPORTS}

get fun `test-au-fs-read-non-utf8-binary-fixture`() {{
    val binaryLike = fs.readFile("fixtures/au-non-utf8.bin");
    expect(binaryLike).toBeNull();
}}
"#
    );

    let project = ProjectBuilder::new("au-stdlib-fs-non-utf8-binary-fixture")
        .test_file("fs_binary_fixture", &test_code)
        .build();

    let binary_path = project.path().join("fixtures/au-non-utf8.bin");
    std::fs::create_dir_all(
        binary_path
            .parent()
            .expect("Binary fixture path must have a parent directory"),
    )
    .expect("Failed to create binary fixture directory");
    std::fs::write(
        &binary_path,
        [0x00, 0xFF, 0x80, 0x10, 0xB5, 0xEE, 0x9C, 0x72],
    )
    .expect("Failed to write non-UTF8 binary fixture");

    project
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_au_fs_tests/au_stdlib_fs_read_file_returns_null_for_non_utf8_binary_fixture.stdout.txt",
        );
}

#[test]
fn fs_read_file_keeps_binary_like_text_content_across_normalized_paths() {
    run_fs_case(
        "au-stdlib-fs-binary-like-content-normalized-paths",
        &[(
            "fixtures/au-binary-like.txt",
            "B5EE9C72\r\n00FF10AA\r\n7F00\r\n",
        )],
        r#"
get fun `test-au-fs-read-binary-like-content-normalized-paths`() {
    val direct = fs.readFile("fixtures/au-binary-like.txt");
    val viaDot = fs.readFile("./fixtures/au-binary-like.txt");
    val viaParent = fs.readFile("fixtures/../fixtures/au-binary-like.txt");

    expect(direct).toBeNotNull();
    expect(viaDot).toBeNotNull();
    expect(viaParent).toBeNotNull();

    expect(direct!).toEqual(viaDot!);
    expect(direct!).toEqual(viaParent!);

    expect(direct!).toEqual("B5EE9C72\r\n00FF10AA\r\n7F00\r\n");
    expect(direct!).toNotEqual("B5EE9C72\n00FF10AA\n7F00\n");
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_au_fs_tests/au_stdlib_fs_read_file_keeps_binary_like_text_content_across_normalized_paths.stdout.txt",
    );
}
