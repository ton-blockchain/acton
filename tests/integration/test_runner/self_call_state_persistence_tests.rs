use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SNAPSHOT_DIR: &str = "integration/snapshots/test-runner/self_call_state_persistence";

const MESSAGES: &str = r"
struct (0x73656c66) SelfCall {}
";

const CONTRACT: &str = r#"
import "messages"

struct Storage {
    totalDeposits: uint32
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

fun onInternalMessage(in: InMessage) {
    if (!in.body.isEmpty()) {
        val _msg = lazy SelfCall.fromSlice(in.body);
        return;
    }

    var storage = Storage.load();
    storage.totalDeposits = storage.totalDeposits + 1;
    storage.save();

    val selfCall = beginCell()
        .storeUint(0x18, 6)
        .storeAddress(contract.getAddress())
        .storeCoins(0)
        .storeUint(0, 1 + 4 + 4 + 64 + 32 + 1 + 1)
        .storeUint(0x73656c66, 32)
        .endCell();

    sendRawMessage(selfCall, SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun totalDeposits(): int {
    return Storage.load().totalDeposits;
}
"#;

const SUSPICIOUS_STORE_ORDER_CONTRACT: &str = r#"
import "messages"

struct Storage {
    totalDeposits: uint32
}

fun Storage.load(): Storage {
    return Storage.fromCell(contract.getData());
}

fun Storage.save(self) {
    contract.setData(self.toCell());
}

fun onInternalMessage(in: InMessage) {
    if (!in.body.isEmpty()) {
        val _msg = lazy SelfCall.fromSlice(in.body);
        return;
    }

    var storage = Storage.load();
    storage.totalDeposits = storage.totalDeposits + 1;
    storage.save();

    val selfCall = beginCell()
        .storeUint(0x18, 6)
        .storeAddress(contract.getAddress())
        .storeCoins(0)
        .storeUint(0x73656c66, 32)
        .storeUint(0, 1 + 4 + 4 + 64 + 32 + 1 + 1)
        .endCell();

    sendRawMessage(selfCall, SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun totalDeposits(): int {
    return Storage.load().totalDeposits;
}
"#;

const TEST_IMPORTS: &str = r#"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/io"
import "../../lib/testing/expect"
import "../../lib/types/big_array"
import "../../lib/types/message"
import "../../lib/types/out_actions"
import "../contracts/messages"
"#;

fn run_success(project_name: &str, test_body: &str, snapshot_name: &str) {
    run_success_with_contract(project_name, CONTRACT, test_body, snapshot_name);
}

fn run_success_with_contract(
    project_name: &str,
    contract: &str,
    test_body: &str,
    snapshot_name: &str,
) {
    run_success_with_contract_with_backtrace(
        project_name,
        contract,
        test_body,
        snapshot_name,
        None,
    );
}

fn run_success_with_contract_with_backtrace(
    project_name: &str,
    contract: &str,
    test_body: &str,
    snapshot_name: &str,
    backtrace: Option<&str>,
) {
    let project = ProjectBuilder::new(project_name)
        .file("contracts/messages", MESSAGES)
        .contract("self_call_counter", contract)
        .test_file("self_call_state", &format!("{TEST_IMPORTS}\n{test_body}\n"))
        .build();
    let test_command = project.acton().test();
    let test_command = if let Some(backtrace) = backtrace {
        test_command.with_backtrace(backtrace)
    } else {
        test_command
    };

    test_command
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(&format!("{SNAPSHOT_DIR}/{snapshot_name}.stdout.txt"));
}

const RAW_SELF_CALL_OPCODE_BEFORE_COMMON_INFO_TAIL_TEST: &str = r#"
get fun `test raw self call opcode before common info tail`() {
    val sender = testing.treasury("sender");
    val init = ContractState {
        code: build("self_call_counter"),
        data: beginCell().storeUint(0, 32).endCell(),
    };
    val counterAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployRes = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("1"),
            dest: {
                stateInit: init,
            },
            body: SelfCall {},
        }),
    );
    expect(deployRes).toHaveSuccessfulDeploy({ to: counterAddress });
    val initialTotalDeposits = net.runGetMethod<int>(counterAddress, "totalDeposits");
    expect(initialTotalDeposits).toEqual(0);

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("10"),
            dest: counterAddress,
        }),
    );

    expect(txs).toHaveLength(1);
    expect(txs.findTransaction({
        from: sender.address,
        to: counterAddress,
        success: false,
    })).toBeNotNull();

    val rootActions = txs.at(0).allOutActions();
    expect(rootActions.size()).toEqual(1);
    expect(rootActions.at(0).kind()).toEqual("send-message");
    val selfCallSend = rootActions.getSendMessageAt(0);
    expect(selfCallSend).toBeNotNull();
    expect(selfCallSend!.mode).toEqual(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);

    println(txs);
    println("txCount={}", txs.size());

    val totalDeposits = net.runGetMethod<int>(counterAddress, "totalDeposits");
    println("totalDeposits={}", totalDeposits);
    expect(totalDeposits).toEqual(initialTotalDeposits);
}
"#;

#[test]
fn self_call_after_set_data_persists_storage_for_followup_get_method() {
    run_success(
        "ag-self-call-after-set-data-persists-storage",
        r#"
get fun `test self call after set data persists storage`() {
    val sender = testing.treasury("sender");
    val init = ContractState {
        code: build("self_call_counter"),
        data: beginCell().storeUint(0, 32).endCell(),
    };
    val counterAddress = AutoDeployAddress { stateInit: init }.calculateAddress();

    val deployRes = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("1"),
            dest: {
                stateInit: init,
            },
            body: SelfCall {},
        }),
    );
    expect(deployRes).toHaveSuccessfulDeploy({ to: counterAddress });
    val initialTotalDeposits = net.runGetMethod<int>(counterAddress, "totalDeposits");
    expect(initialTotalDeposits).toEqual(0);

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("10"),
            dest: counterAddress,
        }),
    );

    expect(txs).toHaveLength(2);
    expect(txs.findTransaction({
        from: sender.address,
        to: counterAddress,
        success: true,
    })).toBeNotNull();
    expect(txs.findTransaction<SelfCall>({
        from: counterAddress,
        to: counterAddress,
        success: true,
    })).toBeNotNull();

    val rootActions = txs.at(0).allOutActions();
    val selfCallActions = txs.at(1).allOutActions();
    expect(rootActions.size()).toEqual(1);
    expect(selfCallActions.size()).toEqual(0);
    expect(rootActions.at(0).kind()).toEqual("send-message");
    val selfCallSend = rootActions.getSendMessageAt(0);
    expect(selfCallSend).toBeNotNull();
    expect(selfCallSend!.mode).toEqual(SEND_MODE_CARRY_ALL_REMAINING_MESSAGE_VALUE);
    expect(rootActions.getSendMessageBodyAt<SelfCall>(0)).toBeNotNull();

    val totalDeposits = net.runGetMethod<int>(counterAddress, "totalDeposits");
    println("totalDeposits={}", totalDeposits);
    println("txCount={}", txs.size());
    expect(totalDeposits).toEqual(initialTotalDeposits + 1);
}
"#,
        "self_call_after_set_data_persists_storage_for_followup_get_method",
    );
}

#[test]
fn raw_self_call_opcode_before_common_info_tail_rolls_back_state() {
    run_success_with_contract(
        "ag-self-call-opcode-before-common-info-tail",
        SUSPICIOUS_STORE_ORDER_CONTRACT,
        RAW_SELF_CALL_OPCODE_BEFORE_COMMON_INFO_TAIL_TEST,
        "raw_self_call_opcode_before_common_info_tail_rolls_back_state",
    );
}

#[test]
fn raw_self_call_opcode_before_common_info_tail_backtrace_full_shows_action_location() {
    run_success_with_contract_with_backtrace(
        "ag-self-call-opcode-before-common-info-tail-backtrace",
        SUSPICIOUS_STORE_ORDER_CONTRACT,
        RAW_SELF_CALL_OPCODE_BEFORE_COMMON_INFO_TAIL_TEST,
        "raw_self_call_opcode_before_common_info_tail_backtrace_full_shows_action_location",
        Some("full"),
    );
}
