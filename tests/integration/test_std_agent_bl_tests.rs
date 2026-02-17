//! Reserved integration test module for subagent BL.
//!
//! Ownership boundary for agent BL:
//! - tests/integration/test_std_agent_bl_tests.rs
//! - tests/integration/snapshots/test_std_agent_bl/**
//! - tests/integration/testdata/test_std_agent_bl/**
//! - tests/support/test_std_agent_bl/** (optional)
//!
//! Required test name prefix:
//! - bl_stdlib_

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const MISSING_LIBRARY_HASH: &str =
    "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
const KNOWN_LIBRARY_HASH: &str =
    "b993c68c596425f05d1bc492d7c03e2979ab669901ed5a57e35e6dd4d6089d27";

#[test]
fn bl_stdlib_fetch_and_register_library_panics_instead_of_returning_false_without_network_bug() {
    ProjectBuilder::new("bl-stdlib-fetch-register-library-false")
        .test_file(
            "bl_stdlib_fetch_register_library_false",
            &format!(
                r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"

get fun `test-bl-stdlib-fetch-register-library-false`() {{
    // BUG: fetchAndRegisterLibrary should gracefully return false on transport errors,
    // expected false, got process-level panic in FFI reqwest path.
    expect(net.fetchAndRegisterLibrary("{MISSING_LIBRARY_HASH}")).toBeFalse();
    expect(net.loadLibrary("{MISSING_LIBRARY_HASH}")).toBeNull();
}}
"#
            ),
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_contains("Attempted to create a NULL object.")
        .assert_contains("panic in a function that cannot unwind");
}

#[test]
#[ignore = "Blocked in this sandbox: outbound DNS is disabled and local TCP bind is not permitted"]
fn bl_stdlib_fetch_and_register_library_returns_true_for_known_hash_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/bl_stdlib_fetch_register_library_true.test.tolk";

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
