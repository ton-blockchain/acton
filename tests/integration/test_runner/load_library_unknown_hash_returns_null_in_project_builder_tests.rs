use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use crate::support::toncenter::append_custom_network;
use std::fs;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/io"
import "../../lib/testing/expect"
"#;

const UNKNOWN_LIBRARY_HASH: &str =
    "b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d28";

#[test]
fn load_library_unknown_hash_returns_null_in_project_builder() {
    let source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test bm load library unknown hash project builder`() {{
    val unknown = net.loadLibrary("{UNKNOWN_LIBRARY_HASH}");
    val empty = net.loadLibrary("");
    expect(unknown).toBeNull();
    expect(empty).toBeNull();

    if (unknown == null && empty == null) {{
        println("bm-load-library-null-project-builder");
    }}
}}
"#
    );

    let project = ProjectBuilder::new("bm-stdlib-load-library-unknown-hash-project-builder")
        .test_file("load_library_unknown_hash", &source)
        .build();
    append_custom_network(
        project.path(),
        "bm-missing-net",
        "http://127.0.0.1:1/api/v2",
    );

    project
        .acton()
        .env("ACTON_DISABLE_SYSTEM_PROXY", "1")
        .test()
        .fork_net("custom:bm-missing-net")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("bm-load-library-null-project-builder")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/load_library_unknown_hash_returns_null_in_project_builder/load_library_unknown_hash_returns_null_in_project_builder.stdout.txt",
        );
}

#[test]
fn load_library_unknown_hash_returns_null_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/bm_load_library_unknown_hash.test.tolk";
    let source = format!(
        r#"
{NETWORK_IMPORTS}

get fun `test bm load library unknown hash fixture`() {{
    val unknown = net.loadLibrary("{UNKNOWN_LIBRARY_HASH}");
    val malformed = net.loadLibrary("not-a-hash");
    expect(unknown).toBeNull();
    expect(malformed).toBeNull();

    if (unknown == null && malformed == null) {{
        println("bm-load-library-null-fixture");
    }}
}}
"#
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write bm fixture load library test");
    append_custom_network(
        fixture.path(),
        "bm-missing-net",
        "http://127.0.0.1:1/api/v2",
    );

    fixture
        .acton()
        .env("ACTON_DISABLE_SYSTEM_PROXY", "1")
        .test()
        .path(test_path)
        .fork_net("custom:bm-missing-net")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("bm-load-library-null-fixture")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/load_library_unknown_hash_returns_null_in_project_builder/load_library_unknown_hash_returns_null_in_fixture_project.stdout.txt",
        );
}
