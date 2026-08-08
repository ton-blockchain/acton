use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const TYPES_ONLY_FILE: &str = r"
struct (0x12345678) TypesOnlyMessage {
    value: uint32
}

contract TypesOnly {
    incomingMessages: TypesOnlyMessage
}
";

const HELPER_WITHOUT_ENTRYPOINT: &str = r"
fun helper(value: int): int {
    return value + 1;
}
";

#[test]
fn build_accepts_contract_name_and_explicit_path() {
    ProjectBuilder::new("x-stdlib-build-by-name-and-path")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "build_paths",
            r#"
            import "../../lib/build"
            import "../../lib/testing/expect"

            get fun `test build by name and path`() {
                val byName = build("simple");
                val byPath = build("simple", "contracts/simple.tolk");

                expect(byName).toEqual(byPath);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_accepts_contract_name_and_explicit_path/build_accepts_contract_name_and_explicit_path.stdout.txt",
        );
}

#[test]
fn build_allows_no_entrypoint_for_explicit_types_tolk_path() {
    ProjectBuilder::new("x-stdlib-build-types-tolk-no-entrypoint")
        .contract("simple", SIMPLE_CONTRACT)
        .raw_file("contracts/precompiled.types.tolk", TYPES_ONLY_FILE)
        .test_file(
            "build_types_path",
            r#"
            import "../../lib/build"

            get fun `test build explicit types path without entrypoint`() {
                val _ = build("precompiled", "contracts/precompiled.types.tolk");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_accepts_contract_name_and_explicit_path/build_allows_no_entrypoint_for_explicit_types_tolk_path.stdout.txt",
        );
}

#[test]
fn build_requires_entrypoint_for_explicit_non_types_tolk_path() {
    ProjectBuilder::new("x-stdlib-build-helper-tolk-no-entrypoint")
        .contract("simple", SIMPLE_CONTRACT)
        .raw_file("contracts/helper.tolk", HELPER_WITHOUT_ENTRYPOINT)
        .test_file(
            "build_helper_path",
            r#"
            import "../../lib/build"

            get fun `test build explicit non types path without entrypoint`() {
                val _ = build("helper", "contracts/helper.tolk");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("has no entrypoint")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_accepts_contract_name_and_explicit_path/build_requires_entrypoint_for_explicit_non_types_tolk_path.stdout.txt",
        );
}

#[test]
fn build_reports_missing_contract_when_path_is_omitted() {
    ProjectBuilder::new("x-stdlib-build-contract-not-found")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "build_missing",
            r#"
            import "../../lib/build"

            get fun `test build contract not found`() {
                val _ = build("missing");
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Contract missing not found in Acton.toml")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/build_accepts_contract_name_and_explicit_path/build_reports_missing_contract_when_path_is_omitted.stdout.txt",
        );
}
