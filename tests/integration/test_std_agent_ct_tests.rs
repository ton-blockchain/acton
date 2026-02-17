//! Reserved for agent-ct.
//! Prefix: ct_stdlib_
//! Ownership: this file and tests/integration/snapshots/test_std_agent_ct/**
//! Agent will add targeted stdlib integration tests here.

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const CT_MESSAGES: &str = r#"
struct (0xC7000001) CtTriggerExternal {
    queryId: uint64
}

struct (0xC7000002) CtExternalNotice {
    count: uint32
}
"#;

const CT_EXTERNAL_CONTRACT: &str = r#"
import "@stdlib/gas-payments"
import "messages"

struct Storage {
    externalCount: uint32
}

fun loadStorage() {
    val data = contract.getData();
    val slice = data.beginParse();
    if (slice.remainingBitsCount() == 0 && slice.remainingRefsCount() == 0) {
        return Storage { externalCount: 0 };
    }
    return Storage.fromCell(data);
}

fun saveStorage(data: Storage) {
    contract.setData(data.toCell());
}

fun onExternalMessage() {
    acceptExternalMessage();

    var storage = loadStorage();
    storage.externalCount = storage.externalCount + 1;
    saveStorage(storage);

    createExternalLogMessage({
        dest: createAddressNone(),
        body: CtExternalNotice {
            count: storage.externalCount,
        },
    }).send(SEND_MODE_REGULAR);
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const CT_NETWORK_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../contracts/messages"

fun deployCtExternalHarness() {
    val sender = net.treasury("ct_sender");

    val externalInit = ContractState {
        code: build("external"),
        data: createEmptyCell(),
    };
    val externalAddress = AutoDeployAddress { stateInit: externalInit }.calculateAddress();

    val deployTxs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("1"),
            dest: {
                stateInit: externalInit,
            },
        }),
    );
    expect(deployTxs).toHaveSuccessfulDeploy({ to: externalAddress });

    return externalAddress;
}
"#;

fn run_ct_stdlib_failure(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{CT_NETWORK_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .file("contracts/messages", CT_MESSAGES)
        .contract("external", CT_EXTERNAL_CONTRACT)
        .test_file("ct_extoutlist_atornull", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn ct_stdlib_ext_out_list_at_or_null_out_of_range_returns_null_bug() {
    run_ct_stdlib_failure(
        "ct-stdlib-ext-out-list-atornull-out-of-range",
        r#"
get fun `test-ct-ext-out-list-atornull-out-of-range`() {
    val externalAddress = deployCtExternalHarness();

    val txs = net.sendExternal(
        createExternalMessage(externalAddress, CtTriggerExternal { queryId: 2 }),
    );
    expect(txs).toHaveLength(1);

    val externals = txs.at(0).externals;
    expect(externals.size()).toEqual(1);

    // BUG: ExtOutList.atOrNull should return null for out-of-range indices, expected null, got exit_code=5 (tuple index out of range).
    val missing = externals.atOrNull<CtExternalNotice>(1);
    expect(missing).toBeNull();
}
"#,
        "integration/snapshots/test_std_agent_ct/ct_stdlib_ext_out_list_at_or_null_out_of_range_returns_null_bug.stdout.txt",
    );
}
