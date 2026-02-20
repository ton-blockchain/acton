use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const MISSING_LIBRARY_HASH: &str =
    "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
const KNOWN_LIBRARY_HASH: &str = "b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27";

#[test]
fn fetch_and_register_library_returns_false_without_panicking_on_transport_errors() {
    let project = ProjectBuilder::new("bl-stdlib-fetch-register-library-false")
        .test_file(
            "fetch_register_library_false",
            &format!(
                r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"

get fun `test-bl-stdlib-fetch-register-library-false`() {{
    expect(net.fetchAndRegisterLibrary("{MISSING_LIBRARY_HASH}")).toBeFalse();
    expect(net.fetchAndRegisterLibrary("not-a-hash")).toBeFalse();
    expect(net.fetchAndRegisterLibrary("")).toBeFalse();

    expect(net.loadLibrary("{MISSING_LIBRARY_HASH}")).toBeNull();
    expect(net.loadLibrary("not-a-hash")).toBeNull();
    expect(net.loadLibrary("")).toBeNull();
}}
"#
            ),
        )
        .build();

    let acton_toml_path = project.path().join("Acton.toml");
    let base_config =
        fs::read_to_string(&acton_toml_path).expect("failed to read Acton.toml for BL test");
    let patched_config = format!(
        r#"{base_config}

[networks.bl-unreachable]
v2-url = "http://127.0.0.1:1/api/v2"
"#
    );
    fs::write(&acton_toml_path, patched_config).expect("failed to patch Acton.toml for BL test");

    project
        .acton()
        .test()
        .env("ACTON_DISABLE_SYSTEM_PROXY", "1")
        .fork_net("custom:bl-unreachable")
        .run()
        .success()
        .assert_passed(1);
}

#[test]
#[ignore = "Blocked in this sandbox: outbound DNS is disabled and local TCP bind is not permitted"]
fn fetch_and_register_library_returns_true_for_known_hash_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/fetch_register_library_true.test.tolk";

    fs::write(
        fixture.path().join(test_path),
        format!(
            r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"

get fun `test-bl-stdlib-fetch-register-library-true`() {{
    expect(net.fetchAndRegisterLibrary("{KNOWN_LIBRARY_HASH}")).toBeTrue();
    expect(net.loadLibrary("{KNOWN_LIBRARY_HASH}")).toBeNotNull();
}}
"#
        ),
    )
    .expect("failed to write BL fixture test file");

    fixture
        .acton()
        .test()
        .path(test_path)
        .arg("--fork-net")
        .arg("testnet")
        .run()
        .success()
        .assert_passed(1);
}
