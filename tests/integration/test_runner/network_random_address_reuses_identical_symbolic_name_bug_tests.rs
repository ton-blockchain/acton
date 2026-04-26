use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
"#;

fn run_network_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{NETWORK_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("bo_random_address_reuse", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn network_random_address_reuses_identical_symbolic_name_bug() {
    run_network_success_case(
        "bo-stdlib-network-random-address-reuse",
        r#"
get fun `test bo stdlib network random address reuse`() {
    val first = randomAddress("bo_reused_symbolic_name");
    val second = randomAddress("bo_reused_symbolic_name");

    expect(second).toEqual(first);
}
"#,
        "integration/snapshots/test-runner/network_random_address_reuses_identical_symbolic_name_bug/network_random_address_reuses_identical_symbolic_name_bug.stdout.txt",
    );
}
