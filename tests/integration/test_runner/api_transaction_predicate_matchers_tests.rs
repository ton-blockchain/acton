use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SNAPSHOT_DIR: &str = "integration/snapshots/test-runner/api_transaction_predicate_matchers";

const PREDICATE_MESSAGES: &str = r"
struct (0xAE110001) Ping {
    queryId: uint64
}

struct (0xAE110002) BounceNotice {
    queryId: uint64
}
";

const PREDICATE_CONTRACT: &str = r#"
import "@stdlib/gas-payments"
import "messages"

const ERR_FAIL = 701;

fun onInternalMessage(in: InMessage) {
    if (in.body.isEmpty()) {
        return;
    }

    val msg = lazy Ping.fromSlice(in.body);

    if (msg.queryId == 10) {
        throw ERR_FAIL;
    }

    if (msg.queryId == 20) {
        reserveToncoinsOnBalance(ton("100"), RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);
        return;
    }

    if (msg.queryId == 40) {
        createMessage({
            bounce: false,
            value: ton("0.1"),
            dest: in.senderAddress,
            body: BounceNotice { queryId: msg.queryId },
        }).send(SEND_MODE_PAY_FEES_SEPARATELY);
        return;
    }
}

fun onBouncedMessage(_: InMessageBounced) {
    throw 777;
}
"#;

const TEST_PRELUDE: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/types/message"
import "../contracts/messages"

const ERR_FAIL = 701;
const ERR_BOUNCE = 777;
const ACTION_FAIL_EXIT = 37;

struct Harness {
    address: address
    init: ContractState
}

fun Harness.create() {
    val init = ContractState {
        code: build("harness"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return Harness { address, init };
}

fun deployHarness() {
    val sender = net.treasury("sender");
    val harness = Harness.create();

    val deployMsg = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit: harness.init,
        },
    });
    val deployRes = net.send(sender.address, deployMsg);
    expect(deployRes).toHaveSuccessfulDeploy({ to: harness.address });

    return (sender, harness, deployRes);
}

fun sendPingWithBounce(sender: Treasury, harness: Harness, queryId: uint64, bounceable: bool) {
    val msg = createMessage({
        bounce: bounceable,
        value: ton("0.5"),
        dest: harness.address,
        body: Ping { queryId },
    });
    return net.send(sender.address, msg);
}

fun sendPing(sender: Treasury, harness: Harness, queryId: uint64) {
    return sendPingWithBounce(sender, harness, queryId, false);
}

fun sendBouncedNotice(sender: Treasury, harness: Harness) {
    val trigger = createMessage({
        bounce: false,
        value: ton("0.5"),
        dest: harness.address,
        body: Ping { queryId: 40 },
    });
    val sendSingleRes = net.sendSingle(sender.address, trigger);
    expect(sendSingleRes.outMessages.size()).toEqual(1);

    val noticeBody = sendSingleRes.outMessages.at<BounceNotice>(0).loadBody().toCell();
    val bouncedBody = beginCell()
        .storeUint(0xFFFFFFFF, 32)
        .storeSlice(noticeBody.beginParse())
        .endCell();

    val bouncedMsg = createMessage({
        bounce: false,
        value: ton("0.3"),
        dest: harness.address,
        body: bouncedBody,
    }).bounced();

    return net.send(sender.address, bouncedMsg);
}
"#;

fn with_prelude(test_body: &str) -> String {
    format!("{TEST_PRELUDE}\n{test_body}\n")
}

fn run_success_case(project_name: &str, test_body: &str, snapshot_name: &str) {
    let source = with_prelude(test_body);
    ProjectBuilder::new(project_name)
        .file("contracts/messages", PREDICATE_MESSAGES)
        .contract("harness", PREDICATE_CONTRACT)
        .test_file("predicate_matchers", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(&format!("{SNAPSHOT_DIR}/{snapshot_name}.stdout.txt"));
}

fn run_failure_case(project_name: &str, test_body: &str, snapshot_name: &str) {
    let source = with_prelude(test_body);
    ProjectBuilder::new(project_name)
        .file("contracts/messages", PREDICATE_MESSAGES)
        .contract("harness", PREDICATE_CONTRACT)
        .test_file("predicate_matchers", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(&format!("{SNAPSHOT_DIR}/{snapshot_name}.stdout.txt"));
}

#[test]
fn predicate_matchers_cover_regular_transaction_fields() {
    run_success_case(
        "ae-predicate-regular-transaction-fields",
        r#"
get fun `test predicate regular transaction fields`() {
    val (sender, harness, _) = deployHarness();
    val expectedBody = Ping { queryId: 1 }.toCell();
    val res = sendPingWithBounce(sender, harness, 1, true);

    expect(res).toHaveTx({
        from: fun(addr: address): bool {
            println("from={}", addr);
            return addr == sender.address;
        },
        to: fun(addr: address): bool {
            println("to={}", addr);
            return addr == harness.address;
        },
        value: fun(value: coins): bool {
            println("value={}", value);
            return value == ton("0.5");
        },
        exitCode: fun(code: int32): bool {
            println("exitCode={}", code);
            return code == 0;
        },
        success: fun(ok: bool): bool {
            println("success={}", ok);
            return ok;
        },
        aborted: fun(flag: bool): bool {
            println("aborted={}", flag);
            return !flag;
        },
        deploy: fun(flag: bool): bool {
            println("deploy={}", flag);
            return !flag;
        },
        bounce: fun(flag: bool): bool {
            println("bounce={}", flag);
            return flag;
        },
        bounced: fun(flag: bool): bool {
            println("bounced={}", flag);
            return !flag;
        },
        opcode: fun(op: uint32): bool {
            println("opcode=0x{:x}", op);
            return op == Ping.getDeclaredPackPrefix2();
        },
        computePhaseSkipped: fun(flag: bool): bool {
            println("computePhaseSkipped={}", flag);
            return !flag;
        },
        body: fun(body: cell): bool {
            println("bodyHash=0x{:x}", body.hash());
            return body.hash() == expectedBody.hash();
        },
    });
}
"#,
        "predicate_matchers_cover_regular_transaction_fields",
    );
}

#[test]
fn predicate_matchers_cover_deploy_true_and_direct_find_transaction() {
    run_success_case(
        "ae-predicate-deploy-and-find-transaction",
        r#"
get fun `test predicate deploy and direct find transaction`() {
    val (_sender, harness, deployRes) = deployHarness();

    val found = deployRes.findTransaction({
        to: fun(addr: address): bool {
            println("deploy.to={}", addr);
            return addr == harness.address;
        },
        deploy: fun(flag: bool): bool {
            println("deploy.deploy={}", flag);
            return flag;
        },
        success: fun(flag: bool): bool {
            println("deploy.success={}", flag);
            return flag;
        },
    });
    expect(found).toBeDefined();

    expect(deployRes).toNotHaveTx({
        deploy: fun(flag: bool): bool {
            println("deploy.negated={}", flag);
            return !flag;
        },
    });
}
"#,
        "predicate_matchers_cover_deploy_true_and_direct_find_transaction",
    );
}

#[test]
fn predicate_matchers_cover_failed_transaction_fields() {
    run_success_case(
        "ae-predicate-failed-transaction-fields",
        r#"
get fun `test predicate failed transaction fields`() {
    val (sender, harness, _) = deployHarness();
    val expectedBody = Ping { queryId: 10 }.toCell();
    val res = sendPing(sender, harness, 10);

    expect(res).toHaveTx({
        from: fun(addr: address): bool {
            println("failed.from={}", addr);
            return addr == sender.address;
        },
        to: fun(addr: address): bool {
            println("failed.to={}", addr);
            return addr == harness.address;
        },
        exitCode: fun(code: int32): bool {
            println("failed.exitCode={}", code);
            return code == ERR_FAIL;
        },
        success: fun(ok: bool): bool {
            println("failed.success={}", ok);
            return !ok;
        },
        aborted: fun(flag: bool): bool {
            println("failed.aborted={}", flag);
            return flag;
        },
        opcode: fun(op: uint32): bool {
            println("failed.opcode=0x{:x}", op);
            return op == Ping.getDeclaredPackPrefix2();
        },
        body: fun(body: cell): bool {
            println("failed.bodyHash=0x{:x}", body.hash());
            return body.hash() == expectedBody.hash();
        },
    });
}
"#,
        "predicate_matchers_cover_failed_transaction_fields",
    );
}

#[test]
fn predicate_matchers_cover_action_exit_code_field() {
    run_success_case(
        "ae-predicate-action-exit-code-field",
        r#"
get fun `test predicate action exit code field`() {
    val (sender, harness, _) = deployHarness();
    val res = sendPing(sender, harness, 20);

    expect(res).toHaveTx({
        from: fun(addr: address): bool {
            println("action.from={}", addr);
            return addr == sender.address;
        },
        to: fun(addr: address): bool {
            println("action.to={}", addr);
            return addr == harness.address;
        },
        exitCode: fun(code: int32): bool {
            println("action.exitCode={}", code);
            return code == 0;
        },
        actionExitCode: fun(code: int32): bool {
            println("action.actionExitCode={}", code);
            return code == ACTION_FAIL_EXIT;
        },
        success: fun(ok: bool): bool {
            println("action.success={}", ok);
            return !ok;
        },
        aborted: fun(flag: bool): bool {
            println("action.aborted={}", flag);
            return flag;
        },
    });
}
"#,
        "predicate_matchers_cover_action_exit_code_field",
    );
}

#[test]
fn predicate_matchers_cover_bounced_transaction_fields() {
    run_success_case(
        "ae-predicate-bounced-transaction-fields",
        r#"
get fun `test predicate bounced transaction fields`() {
    val (sender, harness, _) = deployHarness();
    val res = sendBouncedNotice(sender, harness);

    expect(res).toHaveTx({
        from: fun(addr: address): bool {
            println("bounced.from={}", addr);
            return addr == sender.address;
        },
        to: fun(addr: address): bool {
            println("bounced.to={}", addr);
            return addr == harness.address;
        },
        bounced: fun(flag: bool): bool {
            println("bounced.flag={}", flag);
            return flag;
        },
        exitCode: fun(code: int32): bool {
            println("bounced.exitCode={}", code);
            return code == ERR_BOUNCE;
        },
        opcode: fun(op: uint32): bool {
            println("bounced.opcode=0x{:x}", op);
            return op == BounceNotice.getDeclaredPackPrefix2();
        },
    });
}
"#,
        "predicate_matchers_cover_bounced_transaction_fields",
    );
}

#[test]
fn predicate_matchers_cover_compute_phase_skipped_true() {
    run_success_case(
        "ae-predicate-compute-phase-skipped-true",
        r#"
get fun `test predicate compute phase skipped true`() {
    val sender = net.treasury("sender");
    val missingAddress = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");
    val expectedBody = beginCell().storeUint(0x20, 32).endCell();

    val res = net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: missingAddress,
        body: expectedBody,
    }));

    expect(res).toHaveTx({
        from: fun(addr: address): bool {
            println("skipped.from={}", addr);
            return addr == sender.address;
        },
        to: fun(addr: address): bool {
            println("skipped.to={}", addr);
            return addr == missingAddress;
        },
        bounce: fun(flag: bool): bool {
            println("skipped.bounce={}", flag);
            return !flag;
        },
        computePhaseSkipped: fun(flag: bool): bool {
            println("skipped.computePhaseSkipped={}", flag);
            return flag;
        },
        body: fun(body: cell): bool {
            println("skipped.bodyHash=0x{:x}", body.hash());
            return body.hash() == expectedBody.hash();
        },
    });
}
"#,
        "predicate_matchers_cover_compute_phase_skipped_true",
    );
}

#[test]
fn predicate_matchers_treat_compute_skipped_as_unsuccessful_and_exit_code_less() {
    run_success_case(
        "ae-predicate-compute-skipped-success-semantics",
        r#"
get fun `test predicate compute skipped success semantics`() {
    val sender = net.treasury("sender");
    val missingAddress = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");

    val res = net.send(sender.address, createMessage({
        bounce: false,
        value: ton("1"),
        dest: missingAddress,
        body: beginCell().storeUint(0x21, 32).endCell(),
    }));

    expect(res).toHaveTx({
        success: fun(ok: bool): bool {
            println("skipped.success.false={}", ok);
            return !ok;
        },
        computePhaseSkipped: fun(flag: bool): bool {
            println("skipped.success.computePhaseSkipped={}", flag);
            return flag;
        },
    });
    expect(res).toNotHaveTx({
        success: fun(ok: bool): bool {
            println("skipped.success.true={}", ok);
            return ok;
        },
    });
    expect(res).toNotHaveTx({
        exitCode: fun(code: int32): bool {
            println("skipped.exitCode={}", code);
            return code == 0;
        },
    });

    val impossible = res.findTransaction({
        success: fun(ok: bool): bool {
            println("skipped.find.success.true={}", ok);
            return ok;
        },
    });
    expect(impossible).toBeNone();
}
"#,
        "predicate_matchers_treat_compute_skipped_as_unsuccessful_and_exit_code_less",
    );
}

#[test]
fn predicate_matchers_support_mixed_scalar_and_predicate_fields() {
    run_success_case(
        "ae-predicate-mixed-scalar-and-predicate-fields",
        r#"
get fun `test predicate mixed scalar and predicate fields`() {
    val (sender, harness, _) = deployHarness();
    val expectedBody = Ping { queryId: 1 }.toCell();
    val res = sendPingWithBounce(sender, harness, 1, true);

    expect(res).toHaveTx({
        from: fun(addr: address): bool {
            println("mixed.from={}", addr);
            return addr == sender.address;
        },
        to: harness.address,
        value: ton("0.5"),
        bounce: true,
        body: expectedBody,
    });

    val found = res.findTransaction({
        to: harness.address,
        opcode: Ping.getDeclaredPackPrefix2(),
        success: fun(ok: bool): bool {
            println("mixed.success={}", ok);
            return ok;
        },
    });
    expect(found).toBeDefined();

    expect(res).toNotHaveTx({
        from: fun(addr: address): bool {
            println("mixed.negated.from={}", addr);
            return false;
        },
        to: harness.address,
        value: ton("0.5"),
    });
}
"#,
        "predicate_matchers_support_mixed_scalar_and_predicate_fields",
    );
}

#[test]
fn predicate_bounced_opcode_requires_explicit_bounced_matcher() {
    run_success_case(
        "ae-predicate-bounced-opcode-requires-flag",
        r#"
get fun `test predicate bounced opcode requires explicit flag`() {
    val (sender, harness, _) = deployHarness();
    val res = sendBouncedNotice(sender, harness);

    expect(res).toNotHaveTx({
        opcode: fun(op: uint32): bool {
            println("bounced.missing-flag.opcode=0x{:x}", op);
            return op == BounceNotice.getDeclaredPackPrefix2();
        },
    });

    val missing = res.findTransaction({
        opcode: fun(op: uint32): bool {
            println("bounced.missing-flag.find.opcode=0x{:x}", op);
            return op == BounceNotice.getDeclaredPackPrefix2();
        },
    });
    expect(missing).toBeNone();
}
"#,
        "predicate_bounced_opcode_requires_explicit_bounced_matcher",
    );
}

#[test]
fn predicate_failure_diagnostics_show_function_markers() {
    run_failure_case(
        "ae-predicate-function-marker-diagnostics",
        r#"
get fun `test predicate diagnostics show function markers`() {
    val (sender, harness, _) = deployHarness();
    val res = sendPing(sender, harness, 1);

    expect(res).toHaveTx({
        from: fun(addr: address): bool {
            println("diag.from={}", addr);
            return false;
        },
        to: fun(addr: address): bool {
            println("diag.to={}", addr);
            return false;
        },
        value: fun(value: coins): bool {
            println("diag.value={}", value);
            return false;
        },
        body: fun(body: cell): bool {
            println("diag.bodyHash=0x{:x}", body.hash());
            return false;
        },
    });
}
"#,
        "predicate_failure_diagnostics_show_function_markers",
    );
}

#[test]
fn predicate_runtime_error_surfaces_to_user() {
    run_failure_case(
        "ae-predicate-runtime-error",
        r#"
get fun `test predicate runtime error`() {
    val (sender, harness, _) = deployHarness();
    val res = sendPing(sender, harness, 1);

    expect(res).toHaveTx({
        to: fun(addr: address): bool {
            println("throw.to={}", addr);
            build("missing_contract");
            return true;
        },
    });
}
"#,
        "predicate_runtime_error_surfaces_to_user",
    );
}

#[test]
fn predicate_vm_exit_code_surfaces_to_user() {
    run_failure_case(
        "ae-predicate-vm-exit-code",
        r#"
get fun `test predicate vm exit code`() {
    val (sender, harness, _) = deployHarness();
    val res = sendPing(sender, harness, 1);

    expect(res).toHaveTx({
        to: fun(addr: address): bool {
            println("vm-exit.to={}", addr);
            throw 777;
        },
    });
}
"#,
        "predicate_vm_exit_code_surfaces_to_user",
    );
}

#[test]
fn predicate_unexpected_stack_value_surfaces_to_user() {
    run_failure_case(
        "ae-predicate-unexpected-stack-value",
        r#"
fun badReturnPredicate(_: address): bool asm "DROP PUSHNULL";

get fun `test predicate unexpected stack value`() {
    val (sender, harness, _) = deployHarness();
    val res = sendPing(sender, harness, 1);

    expect(res).toHaveTx({
        to: badReturnPredicate,
    });
}
"#,
        "predicate_unexpected_stack_value_surfaces_to_user",
    );
}
