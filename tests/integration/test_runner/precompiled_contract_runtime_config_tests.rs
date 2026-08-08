use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const PRECOMPILED_RUNTIME_CONTRACT: &str = r#"
struct Storage {
    lastGas: int32
}

fun getPrecompiledGas(): int?
    asm "GETPRECOMPILEDGAS"

fun onInternalMessage(_: InMessage) {
    val gas = getPrecompiledGas();
    contract.setData(Storage {
        lastGas: gas == null ? -1 : gas!,
    }.toCell());
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun lastGas(): int {
    return Storage.fromCell(contract.getData()).lastGas;
}

get fun observedPrecompiledGas(): int? {
    return getPrecompiledGas();
}
"#;

const PRECOMPILED_RUNTIME_IMPORTS: &str = r#"
import "../../lib/build"
import "../../lib/emulation/config"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../../lib/types/transaction"

const PRECOMPILED_FIXED_GAS = 777
const PRECOMPILED_WRONG_GAS = 333

struct Storage {
    lastGas: int32
}

fun computeVmSteps(tx: TlbTransaction): int {
    val description = tx.description.load();
    if (description is TlbTransOrd) {
        if (description.computePh is TlbTrComputeVm) {
            return description.computePh.data.load().vmSteps as int;
        }
    }

    return -1;
}

fun applyPrecompiledConfig(code: cell, fixedGas: int?, wrongGas: int? = null): void {
    var precompiled = PrecompiledContractsConfig {
        list: createEmptyMap<uint256, PrecompiledSmartContract>(),
    };

    if (wrongGas != null) {
        expect(precompiled.addContractGas(code.hash() + 1, wrongGas!)).toBeTrue();
    }

    if (fixedGas != null) {
        expect(precompiled.addContractGas(code.hash(), fixedGas!)).toBeTrue();
    }

    var config = testing.getConfig();
    config.setPrecompiledContractsConfig(precompiled);
    expect(testing.setConfig(config)).toBeTrue();
}

fun deployRuntimeContract(testName: string, fixedGas: int?, wrongGas: int? = null): (Treasury, address) {
    val code = build("receiver");
    applyPrecompiledConfig(code, fixedGas, wrongGas);

    val stateInit = ContractState {
        code,
        data: Storage { lastGas: -1 }.toCell(),
    };
    val receiver = AutoDeployAddress {
        stateInit,
    }.calculateAddress();
    val sender = testing.treasury(testName);

    val deploy = createMessage({
        bounce: false,
        value: ton("1"),
        dest: {
            stateInit,
        },
    });
    val deployTxs = net.send(sender.address, deploy);
    expect(deployTxs).toHaveSuccessfulDeploy({
        from: sender.address,
        to: receiver,
    });

    return (sender, receiver);
}

fun sendRuntimeMessage(sender: Treasury, receiver: address): SendResult {
    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: receiver,
            body: beginCell().storeUint(0xC0DE, 16).endCell(),
        }),
    );
    expect(txs).toHaveSuccessfulTx({
        from: sender.address,
        to: receiver,
    });

    return txs.at(0);
}
"#;

fn run_precompiled_runtime_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{PRECOMPILED_RUNTIME_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("receiver", PRECOMPILED_RUNTIME_CONTRACT)
        .test_file("precompiled_runtime", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn precompiled_code_hash_config_sets_fixed_transaction_gas_and_c7_value() {
    run_precompiled_runtime_case(
        "precompiled-runtime-fixed-gas-and-c7",
        r#"
get fun `test precompiled fixed gas and c7`() {
    val (sender, receiver) = deployRuntimeContract(
        "precompiled_runtime_fixed_gas_sender",
        PRECOMPILED_FIXED_GAS,
    );

    val getterGas: int? = net.runGetMethod(receiver, "observedPrecompiledGas");
    expect(getterGas).toEqual(PRECOMPILED_FIXED_GAS);

    val tx = sendRuntimeMessage(sender, receiver);
    expect(tx.gasUsed).toEqual(PRECOMPILED_FIXED_GAS);
    expect(tx.tx.load().getUsedGas()).toEqual(PRECOMPILED_FIXED_GAS);
    expect(computeVmSteps(tx.tx.load())).toEqual(0);
    expect(net.runGetMethod<int>(receiver, "lastGas")).toEqual(PRECOMPILED_FIXED_GAS);
}
"#,
        "integration/snapshots/test-runner/precompiled_contract_runtime_config/precompiled_code_hash_config_sets_fixed_transaction_gas_and_c7_value.stdout.txt",
    );
}

#[test]
fn precompiled_config_does_not_match_when_only_neighbor_hash_is_present() {
    run_precompiled_runtime_case(
        "precompiled-runtime-wrong-hash-only",
        r#"
get fun `test precompiled wrong hash only does not match`() {
    val (sender, receiver) = deployRuntimeContract(
        "precompiled_runtime_wrong_hash_sender",
        null,
        PRECOMPILED_WRONG_GAS,
    );

    val getterGas: int? = net.runGetMethod(receiver, "observedPrecompiledGas");
    expect(getterGas).toBeNull();

    val tx = sendRuntimeMessage(sender, receiver);
    expect(tx.gasUsed).toNotEqual(PRECOMPILED_WRONG_GAS);
    expect(computeVmSteps(tx.tx.load())).toBeGreater(0);
    expect(net.runGetMethod<int>(receiver, "lastGas")).toEqual(-1);
}
"#,
        "integration/snapshots/test-runner/precompiled_contract_runtime_config/precompiled_config_does_not_match_when_only_neighbor_hash_is_present.stdout.txt",
    );
}

#[test]
fn precompiled_empty_config_does_not_match_any_contract() {
    run_precompiled_runtime_case(
        "precompiled-runtime-empty-config",
        r#"
get fun `test precompiled empty config does not match`() {
    val (sender, receiver) = deployRuntimeContract(
        "precompiled_runtime_empty_config_sender",
        null,
    );

    val getterGas: int? = net.runGetMethod(receiver, "observedPrecompiledGas");
    expect(getterGas).toBeNull();

    val tx = sendRuntimeMessage(sender, receiver);
    expect(tx.gasUsed).toNotEqual(PRECOMPILED_FIXED_GAS);
    expect(computeVmSteps(tx.tx.load())).toBeGreater(0);
    expect(net.runGetMethod<int>(receiver, "lastGas")).toEqual(-1);
}
"#,
        "integration/snapshots/test-runner/precompiled_contract_runtime_config/precompiled_empty_config_does_not_match_any_contract.stdout.txt",
    );
}

#[test]
fn precompiled_code_hash_entry_wins_over_unrelated_neighbor_hash() {
    run_precompiled_runtime_case(
        "precompiled-runtime-code-hash-wins",
        r#"
get fun `test precompiled code hash wins over unrelated hash`() {
    val (sender, receiver) = deployRuntimeContract(
        "precompiled_runtime_code_hash_sender",
        PRECOMPILED_FIXED_GAS,
        PRECOMPILED_WRONG_GAS,
    );

    val tx = sendRuntimeMessage(sender, receiver);
    expect(tx.gasUsed).toEqual(PRECOMPILED_FIXED_GAS);
    expect(tx.gasUsed).toNotEqual(PRECOMPILED_WRONG_GAS);
    expect(computeVmSteps(tx.tx.load())).toEqual(0);
    expect(net.runGetMethod<int>(receiver, "lastGas")).toEqual(PRECOMPILED_FIXED_GAS);
}
"#,
        "integration/snapshots/test-runner/precompiled_contract_runtime_config/precompiled_code_hash_entry_wins_over_unrelated_neighbor_hash.stdout.txt",
    );
}
