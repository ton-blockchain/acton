use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const FS_IMPORTS: &str = r#"
import "../../lib/fs"
import "../../lib/testing/expect"
"#;

fn run_case(
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
fn fs_read_file_returns_content_for_existing_and_normalized_paths() {
    run_case(
        "w-stdlib-fs-existing-and-stable-paths",
        &[("fixtures/w-existing.txt", "line-from-agent-w")],
        r#"
get fun `test-fs-read-existing-and-normalized-paths`() {
    val direct = fs.readFile("fixtures/w-existing.txt");
    val viaDot = fs.readFile("./fixtures/w-existing.txt");
    val viaParent = fs.readFile("fixtures/../fixtures/w-existing.txt");

    expect(direct).toBeNotNull();
    expect(viaDot).toBeNotNull();
    expect(viaParent).toBeNotNull();

    expect(direct!).toEqual("line-from-agent-w");
    expect(viaDot!).toEqual("line-from-agent-w");
    expect(viaParent!).toEqual("line-from-agent-w");
}
"#,
        "integration/snapshots/test-runner/fs_read_file_returns_content_for_existing_and_normalized_paths/fs_read_file_returns_content_for_existing_and_normalized_paths.stdout.txt",
    );
}

#[test]
fn fs_read_file_returns_null_for_missing_file() {
    run_case(
        "w-stdlib-fs-missing-file-null",
        &[],
        r#"
get fun `test-fs-read-missing-file-returns-null`() {
    val missing = fs.readFile("fixtures/w-missing.txt");
    val missingViaParent = fs.readFile("fixtures/../fixtures/w-missing.txt");

    expect(missing).toBeNull();
    expect(missingViaParent).toBeNull();
}
"#,
        "integration/snapshots/test-runner/fs_read_file_returns_content_for_existing_and_normalized_paths/fs_read_file_returns_null_for_missing_file.stdout.txt",
    );
}
