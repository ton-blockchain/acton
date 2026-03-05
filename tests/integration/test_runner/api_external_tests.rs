use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EXTERNAL_CONTRACT: &str = r#"
import "@stdlib/gas-payments"

struct (0x70000001) TriggerExternal {
    id: uint32
}

struct (0x70000002) ExternalAlpha {
    value: uint32
}

struct (0x70000003) ExternalBeta {
    value: uint32
}

fun externalDest() {
    return any_address.fromCell(
        beginCell()
            .storeUint(0b01, 2)
            .storeUint(16, 9)
            .storeUint(0xBEEF, 16)
            .endCell(),
    );
}

fun onExternalMessage() {
    acceptExternalMessage();

    createExternalLogMessage({
        dest: createAddressNone(),
        body: ExternalAlpha { value: 111 },
    }).send(SEND_MODE_REGULAR);

    createExternalLogMessage({
        dest: externalDest(),
        body: ExternalBeta { value: 222 },
    }).send(SEND_MODE_REGULAR);
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const EXTERNAL_API_TEST_PRELUDE: &str = r#"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/types/transaction"

struct (0x70000001) TriggerExternal {
    id: uint32
}

struct (0x70000002) ExternalAlpha {
    value: uint32
}

struct (0x70000003) ExternalBeta {
    value: uint32
}

struct ExternalHarness {
    address: address
    init: ContractState
}

fun ExternalHarness.create() {
    val init = ContractState {
        code: build("external"),
        data: createEmptyCell(),
    };
    val address = AutoDeployAddress { stateInit: init }.calculateAddress();
    return ExternalHarness { address, init };
}

fun deployHarness() {
    val harness = ExternalHarness.create();
    val deployer = net.treasury("deployer");
    val deployRes = net.send(
        deployer.address,
        createMessage({
            bounce: false,
            value: ton("1"),
            dest: {
                stateInit: harness.init,
            },
        }),
    );
    expect(deployRes).toHaveSuccessfulDeploy({ to: harness.address });
    return (harness, deployer);
}

fun externalDest() {
    return any_address.fromCell(
        beginCell()
            .storeUint(0b01, 2)
            .storeUint(16, 9)
            .storeUint(0xBEEF, 16)
            .endCell(),
    );
}
"#;

fn with_prelude(test_body: &str) -> String {
    format!("{EXTERNAL_API_TEST_PRELUDE}\n{test_body}")
}

fn run_success_case(project_name: &str, test_body: &str, test_name: &str) {
    let source = with_prelude(test_body);
    ProjectBuilder::new(project_name)
        .contract("external", EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains(test_name);
}

fn run_failure_case(project_name: &str, test_body: &str, test_name: &str) {
    let source = with_prelude(test_body);
    ProjectBuilder::new(project_name)
        .contract("external", EXTERNAL_CONTRACT)
        .test_file("external_api", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains(test_name);
}

#[test]
fn send_external_collects_external_messages_with_deterministic_order() {
    run_success_case(
        "o-lib-api-send-external-collects-externals",
        r#"
get fun `test-send-external-collects-externals`() {
    val (harness, _) = deployHarness();

    val txs = net.sendExternal(
        createExternalMessage(harness.address, TriggerExternal { id: 1 }.toCell()),
    )!;

    expect(txs).toHaveLength(1);
    val tx = txs.at(0);
    expect(tx.externals).toHaveLength(2);

    val alpha = tx.externals.at<ExternalAlpha>(0);
    expect(alpha.info.src).toEqual(harness.address);
    expect(alpha.info.dest).toEqual(createAddressNone());
    expect(alpha.loadBody()).toEqual(ExternalAlpha { value: 111 });

    val beta = tx.externals.at<ExternalBeta>(1);
    expect(beta.info.src).toEqual(harness.address);
    expect(beta.info.dest).toEqual(externalDest());
    expect(beta.loadBody()).toEqual(ExternalBeta { value: 222 });
}
"#,
        "send-external-collects-externals",
    );
}

#[test]
fn create_external_message_accepts_explicit_external_src() {
    run_success_case(
        "o-lib-api-create-external-explicit-src",
        r#"
get fun `test-create-external-message-with-external-src`() {
    val (harness, _) = deployHarness();

    val txs = net.sendExternal(
        createExternalMessage(
            harness.address,
            TriggerExternal { id: 2 }.toCell(),
            null,
            externalDest(),
        ),
    )!;

    expect(txs).toHaveLength(1);
    expect(txs.at(0).externals).toHaveLength(2);

    val first = txs.at(0).externals.at<ExternalAlpha>(0);
    expect(first.loadBody()).toEqual(ExternalAlpha { value: 111 });
}
"#,
        "create-external-message-with-external-src",
    );
}

#[test]
fn send_external_is_repeatable_for_same_contract() {
    run_success_case(
        "o-lib-api-send-external-repeatable",
        r#"
get fun `test-send-external-repeatable`() {
    val (harness, _) = deployHarness();

    val first = net.sendExternal(
        createExternalMessage(harness.address, TriggerExternal { id: 3 }.toCell()),
    )!;
    val second = net.sendExternal(
        createExternalMessage(harness.address, TriggerExternal { id: 4 }.toCell()),
    )!;

    expect(first).toHaveLength(1);
    expect(second).toHaveLength(1);
    expect(first.at(0).externals).toHaveLength(2);
    expect(second.at(0).externals).toHaveLength(2);

    val firstAlpha = first.at(0).externals.at<ExternalAlpha>(0).loadBody();
    val secondAlpha = second.at(0).externals.at<ExternalAlpha>(0).loadBody();
    expect(firstAlpha).toEqual(ExternalAlpha { value: 111 });
    expect(secondAlpha).toEqual(ExternalAlpha { value: 111 });
}
"#,
        "send-external-repeatable",
    );
}

#[test]
fn send_external_returns_null_when_deployed_contract_has_too_low_balance() {
    run_success_case(
        "o-lib-api-send-external-low-balance-rejected",
        r#"
get fun `test-send-external-low-balance-rejected`() {
    val (harness, _) = deployHarness();

    val tinyBalanceSource = net.randomAddress("o_external_tiny_balance_source");
    net.topUp(tinyBalanceSource, 1);

    val harnessAcc = net.getAccount(harness.address);
    val tinyBalanceAcc = net.getAccount(tinyBalanceSource);

    expect(harnessAcc is AccountInfo).toBeTrue();
    expect(tinyBalanceAcc is AccountInfo).toBeTrue();

    if (harnessAcc is AccountInfo && tinyBalanceAcc is AccountInfo) {
        val lowBalanceAcc = AccountInfo {
            addr: harness.address,
            storageStat: harnessAcc.storageStat,
            storage: {
                lastTransLt: harnessAcc.storage.lastTransLt,
                balance: tinyBalanceAcc.storage.balance,
                state: harnessAcc.storage.state,
            },
        };
        net.setAccount(harness.address, lowBalanceAcc);
    }

    expect(net.balance(harness.address)).toEqual(1);

    val txs = net.sendExternal(
        createExternalMessage(harness.address, TriggerExternal { id: 6 }.toCell()),
    );
    expect(txs == null).toBeTrue();
}
"#,
        "send-external-low-balance-rejected",
    );
}

#[test]
fn create_external_message_rejects_internal_src() {
    run_failure_case(
        "o-lib-api-create-external-rejects-internal-src",
        r#"
get fun `test-create-external-message-rejects-internal-src`() {
    val (harness, deployer) = deployHarness();

    createExternalMessage(
        harness.address,
        TriggerExternal { id: 5 }.toCell(),
        null,
        deployer.address,
    );
}
"#,
        "create-external-message-rejects-internal-src",
    );
}

#[test]
fn find_external_out_message_has_generic_compilation_bug() {
    run_success_case(
        "o-lib-api-find-external-out-generic-bug",
        r#"
get fun `test-find-external-out-message-bug`() {
    val (harness, _) = deployHarness();

    val txs = net.sendExternal(
        createExternalMessage(harness.address, TriggerExternal { id: 5 }.toCell()),
    );

    val found = txs!.findExternalOutMessage<ExternalAlpha>({
        from: harness.address,
        to: createAddressNone(),
    });

    expect(found).toBeDefined();
}
"#,
        "find-external-out-message-bug",
    );
}
