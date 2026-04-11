use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EI_NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
"#;

fn run_network_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{EI_NETWORK_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("ei_crc16_vectors", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn crc16_matches_known_vectors() {
    run_network_success(
        "ei-stdlib-crc16-known-vectors",
        r#"
get fun `test ei stdlib crc16 known vectors`() {
    expect(crc16("")).toEqual(0);
    expect(crc16("hello")).toEqual(50018);
    expect(crc16("123456789")).toEqual(12739);
    expect(crc16("TON")).toEqual(14070);
}
"#,
        "integration/snapshots/test-runner/crc16_matches_known_vectors/crc16_matches_known_vectors.stdout.txt",
    );
}

#[test]
fn crc16_is_deterministic_and_stays_in_u16_range() {
    run_network_success(
        "ei-stdlib-crc16-deterministic-u16-range",
        r#"
get fun `test ei stdlib crc16 deterministic u16 range`() {
    val first = crc16("Acton");
    val second = crc16("Acton");
    val third = crc16("Acton");

    expect(first).toEqual(60291);
    expect(second).toEqual(first);
    expect(third).toEqual(first);
    expect(first >= 0).toBeTrue();
    expect(first <= 65535).toBeTrue();
}
"#,
        "integration/snapshots/test-runner/crc16_matches_known_vectors/crc16_is_deterministic_and_stays_in_u16_range.stdout.txt",
    );
}
