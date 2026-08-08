use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const OUT_ACTIONS_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../../lib/types/message"
import "../../lib/types/out_actions"
import "../../lib/ffi"
import "../../lib/impl"

struct (0xA1100001) InlinePayload {
    queryId: uint64
    amount: uint32
}

struct (0xA1100002) RefPayload {
    queryId: uint64
    part1: uint256
    part2: uint256
    part3: uint256
}

fun changeLib(code: cell, mode: int): void asm "SETLIBCODE"
"#;

fn run_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{OUT_ACTIONS_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("out_actions", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn out_actions_list_traversal_preserves_reverse_order_and_kinds() {
    run_success(
        "al-stdlib-out-actions-list-traversal-order",
        r#"
get fun `test al out actions list traversal order`() {
    val dest = randomAddress("al_out_actions_order");
    val msg = createMessage({
        bounce: false,
        value: grams("1"),
        dest,
        body: InlinePayload {
            queryId: 1,
            amount: 10,
        },
    });

    val newCode = beginCell().storeUint(0xAB, 8).endCell();
    contract.setCodePostponed(newCode);
    msg.send(SEND_MODE_REGULAR | SEND_MODE_BOUNCE_ON_ACTION_FAIL);
    reserveGramsOnBalance(
        grams("0.05"),
        RESERVE_MODE_ALL_BUT_AMOUNT | RESERVE_MODE_BOUNCE_ON_ACTION_FAIL
    );
    changeLib(createEmptyCell(), 2);

    val outActions = testing.outActions();
    expect(outActions.size()).toEqual(4);

    expect(outActions.at(0).kind()).toEqual("change-library");
    expect(outActions.at(1).kind()).toEqual("reserve-currency");
    expect(outActions.at(2).kind()).toEqual("send-message");
    expect(outActions.at(3).kind()).toEqual("set-code");
}
"#,
        "integration/snapshots/test-runner/out_actions_list_traversal_preserves_reverse_order_and_kinds/out_actions_list_traversal_preserves_reverse_order_and_kinds.stdout.txt",
    );
}

#[test]
fn parse_out_actions_from_raw_c5_matches_vm_out_actions() {
    run_success(
        "al-stdlib-parse-out-actions-from-c5",
        r#"
get fun `test al parse out actions from c5`() {
    val dest = randomAddress("al_out_actions_parse_c5");

    createMessage({
        bounce: false,
        value: grams("1"),
        dest,
        body: InlinePayload {
            queryId: 2,
            amount: 20,
        },
    }).send(SEND_MODE_REGULAR);

    createMessage({
        bounce: false,
        value: grams("1"),
        dest,
        body: InlinePayload {
            queryId: 3,
            amount: 30,
        },
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);

    val viaVm = testing.outActions();
    val viaRaw = impl.parseOutActions(impl.getC5());

    expect(viaVm.size()).toEqual(2);
    expect(viaRaw.size()).toEqual(2);

    expect(viaRaw.at(0).kind()).toEqual(viaVm.at(0).kind());
    expect(viaRaw.at(1).kind()).toEqual(viaVm.at(1).kind());

    val first = viaRaw.getSendMessageAt(0);
    val second = viaRaw.getSendMessageAt(1);
    expect(first).toBeNotNull();
    expect(second).toBeNotNull();
    expect(first!.mode).toEqual(SEND_MODE_PAY_FEES_SEPARATELY);
    expect(second!.mode).toEqual(SEND_MODE_REGULAR);
}
"#,
        "integration/snapshots/test-runner/out_actions_list_traversal_preserves_reverse_order_and_kinds/parse_out_actions_from_raw_c5_matches_vm_out_actions.stdout.txt",
    );
}

#[test]
fn get_send_message_at_returns_null_for_non_send_entries() {
    run_success(
        "al-stdlib-get-send-message-at-null-non-send",
        r#"
get fun `test al get send message at null non send`() {
    val dest = randomAddress("al_get_send_message_at");

    contract.setCodePostponed(beginCell().storeUint(1, 1).endCell());
    createMessage({
        bounce: false,
        value: grams("1"),
        dest,
        body: InlinePayload {
            queryId: 4,
            amount: 40,
        },
    }).send(SEND_MODE_REGULAR);

    val outActions = testing.outActions();
    expect(outActions.size()).toEqual(2);

    val sendAction = outActions.getSendMessageAt(0);
    val nonSendAction = outActions.getSendMessageAt(1);

    expect(sendAction).toBeNotNull();
    expect(sendAction!.mode).toEqual(SEND_MODE_REGULAR);
    expect(nonSendAction == null).toBeTrue();
    expect(outActions.at(1).kind()).toEqual("set-code");
}
"#,
        "integration/snapshots/test-runner/out_actions_list_traversal_preserves_reverse_order_and_kinds/get_send_message_at_returns_null_for_non_send_entries.stdout.txt",
    );
}

#[test]
fn get_send_message_body_at_reads_inline_body_left_branch() {
    run_success(
        "al-stdlib-get-send-message-body-inline-left",
        r#"
get fun `test al get send message body inline left`() {
    val dest = randomAddress("al_get_body_left");

    createMessage({
        bounce: false,
        value: grams("1"),
        dest,
        body: InlinePayload {
            queryId: 7,
            amount: 77,
        },
    }).send(SEND_MODE_REGULAR);

    val outActions = testing.outActions();
    val action = outActions.getSendMessageAt(0);
    expect(action).toBeNotNull();

    var encodedBody = action!.loadGenericMessage().body;
    expect(encodedBody.loadBool()).toBeFalse();

    val body = outActions.getSendMessageBodyAt<InlinePayload>(0);
    expect(body).toBeNotNull();
    expect(body!.queryId).toEqual(7);
    expect(body!.amount).toEqual(77);
}
"#,
        "integration/snapshots/test-runner/out_actions_list_traversal_preserves_reverse_order_and_kinds/get_send_message_body_at_reads_inline_body_left_branch.stdout.txt",
    );
}

#[test]
fn get_send_message_body_at_reads_ref_body_right_branch() {
    run_success(
        "al-stdlib-get-send-message-body-ref-right",
        r#"
get fun `test al get send message body ref right`() {
    val dest = randomAddress("al_get_body_right");

    createMessage({
        bounce: false,
        value: grams("1"),
        dest,
        body: RefPayload {
            queryId: 9,
            part1: 0x11,
            part2: 0x22,
            part3: 0x33,
        },
    }).send(SEND_MODE_REGULAR);

    val outActions = testing.outActions();
    val action = outActions.getSendMessageAt(0);
    expect(action).toBeNotNull();

    var encodedBody = action!.loadGenericMessage().body;
    expect(encodedBody.loadBool()).toBeTrue();

    val body = outActions.getSendMessageBodyAt<RefPayload>(0);
    expect(body).toBeNotNull();
    expect(body!.queryId).toEqual(9);
    expect(body!.part1).toEqual(0x11);
    expect(body!.part2).toEqual(0x22);
    expect(body!.part3).toEqual(0x33);
}
"#,
        "integration/snapshots/test-runner/out_actions_list_traversal_preserves_reverse_order_and_kinds/get_send_message_body_at_reads_ref_body_right_branch.stdout.txt",
    );
}

#[test]
fn get_send_message_body_at_returns_null_for_non_send_action() {
    run_success(
        "al-stdlib-get-send-message-body-null-non-send",
        r#"
get fun `test al get send message body null non send`() {
    contract.setCodePostponed(beginCell().storeUint(0xCC, 8).endCell());

    val outActions = testing.outActions();
    expect(outActions.size()).toEqual(1);
    expect(outActions.at(0).kind()).toEqual("set-code");

    val body = outActions.getSendMessageBodyAt<InlinePayload>(0);
    expect(body == null).toBeTrue();
}
"#,
        "integration/snapshots/test-runner/out_actions_list_traversal_preserves_reverse_order_and_kinds/get_send_message_body_at_returns_null_for_non_send_action.stdout.txt",
    );
}

#[test]
fn out_message_out_actions_helper_returns_send_action_with_mode_and_body() {
    run_success(
        "al-stdlib-out-message-out-actions-helper",
        r#"
get fun `test al out message out actions helper`() {
    val dest = randomAddress("al_out_message_helper");
    val msg = createMessage({
        bounce: false,
        value: grams("1"),
        dest,
        body: InlinePayload {
            queryId: 55,
            amount: 505,
        },
    });

    val outActions = msg.outActions(SEND_MODE_PAY_FEES_SEPARATELY);
    expect(outActions.size()).toEqual(1);

    val action = outActions.getSendMessageAt(0);
    expect(action).toBeNotNull();
    expect(action!.mode).toEqual(SEND_MODE_PAY_FEES_SEPARATELY);

    val body = outActions.getSendMessageBodyAt<InlinePayload>(0);
    expect(body).toBeNotNull();
    expect(body!.queryId).toEqual(55);
    expect(body!.amount).toEqual(505);
}
"#,
        "integration/snapshots/test-runner/out_actions_list_traversal_preserves_reverse_order_and_kinds/out_message_out_actions_helper_returns_send_action_with_mode_and_body.stdout.txt",
    );
}

#[test]
fn get_send_message_helpers_return_null_for_reserve_and_change_library_actions() {
    run_success(
        "al-stdlib-get-send-message-helpers-null-for-reserve-and-change-library",
        r#"
get fun `test al get send message helpers null for reserve and change library`() {
    reserveGramsOnBalance(1, RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);
    changeLib(beginCell().storeUint(0xEE, 8).endCell(), 2);

    val outActions = testing.outActions();
    expect(outActions.size()).toEqual(2);
    expect(outActions.at(0).kind()).toEqual("change-library");
    expect(outActions.at(1).kind()).toEqual("reserve-currency");

    val sendAtChange = outActions.getSendMessageAt(0);
    val sendAtReserve = outActions.getSendMessageAt(1);
    expect(sendAtChange == null).toBeTrue();
    expect(sendAtReserve == null).toBeTrue();

    val bodyAtChange = outActions.getSendMessageBodyAt<InlinePayload>(0);
    val bodyAtReserve = outActions.getSendMessageBodyAt<InlinePayload>(1);
    expect(bodyAtChange == null).toBeTrue();
    expect(bodyAtReserve == null).toBeTrue();
}
"#,
        "integration/snapshots/test-runner/out_actions_list_traversal_preserves_reverse_order_and_kinds/get_send_message_helpers_return_null_for_reserve_and_change_library_actions.stdout.txt",
    );
}
