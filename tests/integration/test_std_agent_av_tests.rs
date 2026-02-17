//! Reserved integration test module for subagent AV.
//!
//! Ownership boundary for agent AV:
//! - tests/integration/test_std_agent_av_tests.rs
//! - tests/integration/snapshots/test_std_agent_av/**
//! - tests/integration/testdata/test_std_agent_av/**
//! - tests/support/test_std_agent_av/** (optional)
//!
//! Required test name prefix:
//! - av_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const FS_IMPORTS: &str = r#"
import "../../lib/fs"
import "../../lib/testing/expect"
"#;

fn run_project_builder_fs_case(
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
fn av_stdlib_fs_read_file_normalizes_parent_segments_from_nested_paths() {
    run_project_builder_fs_case(
        "av-stdlib-fs-parent-normalization-nested",
        &[
            ("fixtures/av-root.txt", "line-from-agent-av"),
            ("fixtures/nested/marker.txt", "marker"),
        ],
        r#"
get fun `test-fs-read-parent-normalization-nested`() {
    val direct = fs.readFile("fixtures/av-root.txt");
    val viaParent = fs.readFile("fixtures/nested/../av-root.txt");
    val viaParentAndDot = fs.readFile("./fixtures/nested/.././av-root.txt");

    expect(direct).toBeNotNull();
    expect(viaParent).toBeNotNull();
    expect(viaParentAndDot).toBeNotNull();

    expect(direct!).toEqual("line-from-agent-av");
    expect(viaParent!).toEqual("line-from-agent-av");
    expect(viaParentAndDot!).toEqual("line-from-agent-av");
    expect(direct!).toEqual(viaParent!);
    expect(viaParent!).toEqual(viaParentAndDot!);
}
"#,
        "integration/snapshots/test_std_agent_av/av_stdlib_fs_read_file_normalizes_parent_segments_from_nested_paths.stdout.txt",
    );
}

#[test]
fn av_stdlib_fs_read_file_repeated_normalized_reads_are_stable() {
    let fixture = FixtureProject::load("basic");
    let project_path = fixture.path();

    fs::create_dir_all(project_path.join("fixtures/stable/nested"))
        .expect("failed to create stable fixture directory");
    fs::write(
        project_path.join("fixtures/stable/value.txt"),
        "stable-line-from-agent-av",
    )
    .expect("failed to write stable fixture content");
    fs::write(project_path.join("fixtures/stable/nested/marker.txt"), "marker")
        .expect("failed to write nested marker fixture");

    let test_code = format!(
        r#"
{FS_IMPORTS}
get fun `test-fs-read-repeated-normalized-is-stable`() {{
    val first = fs.readFile("fixtures/stable/nested/../value.txt");
    val second = fs.readFile("fixtures/stable/nested/../value.txt");
    val direct = fs.readFile("./fixtures/stable/value.txt");

    expect(first).toBeNotNull();
    expect(second).toBeNotNull();
    expect(direct).toBeNotNull();

    expect(first!).toEqual("stable-line-from-agent-av");
    expect(second!).toEqual("stable-line-from-agent-av");
    expect(direct!).toEqual("stable-line-from-agent-av");
    expect(first!).toEqual(second!);
    expect(first!).toEqual(direct!);
}}
"#
    );

    fs::write(project_path.join("tests/fs_read_stable.test.tolk"), test_code)
        .expect("failed to write fs_read_stable test");

    fixture
        .acton()
        .test()
        .path("tests/fs_read_stable.test.tolk")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_av/av_stdlib_fs_read_file_repeated_normalized_reads_are_stable.stdout.txt",
        );
}
