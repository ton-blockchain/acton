use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const DJ_SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const DJ_VM_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/vm/vm"
"#;

fn run_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DJ_VM_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("simple", DJ_SIMPLE_CONTRACT)
        .test_file("dj_vm_logical_time_slots", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn vm_set_block_and_logical_time_are_independent_in_c7_slots() {
    run_success_case(
        "dj-stdlib-vm-set-block-and-logical-time-independent-slots",
        r#"
get fun `test-dj-vm-set-block-and-logical-time-independent-slots`() {
    val c7Before = vm.getC7();
    val paramsBefore = c7Before.get(0) as tuple;
    val nowBefore = paramsBefore.get(3) as int;
    val blockLtBefore = paramsBefore.get(4) as int;
    val logicalLtBefore = paramsBefore.get(5) as int;

    vm.setBlockLogicalTime(blockLtBefore + 101);
    val c7AfterBlock = vm.getC7();
    val paramsAfterBlock = c7AfterBlock.get(0) as tuple;
    expect(paramsAfterBlock.get(3) as int).toEqual(nowBefore);
    expect(paramsAfterBlock.get(4) as int).toEqual(blockLtBefore + 101);
    expect(paramsAfterBlock.get(5) as int).toEqual(logicalLtBefore);

    vm.setLogicalTime(logicalLtBefore + 202);
    val c7AfterLogical = vm.getC7();
    val paramsAfterLogical = c7AfterLogical.get(0) as tuple;
    expect(paramsAfterLogical.get(3) as int).toEqual(nowBefore);
    expect(paramsAfterLogical.get(4) as int).toEqual(blockLtBefore + 101);
    expect(paramsAfterLogical.get(5) as int).toEqual(logicalLtBefore + 202);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_vm_set_block_and_logical_time_are_independent_in_c7_slots_tests/vm_set_block_and_logical_time_are_independent_in_c7_slots.stdout.txt",
    );
}

#[test]
fn vm_set_logical_then_block_time_preserves_slot_isolation_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/dj_vm_set_logical_then_block_slot_isolation.test.tolk";
    let source = format!(
        r#"
{DJ_VM_IMPORTS}
get fun `test-dj-vm-set-logical-then-block-slot-isolation`() {{
    val c7Before = vm.getC7();
    val paramsBefore = c7Before.get(0) as tuple;
    val nowBefore = paramsBefore.get(3) as int;
    val blockLtBefore = paramsBefore.get(4) as int;
    val logicalLtBefore = paramsBefore.get(5) as int;

    vm.setLogicalTime(logicalLtBefore + 303);
    var c7AfterLogical = vm.getC7();
    var paramsAfterLogical = c7AfterLogical.get(0) as tuple;
    expect(paramsAfterLogical.get(3) as int).toEqual(nowBefore);
    expect(paramsAfterLogical.get(4) as int).toEqual(blockLtBefore);
    expect(paramsAfterLogical.get(5) as int).toEqual(logicalLtBefore + 303);

    vm.setBlockLogicalTime(blockLtBefore + 404);
    val c7AfterBlock = vm.getC7();
    val paramsAfterBlock = c7AfterBlock.get(0) as tuple;
    expect(paramsAfterBlock.get(3) as int).toEqual(nowBefore);
    expect(paramsAfterBlock.get(4) as int).toEqual(blockLtBefore + 404);
    expect(paramsAfterBlock.get(5) as int).toEqual(logicalLtBefore + 303);
}}
"#
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write dj fixture logical/block slot isolation test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_stdlib_vm_set_block_and_logical_time_are_independent_in_c7_slots_tests/vm_set_logical_then_block_time_preserves_slot_isolation_in_fixture_project.stdout.txt",
        );
}
