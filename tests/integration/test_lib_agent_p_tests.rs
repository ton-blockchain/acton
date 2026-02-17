//! Reserved integration test module for subagent P.
//!
//! Ownership boundary for agent P:
//! - tests/integration/test_lib_agent_p_tests.rs
//! - tests/integration/snapshots/test_lib_agent_p/**
//! - tests/integration/testdata/test_lib_agent_p/**
//! - tests/support/test_lib_agent_p/** (optional)
//!
//! Required test name prefix:
//! - p_lib_api_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

const ACTION_FAIL_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {
    reserveToncoinsOnBalance(ton("100"), RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);
}
"#;

const TEST_IMPORTS: &str = r#"
import "../../lib/io"
import "../../lib/build/build"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/testing/transaction_expect"
"#;

#[test]
fn p_lib_api_to_have_and_not_have_tx_by_action_exit_code() {
    let test_code = format!(
        r#"
            {}

            get fun `test-action-exit-code-filter`() {{
                val init = ContractState {{
                    code: build("simple"),
                    data: createEmptyCell(),
                }};
                val target = AutoDeployAddress {{ stateInit: init }}.calculateAddress();
                val sender = net.treasury("sender");

                val txs = net.send(sender.address, createMessage({{
                    bounce: false,
                    value: ton("1"),
                    dest: {{ stateInit: init }},
                    body: beginCell().storeUint(0x10, 32).endCell(),
                }}));

                expect(txs).toHaveTx({{
                    from: sender.address,
                    to: target,
                    actionExitCode: 37,
                }});
                expect(txs).toNotHaveTx({{
                    from: sender.address,
                    to: target,
                    actionExitCode: 38,
                }});
            }}
        "#,
        TEST_IMPORTS,
    );

    ProjectBuilder::new("p-lib-api-action-exit-code")
        .contract("simple", ACTION_FAIL_CONTRACT)
        .test_file("search_params", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_p/p_lib_api_to_have_and_not_have_tx_by_action_exit_code.stdout.txt",
        );
}

#[test]
fn p_lib_api_to_have_and_not_have_tx_by_compute_phase_skipped() {
    let test_code = format!(
        r#"
            {}

            get fun `test-compute-phase-skipped-filter`() {{
                val sender = net.treasury("sender");
                val missingAddress = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");

                val txs = net.send(sender.address, createMessage({{
                    bounce: false,
                    value: ton("1"),
                    dest: missingAddress,
                    body: beginCell().storeUint(0x20, 32).endCell(),
                }}));

                expect(txs).toHaveTx({{
                    from: sender.address,
                    to: missingAddress,
                    computePhaseSkipped: true,
                }});
                expect(txs).toNotHaveTx({{
                    from: sender.address,
                    to: missingAddress,
                    computePhaseSkipped: false,
                }});
            }}
        "#,
        TEST_IMPORTS,
    );

    ProjectBuilder::new("p-lib-api-compute-phase-skipped")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("search_params", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_p/p_lib_api_to_have_and_not_have_tx_by_compute_phase_skipped.stdout.txt",
        );
}

#[test]
fn p_lib_api_to_have_and_not_have_tx_by_body() {
    let test_code = format!(
        r#"
            {}

            get fun `test-body-filter`() {{
                val init = ContractState {{
                    code: build("simple"),
                    data: createEmptyCell(),
                }};
                val target = AutoDeployAddress {{ stateInit: init }}.calculateAddress();
                val sender = net.treasury("sender");

                val expectedBody = beginCell()
                    .storeUint(0xABCDEF01, 32)
                    .storeUint(777, 16)
                    .endCell();
                val differentBody = beginCell()
                    .storeUint(0xABCDEF01, 32)
                    .storeUint(778, 16)
                    .endCell();

                val txs = net.send(sender.address, createMessage({{
                    bounce: false,
                    value: ton("1"),
                    dest: {{ stateInit: init }},
                    body: expectedBody,
                }}));

                expect(txs).toHaveTx({{
                    from: sender.address,
                    to: target,
                    body: expectedBody,
                }});
                expect(txs).toNotHaveTx({{
                    from: sender.address,
                    to: target,
                    body: differentBody,
                }});
            }}
        "#,
        TEST_IMPORTS,
    );

    ProjectBuilder::new("p-lib-api-body")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("search_params", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_p/p_lib_api_to_have_and_not_have_tx_by_body.stdout.txt",
        );
}

#[test]
fn p_lib_api_to_have_and_not_have_tx_by_opcode() {
    let test_code = format!(
        r#"
            {}

            get fun `test-opcode-filter`() {{
                val init = ContractState {{
                    code: build("simple"),
                    data: createEmptyCell(),
                }};
                val target = AutoDeployAddress {{ stateInit: init }}.calculateAddress();
                val sender = net.treasury("sender");

                val txs = net.send(sender.address, createMessage({{
                    bounce: false,
                    value: ton("1"),
                    dest: {{ stateInit: init }},
                    body: beginCell().storeUint(0x11223344, 32).storeUint(1, 32).endCell(),
                }}));

                expect(txs).toHaveTx({{
                    from: sender.address,
                    to: target,
                    opcode: 0x11223344,
                }});
                // BUG: expected opcode filter 0x11223345 to reject tx with opcode 0x11223344, but matcher still reports it as found.
                expect(txs).toNotHaveTx({{
                    from: sender.address,
                    to: target,
                    opcode: 0x11223345,
                }});
            }}
        "#,
        TEST_IMPORTS,
    );

    ProjectBuilder::new("p-lib-api-opcode")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("search_params", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_p/p_lib_api_to_have_and_not_have_tx_by_opcode.stdout.txt",
        );
}

#[test]
fn p_lib_api_to_have_tx_with_bounced_opcode_prefix() {
    let test_code = format!(
        r#"
            {}

            get fun `test-bounced-opcode-filter`() {{
                val init = ContractState {{
                    code: build("simple"),
                    data: createEmptyCell(),
                }};
                val target = AutoDeployAddress {{ stateInit: init }}.calculateAddress();
                val sender = net.treasury("sender");

                net.send(sender.address, createMessage({{
                    bounce: false,
                    value: ton("1"),
                    dest: {{ stateInit: init }},
                }}));

                val payload = beginCell()
                    .storeUint(0x12345678, 32)
                    .storeUint(1, 32)
                    .endCell();
                val bouncedBody = beginCell()
                    .storeUint(0xFFFFFFFF, 32)
                    .storeSlice(payload.beginParse())
                    .endCell();

                val txs = net.send(sender.address, createMessage({{
                    bounce: false,
                    value: ton("0.5"),
                    dest: target,
                    body: bouncedBody,
                }}).bounced());

                expect(txs).toHaveTx({{
                    from: sender.address,
                    to: target,
                    bounced: true,
                    opcode: 0x12345678,
                }});
                expect(txs).toNotHaveTx({{
                    from: sender.address,
                    to: target,
                    bounced: false,
                    opcode: 0x12345678,
                }});
            }}
        "#,
        TEST_IMPORTS,
    );

    ProjectBuilder::new("p-lib-api-bounced-opcode")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("search_params", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test_lib_agent_p/p_lib_api_to_have_tx_with_bounced_opcode_prefix.stdout.txt",
        );
}
