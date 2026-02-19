use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CN_VM_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/types/out_actions"
import "../../lib/vm/vm"
"#;

fn run_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CN_VM_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("c5_roundtrip", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn set_c5_roundtrip_restores_single_and_double_action_out_lists() {
    run_success(
        "cn-stdlib-set-c5-roundtrip-non-empty-transitions",
        r#"
get fun `test-cn-set-c5-roundtrip-non-empty-transitions`() {
    val dest = net.randomAddress("cn_set_c5_roundtrip_dest");
    createMessage({
        bounce: false,
        value: ton("1"),
        dest,
        body: beginCell().storeUint(0xC0FFEE01, 32).endCell().beginParse(),
    }).send(SEND_MODE_REGULAR);
    val singleActionC5 = vm.getC5();

    createMessage({
        bounce: false,
        value: ton("2"),
        dest,
        body: beginCell().storeUint(0xC0FFEE02, 32).endCell().beginParse(),
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);
    val doubleActionC5 = vm.getC5();

    val doubleActions = vm.outActions();
    expect(doubleActions.size()).toEqual(2);
    expect(doubleActions.at(0).kind()).toEqual("send-message");
    expect(doubleActions.at(1).kind()).toEqual("send-message");
    val firstDouble = doubleActions.getSendMessageAt(0);
    val secondDouble = doubleActions.getSendMessageAt(1);
    expect(firstDouble).toBeNotNull();
    expect(secondDouble).toBeNotNull();
    expect(firstDouble!.mode).toEqual(SEND_MODE_PAY_FEES_SEPARATELY);
    expect(secondDouble!.mode).toEqual(SEND_MODE_REGULAR);

    vm.setC5(singleActionC5);
    val singleActions = vm.outActions();
    expect(singleActions.size()).toEqual(1);
    expect(singleActions.at(0).kind()).toEqual("send-message");
    val single = singleActions.getSendMessageAt(0);
    expect(single).toBeNotNull();
    expect(single!.mode).toEqual(SEND_MODE_REGULAR);

    vm.setC5(doubleActionC5);
    val restoredDoubleActions = vm.outActions();
    expect(restoredDoubleActions.size()).toEqual(2);
    val restoredFirst = restoredDoubleActions.getSendMessageAt(0);
    val restoredSecond = restoredDoubleActions.getSendMessageAt(1);
    expect(restoredFirst).toBeNotNull();
    expect(restoredSecond).toBeNotNull();
    expect(restoredFirst!.mode).toEqual(SEND_MODE_PAY_FEES_SEPARATELY);
    expect(restoredSecond!.mode).toEqual(SEND_MODE_REGULAR);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_set_c5_roundtrip_restores_single_and_double_action_out_lists_tests/set_c5_roundtrip_restores_single_and_double_action_out_lists.stdout.txt",
    );
}

#[test]
fn set_c5_to_empty_cell_breaks_vm_out_actions_bug() {
    run_success(
        "cn-stdlib-set-c5-empty-cell-out-actions-bug",
        r#"
get fun `test-cn-set-c5-empty-cell-out-actions-bug`() {
    val emptyC5 = vm.getC5();
    vm.setC5(emptyC5);

    val parsed = vm.outActions();
    expect(parsed.size()).toEqual(0);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_set_c5_roundtrip_restores_single_and_double_action_out_lists_tests/set_c5_to_empty_cell_breaks_vm_out_actions_bug.stdout.txt",
    );
}

#[test]
fn parse_out_actions_direct_empty_cell_returns_empty_list() {
    run_success(
        "cn-stdlib-parse-out-actions-direct-empty-cell",
        r#"
get fun `test-cn-parse-out-actions-direct-empty-cell`() {
    val parsed = vm.parseOutActions(createEmptyCell());
    expect(parsed.size()).toEqual(0);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_set_c5_roundtrip_restores_single_and_double_action_out_lists_tests/parse_out_actions_direct_empty_cell_returns_empty_list.stdout.txt",
    );
}
