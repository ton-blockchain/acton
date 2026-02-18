//! Reserved for agent-dg.
//! Prefix: dg_stdlib_
//! Ownership: this file and tests/integration/snapshots/test_std_agent_dg/**
//! Agent-owned tests for net.sendExternal stateInit branch and tx search checks.

use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const DG_MESSAGES: &str = r#"
struct (0xD7000001) DgExternalPing {
    queryId: uint64
}

struct (0xD7000002) DgExternalNotice {
    hits: uint32
}
"#;

const DG_EXTERNAL_CONTRACT: &str = r#"
import "@stdlib/gas-payments"
import "dg_messages"

struct Storage {
    hits: uint32
}

fun loadStorage() {
    val data = contract.getData();
    val slice = data.beginParse();
    if (slice.remainingBitsCount() == 0 && slice.remainingRefsCount() == 0) {
        return Storage { hits: 0 };
    }
    return Storage.fromCell(data);
}

fun saveStorage(data: Storage) {
    contract.setData(data.toCell());
}

fun onExternalMessage() {
    acceptExternalMessage();

    var storage = loadStorage();
    storage.hits = storage.hits + 1;
    saveStorage(storage);

    createExternalLogMessage({
        dest: createAddressNone(),
        body: DgExternalNotice {
            hits: storage.hits,
        },
    }).send(SEND_MODE_REGULAR);
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}

get fun hits(): int {
    return loadStorage().hits;
}
"#;

const DG_IMPORTS: &str = r#"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
import "../contracts/dg_messages"
"#;

fn run_dg_project_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DG_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .file("contracts/dg_messages", DG_MESSAGES)
        .contract("dg_external_stateinit", DG_EXTERNAL_CONTRACT)
        .test_file("dg_send_external_stateinit", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn dg_stdlib_net_send_external_with_state_init_deploys_and_supports_transaction_presence_search_in_project_builder()
 {
    run_dg_project_success(
        "dg-stdlib-send-external-state-init-project-builder",
        r#"
get fun `test-dg-send-external-state-init-project-builder`() {
    val init = ContractState {
        code: build("dg_external_stateinit"),
        data: createEmptyCell(),
    };
    val externalInit = StateInit {
        fixedPrefixLength: null,
        special: null,
        code: init.code,
        data: init.data,
        library: beginCell().storeBool(false).endCell(),
    };
    val target = AutoDeployAddress { stateInit: init }.calculateAddress();

    expect(net.isDeployed(target)).toBeFalse();

    val txs = net.sendExternal(
        createExternalMessage(
            target,
            DgExternalPing { queryId: 9001 },
            externalInit,
        ),
    );

    // BUG: net.sendExternal drops external messages with stateInit, expected one deploy transaction, got empty SendResultList.
    expect(txs).toHaveLength(1);
    expect(txs).toHaveSuccessfulDeploy({ to: target });

    val found = txs.findTransaction<DgExternalPing>({
        deploy: true,
        success: true,
    });
    expect(found).toBeDefined();
    val foundTx = found.unwrap();
    expect(foundTx.getAccountAddress()).toEqual(target);
    expect(foundTx.loadBody<DgExternalPing>()).toEqual(DgExternalPing { queryId: 9001 });

    val missing = txs.findTransaction<DgExternalPing>({
        success: false,
    });
    expect(missing).toBeNone();

    expect(net.isDeployed(target)).toBeTrue();
    expect(net.runGetMethod<int>(target, "hits")).toEqual(1);

    val notice = txs.at(0).externals.at<DgExternalNotice>(0).loadBody();
    expect(notice.hits).toEqual(1);
}
"#,
        "integration/snapshots/test_std_agent_dg/dg_stdlib_net_send_external_with_state_init_deploys_and_supports_transaction_presence_search_in_project_builder.stdout.txt",
    );
}

#[test]
fn dg_stdlib_net_send_external_with_state_init_deploys_and_supports_transaction_presence_search_in_fixture_project()
 {
    let fixture = FixtureProject::load("basic");

    fs::write(
        fixture.path().join("contracts/dg_messages.tolk"),
        DG_MESSAGES,
    )
    .expect("failed to write fixture messages for dg sendExternal stateInit test");
    fs::write(
        fixture.path().join("contracts/dg_external_stateinit.tolk"),
        DG_EXTERNAL_CONTRACT,
    )
    .expect("failed to write fixture contract for dg sendExternal stateInit test");

    let acton_path = fixture.path().join("Acton.toml");
    let mut acton_toml = fs::read_to_string(&acton_path)
        .expect("failed to read fixture Acton.toml for dg sendExternal stateInit test");
    acton_toml.push_str(
        r#"

[contracts.dg_external_stateinit]
name = "DgExternalStateInit"
src = "contracts/dg_external_stateinit.tolk"
depends = []
"#,
    );
    fs::write(&acton_path, acton_toml)
        .expect("failed to update fixture Acton.toml for dg sendExternal stateInit test");

    let test_path = "tests/dg_send_external_stateinit.test.tolk";
    let source = format!(
        r#"{DG_IMPORTS}
get fun `test-dg-send-external-state-init-fixture-project`() {{
    val init = ContractState {{
        code: build("dg_external_stateinit"),
        data: createEmptyCell(),
    }};
    val externalInit = StateInit {{
        fixedPrefixLength: null,
        special: null,
        code: init.code,
        data: init.data,
        library: beginCell().storeBool(false).endCell(),
    }};
    val target = AutoDeployAddress {{ stateInit: init }}.calculateAddress();

    expect(net.isDeployed(target)).toBeFalse();

    val txs = net.sendExternal(
        createExternalMessage(
            target,
            DgExternalPing {{ queryId: 9002 }},
            externalInit,
        ),
    );

    // BUG: net.sendExternal drops external messages with stateInit, expected one deploy transaction, got empty SendResultList.
    expect(txs).toHaveLength(1);
    expect(txs).toHaveSuccessfulDeploy({{ to: target }});

    val found = txs.findTransaction<DgExternalPing>({{
        deploy: true,
        success: true,
    }});
    expect(found).toBeDefined();
    val foundTx = found.unwrap();
    expect(foundTx.getAccountAddress()).toEqual(target);
    expect(foundTx.loadBody<DgExternalPing>()).toEqual(DgExternalPing {{ queryId: 9002 }});

    val missing = txs.findTransaction<DgExternalPing>({{
        success: false,
    }});
    expect(missing).toBeNone();

    expect(net.isDeployed(target)).toBeTrue();
    expect(net.runGetMethod<int>(target, "hits")).toEqual(1);

    val notice = txs.at(0).externals.at<DgExternalNotice>(0).loadBody();
    expect(notice.hits).toEqual(1);
}}
"#
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write fixture test for dg sendExternal stateInit test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_std_agent_dg/dg_stdlib_net_send_external_with_state_init_deploys_and_supports_transaction_presence_search_in_fixture_project.stdout.txt",
        );
}
