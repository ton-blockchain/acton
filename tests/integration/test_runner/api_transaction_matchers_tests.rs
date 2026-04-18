use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const ACTION_FAIL_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {
    reserveToncoinsOnBalance(ton("100"), RESERVE_MODE_BOUNCE_ON_ACTION_FAIL);
}
"#;

const TEST_IMPORTS: &str = r#"
import "../../lib/io"
import "../../lib/build"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/testing/expect"
"#;

#[test]
fn to_have_and_not_have_tx_by_action_exit_code() {
    let test_code = format!(
        r#"
            {TEST_IMPORTS}

            get fun `test-action-exit-code-filter`() {{
                val init = ContractState {{
                    code: build("simple"),
                    data: createEmptyCell(),
                }};
                val target = AutoDeployAddress {{ stateInit: init }}.calculateAddress();
                val sender = testing.treasury("sender");

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
            "integration/snapshots/test-runner/api_transaction_matchers/lib_api_to_have_and_not_have_tx_by_action_exit_code.stdout.txt",
        );
}

#[test]
fn to_have_and_not_have_tx_by_compute_phase_skipped() {
    let test_code = format!(
        r#"
            {TEST_IMPORTS}

            get fun `test compute phase skipped filter`() {{
                val sender = testing.treasury("sender");
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
            "integration/snapshots/test-runner/api_transaction_matchers/lib_api_to_have_and_not_have_tx_by_compute_phase_skipped.stdout.txt",
        );
}

#[test]
fn compute_skipped_success_and_exit_code_filters_have_consistent_scalar_semantics() {
    let test_code = format!(
        r#"
            {TEST_IMPORTS}

            get fun `test compute skipped success and exit code semantics`() {{
                val sender = testing.treasury("sender");
                val missingAddress = address("EQC2jeGorIAFh2LXwsDjHfRK-GSo9UzchdIEMh24A7T7AHot");

                val txs = net.send(sender.address, createMessage({{
                    bounce: false,
                    value: ton("1"),
                    dest: missingAddress,
                    body: beginCell().storeUint(0x21, 32).endCell(),
                }}));

                expect(txs).toHaveTx({{
                    from: sender.address,
                    to: missingAddress,
                    success: false,
                }});
                expect(txs).toNotHaveTx({{
                    from: sender.address,
                    to: missingAddress,
                    success: true,
                }});
                expect(txs).toNotHaveTx({{
                    from: sender.address,
                    to: missingAddress,
                    exitCode: 0,
                }});
                expect(txs).toNotHaveTx({{
                    from: sender.address,
                    to: missingAddress,
                    exitCode: 77,
                }});

                val failed = txs.findTransaction({{
                    from: sender.address,
                    to: missingAddress,
                    success: false,
                }});
                expect(failed).toBeNotNull();

                val impossible = txs.findTransaction({{
                    from: sender.address,
                    to: missingAddress,
                    success: true,
                }});
                expect(impossible).toBeNull();
            }}
        "#,
    );

    ProjectBuilder::new("p-lib-api-compute-skipped-success-semantics")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("search_params", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_transaction_matchers/lib_api_compute_skipped_success_and_exit_code_semantics.stdout.txt",
        );
}

#[test]
fn to_have_and_not_have_tx_by_body() {
    let test_code = format!(
        r#"
            {TEST_IMPORTS}

            get fun `test body filter`() {{
                val init = ContractState {{
                    code: build("simple"),
                    data: createEmptyCell(),
                }};
                val target = AutoDeployAddress {{ stateInit: init }}.calculateAddress();
                val sender = testing.treasury("sender");

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
            "integration/snapshots/test-runner/api_transaction_matchers/lib_api_to_have_and_not_have_tx_by_body.stdout.txt",
        );
}

#[test]
fn to_have_and_not_have_tx_by_opcode() {
    let test_code = format!(
        r#"
            {TEST_IMPORTS}

            get fun `test opcode filter`() {{
                val init = ContractState {{
                    code: build("simple"),
                    data: createEmptyCell(),
                }};
                val target = AutoDeployAddress {{ stateInit: init }}.calculateAddress();
                val sender = testing.treasury("sender");

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
                expect(txs).toNotHaveTx({{
                    from: sender.address,
                    to: target,
                    opcode: 0x11223345,
                }});
            }}
        "#,
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
            "integration/snapshots/test-runner/api_transaction_matchers/lib_api_to_have_and_not_have_tx_by_opcode.stdout.txt",
        );
}

#[test]
fn find_transaction_by_explicit_opcode_without_generic() {
    let test_code = format!(
        r#"
            {TEST_IMPORTS}

            get fun `test find transaction opcode filter`() {{
                val init = ContractState {{
                    code: build("simple"),
                    data: createEmptyCell(),
                }};
                val target = AutoDeployAddress {{ stateInit: init }}.calculateAddress();
                val sender = testing.treasury("sender");

                val txs = net.send(sender.address, createMessage({{
                    bounce: false,
                    value: ton("1"),
                    dest: {{ stateInit: init }},
                    body: beginCell().storeUint(0x55667788, 32).storeUint(5, 32).endCell(),
                }}));

                val found = txs.findTransaction({{
                    from: sender.address,
                    to: target,
                    opcode: 0x55667788,
                }});
                expect(found).toBeNotNull();

                val missing = txs.findTransaction({{
                    from: sender.address,
                    to: target,
                    opcode: 0x55667789,
                }});
                expect(missing).toBeNull();
            }}
        "#,
    );

    ProjectBuilder::new("p-lib-api-find-transaction-opcode")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("search_params", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_transaction_matchers/lib_api_find_transaction_by_explicit_opcode_without_generic.stdout.txt",
        );
}

#[test]
fn to_have_tx_with_bounced_opcode_prefix() {
    let test_code = format!(
        r#"
            {TEST_IMPORTS}

            get fun `test bounced opcode filter`() {{
                val init = ContractState {{
                    code: build("simple"),
                    data: createEmptyCell(),
                }};
                val target = AutoDeployAddress {{ stateInit: init }}.calculateAddress();
                val sender = testing.treasury("sender");

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
            "integration/snapshots/test-runner/api_transaction_matchers/lib_api_to_have_tx_with_bounced_opcode_prefix.stdout.txt",
        );
}

#[test]
fn bounced_opcode_requires_explicit_bounced_flag_on_scalar_path() {
    let test_code = format!(
        r#"
            {TEST_IMPORTS}

            get fun `test bounced opcode requires explicit flag`() {{
                val init = ContractState {{
                    code: build("simple"),
                    data: createEmptyCell(),
                }};
                val target = AutoDeployAddress {{ stateInit: init }}.calculateAddress();
                val sender = testing.treasury("sender");

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
                    opcode: 0x12345678,
                }});

                val missing = txs.findTransaction({{
                    from: sender.address,
                    to: target,
                    opcode: 0x12345678,
                }});
                expect(missing).toBeNull();
            }}
        "#,
    );

    ProjectBuilder::new("p-lib-api-bounced-opcode-requires-flag")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("search_params", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/api_transaction_matchers/lib_api_bounced_opcode_requires_explicit_bounced_flag.stdout.txt",
        );
}
