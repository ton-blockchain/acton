use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const VM_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/vm/vm"
"#;

fn run_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_source = format!("{VM_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("cm_vm_set_c7", &test_source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn vm_set_c7_overwrites_slot_three_without_clobbering_neighbors() {
    run_success_case(
        "cm-stdlib-vm-set-c7-overwrite-slot-three",
        r#"
get fun `test-cm-stdlib-vm-set-c7-overwrite-slot-three`() {
    val c7Before = vm.getC7();
    val paramsBefore = c7Before.get(0) as tuple;
    val beforeBlockLogicalTime = paramsBefore.get(4) as int;
    val beforeLogicalTime = paramsBefore.get(5) as int;

    var updatedC7 = vm.getC7();
    var updatedParams = updatedC7.get(0) as tuple;
    updatedParams.set(1700003000, 3);
    updatedC7.set(updatedParams, 0);
    vm.setC7(updatedC7);

    val c7After = vm.getC7();
    val paramsAfter = c7After.get(0) as tuple;

    expect(paramsAfter.get(3) as int).toEqual(1700003000);
    expect(paramsAfter.get(4) as int).toEqual(beforeBlockLogicalTime);
    expect(paramsAfter.get(5) as int).toEqual(beforeLogicalTime);
    expect(blockchain.now()).toEqual(1700003000);
}
"#,
        "integration/snapshots/test-runner/vm_set_c7_overwrites_slot_three_without_clobbering_neighbors/vm_set_c7_overwrites_slot_three_without_clobbering_neighbors.stdout.txt",
    );
}

#[test]
fn vm_set_c7_repeated_reads_and_writes_preserve_unmodified_slots() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/cm_vm_set_c7_repeated_reads_writes.test.tolk";
    let source = format!(
        r#"
{VM_IMPORTS}
get fun `test-cm-stdlib-vm-set-c7-repeated-reads-writes`() {{
    val c7Initial = vm.getC7();
    val paramsInitial = c7Initial.get(0) as tuple;
    val initialNow = paramsInitial.get(3) as int;
    val initialBlockLogicalTime = paramsInitial.get(4) as int;
    val initialLogicalTime = paramsInitial.get(5) as int;

    var c7RoundOne = vm.getC7();
    var paramsRoundOne = c7RoundOne.get(0) as tuple;
    paramsRoundOne.set(initialNow + 1111, 3);
    c7RoundOne.set(paramsRoundOne, 0);
    vm.setC7(c7RoundOne);

    var c7RoundTwo = vm.getC7();
    var paramsRoundTwo = c7RoundTwo.get(0) as tuple;
    paramsRoundTwo.set(initialBlockLogicalTime + 2222, 4);
    c7RoundTwo.set(paramsRoundTwo, 0);
    vm.setC7(c7RoundTwo);

    val c7Final = vm.getC7();
    val paramsFinal = c7Final.get(0) as tuple;

    expect(paramsFinal.get(3) as int).toEqual(initialNow + 1111);
    expect(paramsFinal.get(4) as int).toEqual(initialBlockLogicalTime + 2222);
    expect(paramsFinal.get(5) as int).toEqual(initialLogicalTime);
    expect(blockchain.now()).toEqual(initialNow + 1111);
}}
"#
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write cm repeated vm.setC7 fixture test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/vm_set_c7_overwrites_slot_three_without_clobbering_neighbors/vm_set_c7_repeated_reads_and_writes_preserve_unmodified_slots.stdout.txt",
        );
}
