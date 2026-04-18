use crate::support::TestOutputExt;
use crate::support::fixtures::FixtureProject;
use crate::support::project::ProjectBuilder;
use std::fs;

const DC_TRANSACTION_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
import "../../lib/types/transaction"

struct (0xDC700001) DcAddressPayload {
    queryId: uint64
}

struct DcAddressParts {
    tag: int
    anycastPresent: bool
    workchainRaw: int
    hash: int
}

fun parseDcAddressParts(addr: address): DcAddressParts {
    var addrSlice = addr.toCell().beginParse();
    return DcAddressParts {
        tag: addrSlice.loadUint(2),
        anycastPresent: addrSlice.loadBool(),
        workchainRaw: addrSlice.loadUint(8),
        hash: addrSlice.loadUint(256),
    };
}
"#;

fn run_transaction_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DC_TRANSACTION_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("dc_transaction_account_address", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn transaction_get_account_address_defaults_to_basechain_and_masterchain_override_changes_only_workchain()
 {
    run_transaction_success(
        "dc-stdlib-transaction-get-account-address-default-vs-masterchain",
        r#"
get fun `test dc stdlib transaction get account address default vs masterchain`() {
    val sender = testing.treasury("dc_sender_default");
    val destination = randomAddress("dc_destination_default");

    val txs = net.send(
        sender.address,
        createMessage({
            bounce: false,
            value: ton("0.2"),
            dest: destination,
            body: DcAddressPayload { queryId: 1 },
        }),
    );

    expect(txs).toHaveLength(1);
    val tx = txs.at(0).tx.load();

    val defaultAddress = tx.getAccountAddress();
    val baseAddress = tx.getAccountAddress(BASECHAIN);
    val masterAddress = tx.getAccountAddress(MASTERCHAIN);

    expect(defaultAddress).toEqual(destination);
    expect(baseAddress).toEqual(destination);
    expect(masterAddress).toNotEqual(destination);

    val defaultParts = parseDcAddressParts(defaultAddress);
    val baseParts = parseDcAddressParts(baseAddress);
    val masterParts = parseDcAddressParts(masterAddress);

    expect(defaultParts.tag).toEqual(0b10);
    expect(baseParts.tag).toEqual(0b10);
    expect(masterParts.tag).toEqual(0b10);
    expect(defaultParts.anycastPresent).toBeFalse();
    expect(baseParts.anycastPresent).toBeFalse();
    expect(masterParts.anycastPresent).toBeFalse();

    expect(defaultParts.workchainRaw).toEqual(0);
    expect(baseParts.workchainRaw).toEqual(0);
    expect(masterParts.workchainRaw).toEqual(255);

    expect(defaultParts.hash).toEqual(baseParts.hash);
    expect(defaultParts.hash).toEqual(masterParts.hash);
}
"#,
        "integration/snapshots/test-runner/transaction_get_account_address_defaults_to_basechain_and_masterchain_override_changes_only_workchain/transaction_get_account_address_defaults_to_basechain.stdout.txt",
    );
}

#[test]
fn transaction_get_account_address_masterchain_override_is_stable_in_fixture_project() {
    let fixture = FixtureProject::load("basic");
    let test_path = "tests/dc_transaction_get_account_address_fixture.test.tolk";
    let source = format!(
        r#"{DC_TRANSACTION_IMPORTS}
get fun `test dc stdlib transaction get account address masterchain stable`() {{
    val sender = testing.treasury("dc_sender_fixture");
    val destination = randomAddress("dc_destination_fixture");

    val txs = net.send(
        sender.address,
        createMessage({{
            bounce: false,
            value: ton("0.2"),
            dest: destination,
            body: DcAddressPayload {{ queryId: 2 }},
        }}),
    );

    expect(txs).toHaveLength(1);
    val tx = txs.at(0).tx.load();

    val baseAddress = tx.getAccountAddress(BASECHAIN);
    val masterAddressA = tx.getAccountAddress(MASTERCHAIN);
    val masterAddressB = tx.getAccountAddress(MASTERCHAIN);

    expect(masterAddressA).toEqual(masterAddressB);
    expect(masterAddressA).toNotEqual(baseAddress);

    val baseParts = parseDcAddressParts(baseAddress);
    val masterPartsA = parseDcAddressParts(masterAddressA);
    val masterPartsB = parseDcAddressParts(masterAddressB);

    expect(baseParts.workchainRaw).toEqual(0);
    expect(masterPartsA.workchainRaw).toEqual(255);
    expect(masterPartsB.workchainRaw).toEqual(255);

    expect(baseParts.hash).toEqual(masterPartsA.hash);
    expect(masterPartsA.hash).toEqual(masterPartsB.hash);
}}
"#
    );

    fs::write(fixture.path().join(test_path), source)
        .expect("failed to write dc fixture transaction test");

    fixture
        .acton()
        .test()
        .path(test_path)
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/transaction_get_account_address_defaults_to_basechain_and_masterchain_override_changes_only_workchain/transaction_get_account_address_masterchain_override.stdout.txt",
        );
}
