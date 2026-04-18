use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const DL_SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const DL_VM_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
"#;

fn run_failure_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DL_VM_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .contract("simple", DL_SIMPLE_CONTRACT)
        .test_file("dl_vm_set_config_unpacked_malformed", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Range check error")
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn vm_set_config_unpacked_empty_tuple_reports_tuple_index_diagnostic() {
    run_failure_case(
        "dl-stdlib-vm-set-config-unpacked-empty-tuple-diagnostic",
        r"
get fun `test dl vm set config unpacked empty tuple diagnostic`() {
    __acton_impl_setConfigParam([], 14);

    var config = testing.getConfig();
    val applied = testing.setConfig(config);
    expect(applied).toBeTrue();
}
",
        "integration/snapshots/test-runner/vm_set_config_unpacked_empty_tuple_reports_tuple_index_diagnostic/vm_set_config_unpacked_empty_tuple_reports_tuple_index_diagnostic.stdout.txt",
    );
}

#[test]
fn vm_set_config_unpacked_single_item_tuple_reports_tuple_index_diagnostic_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/dl_vm_set_config_unpacked_single_item_malformed.test.tolk";
    let source = format!(
        r"{DL_VM_IMPORTS}
get fun `test dl vm set config unpacked single item tuple diagnostic`() {{
    var malformed = [];
    malformed.push(1);
    __acton_impl_setConfigParam(malformed, 14);

    var config = testing.getConfig();
    val applied = testing.setConfig(config);
    expect(applied).toBeTrue();
}}
"
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write dl fixture malformed config-unpacked test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Range check error")
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/vm_set_config_unpacked_empty_tuple_reports_tuple_index_diagnostic/vm_set_config_unpacked_single_item_tuple_reports_tuple_index_diagnostic_in_fixture_project.stdout.txt",
        );
}
