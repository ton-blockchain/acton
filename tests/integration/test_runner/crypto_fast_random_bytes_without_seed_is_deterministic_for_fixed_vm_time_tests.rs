use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CRYPTO_IMPORTS: &str = r#"
import "../../lib/crypto/crypto"
import "../../lib/testing/expect"
import "../../lib/vm/vm"
"#;

fn run_crypto_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CRYPTO_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("crypto_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn crypto_fast_random_bytes_without_seed_is_deterministic_for_fixed_vm_time() {
    run_crypto_case(
        "az-stdlib-fast-random-without-seed-fixed-time",
        r#"
get fun `test-az-stdlib-fast-random-without-seed-fixed-time`() {
    vm.setTime(1700004321);

    val randomA = crypto.getFastRandomBytes(64);
    val randomB = crypto.getFastRandomBytes(64);

    expect(blockchain.now()).toEqual(1700004321);
    expect(randomA.remainingBitsCount()).toEqual(64 * 8);
    expect(randomA.remainingRefsCount()).toEqual(0);
    // BUG: getFastRandomBytes without seed should be deterministic for fixed vm time; expected equal slices, got different values.
    expect(randomA).toEqual(randomB);
}
"#,
        "integration/snapshots/test-runner/crypto_fast_random_bytes_without_seed_is_deterministic_for_fixed_vm_time/crypto_fast_random_bytes_without_seed_is_deterministic_for_fixed_vm_time.stdout.txt",
    );
}
