use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CJ_OUT_ACTIONS_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/impl"
import "../../lib/testing/expect"
import "../../lib/types/out_actions"

fun changeLib(code: cell, mode: int): void asm "SETLIBCODE"
"#;

fn run_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CJ_OUT_ACTIONS_IMPORTS}\n{test_body}\n");
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
fn parse_out_actions_preserves_reserve_and_change_library_kinds() {
    run_success(
        "cj-stdlib-parse-out-actions-preserves-kinds",
        r#"
get fun `test cj parse out actions preserves kinds`() {
    reserveGramsOnBalance(
        grams("0.05"),
        RESERVE_MODE_ALL_BUT_AMOUNT | RESERVE_MODE_BOUNCE_ON_ACTION_FAIL
    );
    changeLib(beginCell().storeUint(0xAB, 8).endCell(), 2);

    val viaVm = testing.outActions();
    val viaRaw = impl.parseOutActions(impl.getC5());

    expect(viaVm.size()).toEqual(2);
    expect(viaRaw.size()).toEqual(2);

    expect(viaVm.at(0).kind()).toEqual("change-library");
    expect(viaVm.at(1).kind()).toEqual("reserve-currency");
    expect(viaRaw.at(0).kind()).toEqual("change-library");
    expect(viaRaw.at(1).kind()).toEqual("reserve-currency");
}
"#,
        "integration/snapshots/test-runner/parse_out_actions_preserves_reserve_and_change_library_kinds/parse_out_actions_preserves_reserve_and_change_library_kinds.stdout.txt",
    );
}

#[test]
fn parse_out_actions_reserve_nanogram_is_misparsed_as_change_library_bug() {
    run_success(
        "cj-stdlib-parse-out-actions-reserve-one-nanogram-kind-bug",
        r#"
get fun `test cj parse out actions reserve one nanogram kind bug`() {
    reserveGramsOnBalance(1, RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);

    val parsed = testing.outActions();
    expect(parsed.size()).toEqual(1);

    expect(parsed.at(0).kind()).toEqual("reserve-currency");
}
"#,
        "integration/snapshots/test-runner/parse_out_actions_preserves_reserve_and_change_library_kinds/parse_out_actions_reserve_nanogram_is_misparsed_as_change_library_bug.stdout.txt",
    );
}

#[test]
fn parse_out_actions_reserve_zero_nanogram_stays_reserve_currency() {
    run_success(
        "cj-stdlib-parse-out-actions-reserve-zero-nanogram-kind",
        r#"
get fun `test cj parse out actions reserve zero nanogram kind`() {
    reserveGramsOnBalance(0, RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);

    val parsed = testing.outActions();
    expect(parsed.size()).toEqual(1);
    expect(parsed.at(0).kind()).toEqual("reserve-currency");
}
"#,
        "integration/snapshots/test-runner/parse_out_actions_preserves_reserve_and_change_library_kinds/parse_out_actions_reserve_zero_nanogram_stays_reserve_currency.stdout.txt",
    );
}

#[test]
fn parse_out_actions_single_set_code_action_is_decoded() {
    run_success(
        "cj-stdlib-parse-out-actions-single-set-code",
        r#"
get fun `test cj parse out actions single set code`() {
    contract.setCodePostponed(beginCell().storeUint(0xA1, 8).endCell());

    val parsed = testing.outActions();
    expect(parsed.size()).toEqual(1);
    expect(parsed.at(0).kind()).toEqual("set-code");
}
"#,
        "integration/snapshots/test-runner/parse_out_actions_preserves_reserve_and_change_library_kinds/parse_out_actions_single_set_code_action_is_decoded.stdout.txt",
    );
}

#[test]
fn parse_out_actions_single_send_message_action_preserves_mode() {
    run_success(
        "cj-stdlib-parse-out-actions-single-send-message",
        r#"
get fun `test cj parse out actions single send message`() {
    val dest = randomAddress("cj_single_send_dest");
    createMessage({
        bounce: false,
        value: grams("0.2"),
        dest,
        body: beginCell().storeUint(0xC7000011, 32).endCell().beginParse(),
    }).send(SEND_MODE_PAY_FEES_SEPARATELY);

    val parsed = testing.outActions();
    expect(parsed.size()).toEqual(1);
    expect(parsed.at(0).kind()).toEqual("send-message");
    val send = parsed.getSendMessageAt(0);
    expect(send).toBeNotNull();
    expect(send!.mode).toEqual(SEND_MODE_PAY_FEES_SEPARATELY);
}
"#,
        "integration/snapshots/test-runner/parse_out_actions_preserves_reserve_and_change_library_kinds/parse_out_actions_single_send_message_action_preserves_mode.stdout.txt",
    );
}

#[test]
fn parse_out_actions_reserve_zero_and_one_preserve_kind_and_amount() {
    run_success(
        "cj-stdlib-parse-out-actions-reserve-zero-and-one",
        r#"
get fun `test cj parse out actions reserve zero and one`() {
    reserveGramsOnBalance(0, RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);
    reserveGramsOnBalance(1, RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);

    val parsed = testing.outActions();
    expect(parsed.size()).toEqual(2);
    expect(parsed.at(0).kind()).toEqual("reserve-currency");
    expect(parsed.at(1).kind()).toEqual("reserve-currency");

    val first = parsed.at(0);
    val second = parsed.at(1);
    expect(first is TlbOutActionReserveCurrency).toBeTrue();
    expect(second is TlbOutActionReserveCurrency).toBeTrue();
    if (first is TlbOutActionReserveCurrency) {
        expect(first.currency.grams).toEqual(1);
    }
    if (second is TlbOutActionReserveCurrency) {
        expect(second.currency.grams).toEqual(0);
    }
}
"#,
        "integration/snapshots/test-runner/parse_out_actions_preserves_reserve_and_change_library_kinds/parse_out_actions_reserve_zero_and_one_preserve_kind_and_amount.stdout.txt",
    );
}

#[test]
fn parse_out_actions_mixed_action_chain_preserves_order_and_types() {
    run_success(
        "cj-stdlib-parse-out-actions-mixed-chain",
        r#"
get fun `test cj parse out actions mixed chain`() {
    val dest = randomAddress("cj_mixed_chain_dest");

    reserveGramsOnBalance(1, RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);
    contract.setCodePostponed(beginCell().storeUint(0xB2, 8).endCell());
    createMessage({
        bounce: false,
        value: grams("0.3"),
        dest,
        body: beginCell().storeUint(0xC7000022, 32).endCell().beginParse(),
    }).send(SEND_MODE_REGULAR);
    changeLib(beginCell().storeUint(0xCD, 8).endCell(), 2);

    val parsed = testing.outActions();
    expect(parsed.size()).toEqual(4);
    expect(parsed.at(0).kind()).toEqual("change-library");
    expect(parsed.at(1).kind()).toEqual("send-message");
    expect(parsed.at(2).kind()).toEqual("set-code");
    expect(parsed.at(3).kind()).toEqual("reserve-currency");

    val send = parsed.getSendMessageAt(1);
    expect(send).toBeNotNull();
    expect(send!.mode).toEqual(SEND_MODE_REGULAR);

    val reserve = parsed.at(3);
    expect(reserve is TlbOutActionReserveCurrency).toBeTrue();
    if (reserve is TlbOutActionReserveCurrency) {
        expect(reserve.currency.grams).toEqual(1);
    }
}
"#,
        "integration/snapshots/test-runner/parse_out_actions_preserves_reserve_and_change_library_kinds/parse_out_actions_mixed_action_chain_preserves_order_and_types.stdout.txt",
    );
}
