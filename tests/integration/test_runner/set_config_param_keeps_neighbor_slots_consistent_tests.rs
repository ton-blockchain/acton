use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const VM_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/vm/vm"
"#;

fn run_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_source = format!("{VM_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("cl_vm_helpers", &test_source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn set_config_param_keeps_neighbor_slots_consistent() {
    run_success_case(
        "cl-stdlib-set-config-param-neighbor-slots",
        r"
get fun `test cl stdlib set config param neighbor slots`() {
    val c7Before = vm.getC7();
    val paramsBefore = c7Before.get(0) as tuple;
    val beforeNow = paramsBefore.get(3) as int;

    vm.setConfigParam(401234567, 4);
    vm.setConfigParam(501234567, 5);

    val c7After = vm.getC7();
    val paramsAfter = c7After.get(0) as tuple;

    expect(paramsAfter.get(3) as int).toEqual(beforeNow);
    expect(paramsAfter.get(4) as int).toEqual(401234567);
    expect(paramsAfter.get(5) as int).toEqual(501234567);
}
",
        "integration/snapshots/test-runner/set_config_param_keeps_neighbor_slots_consistent/set_config_param_keeps_neighbor_slots_consistent.stdout.txt",
    );
}

#[test]
fn set_config_param_slot_three_updates_blockchain_now_and_c7() {
    run_success_case(
        "cl-stdlib-set-config-param-slot-three",
        r"
get fun `test cl stdlib set config param slot three`() {
    val c7Before = vm.getC7();
    val paramsBefore = c7Before.get(0) as tuple;
    val beforeBlockLogicalTime = paramsBefore.get(4) as int;
    val beforeLogicalTime = paramsBefore.get(5) as int;

    vm.setConfigParam(1700002222, 3);

    val c7After = vm.getC7();
    val paramsAfter = c7After.get(0) as tuple;

    expect(blockchain.now()).toEqual(1700002222);
    expect(paramsAfter.get(3) as int).toEqual(1700002222);
    expect(paramsAfter.get(4) as int).toEqual(beforeBlockLogicalTime);
    expect(paramsAfter.get(5) as int).toEqual(beforeLogicalTime);
}
",
        "integration/snapshots/test-runner/set_config_param_keeps_neighbor_slots_consistent/set_config_param_slot_three_updates_blockchain_now_and_c7.stdout.txt",
    );
}

#[test]
fn get_config_param_tuple_read_is_not_usable_bug() {
    run_success_case(
        "cl-stdlib-get-config-param-tuple-read-bug",
        r#"
get fun `test cl stdlib get config param tuple read bug`() {
    vm.setConfigParam(tuple [ton("9"), null], 7);

    val originalBalance = vm.getConfigParam<tuple>(7);
    expect(originalBalance.get(0) as int).toEqual(ton("9"));
}
"#,
        "integration/snapshots/test-runner/set_config_param_keeps_neighbor_slots_consistent/get_config_param_tuple_read_is_not_usable_bug.stdout.txt",
    );
}
