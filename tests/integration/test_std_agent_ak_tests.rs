//! Reserved integration test module for subagent AK.
//!
//! Ownership boundary for agent AK:
//! - tests/integration/test_std_agent_ak_tests.rs
//! - tests/integration/snapshots/test_std_agent_ak/**
//! - tests/integration/testdata/test_std_agent_ak/**
//! - tests/support/test_std_agent_ak/** (optional)
//!
//! Required test name prefix:
//! - ak_stdlib_

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const MESSAGE_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/tlb/either"
import "../../lib/tlb/maybe"
import "../../lib/types/message"

struct (0x1234ABCD) AkPayload {
    queryId: uint64
    amount: uint32
}

fun akNoInit(): Maybe<Either<StateInit, Cell<StateInit>>> {
    return Maybe<Either<StateInit, Cell<StateInit>>>.none();
}

fun akIntInfo(): IntMsgInfoRelaxed {
    return IntMsgInfoRelaxed {
        ihrDisabled: true,
        bounce: false,
        bounced: false,
        src: address("0:00000000000000000000000000000000000000000000000000000000000000AA")
            as any_address,
        dest: address("0:00000000000000000000000000000000000000000000000000000000000000BB"),
        value: CurrencyCollection {
            grams: 0,
            other: createEmptyMap<int32, varuint32>(),
        },
        extraFlags: 0,
        fwdFee: 0,
        createdLt: 0,
        createdAt: 0,
    };
}
"#;

fn run_message_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{MESSAGE_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .test_file("message_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn ak_stdlib_load_opcode_reads_opcode_from_either_left_body() {
    run_message_case(
        "ak-stdlib-load-opcode-left",
        r#"
get fun `test-ak-stdlib-load-opcode-left`() {
    val body = beginCell()
        .storeBool(false)
        .storeUint(0x1234ABCD, 32)
        .storeUint(7, 4)
        .endCell()
        .beginParse();

    val msg = MessageRelaxedGeneric {
        info: akIntInfo(),
        init: akNoInit(),
        body,
    };

    expect(msg.loadOpcode()).toEqual(0x1234ABCD);
}
"#,
        "integration/snapshots/test_std_agent_ak/ak_stdlib_load_opcode_reads_opcode_from_either_left_body.stdout.txt",
    );
}

#[test]
fn ak_stdlib_load_opcode_reads_opcode_from_either_right_body_ref() {
    run_message_case(
        "ak-stdlib-load-opcode-right-ref",
        r#"
get fun `test-ak-stdlib-load-opcode-right-ref`() {
    val body = beginCell()
        .storeBool(true)
        .storeRef(
            beginCell()
                .storeUint(0x2345BCDE, 32)
                .storeUint(15, 8)
                .endCell()
        )
        .endCell()
        .beginParse();

    val msg = MessageRelaxedGeneric {
        info: akIntInfo(),
        init: akNoInit(),
        body,
    };

    expect(msg.loadOpcode()).toEqual(0x2345BCDE);
}
"#,
        "integration/snapshots/test_std_agent_ak/ak_stdlib_load_opcode_reads_opcode_from_either_right_body_ref.stdout.txt",
    );
}

#[test]
fn ak_stdlib_load_opcode_returns_null_for_either_right_without_ref() {
    run_message_case(
        "ak-stdlib-load-opcode-right-without-ref",
        r#"
get fun `test-ak-stdlib-load-opcode-right-without-ref`() {
    val body = beginCell().storeBool(true).endCell().beginParse();

    val msg = MessageRelaxedGeneric {
        info: akIntInfo(),
        init: akNoInit(),
        body,
    };

    expect(msg.loadOpcode()).toBeNull();
}
"#,
        "integration/snapshots/test_std_agent_ak/ak_stdlib_load_opcode_returns_null_for_either_right_without_ref.stdout.txt",
    );
}

#[test]
fn ak_stdlib_load_opcode_returns_null_when_body_too_short() {
    run_message_case(
        "ak-stdlib-load-opcode-short-body",
        r#"
get fun `test-ak-stdlib-load-opcode-short-body`() {
    val body = beginCell().storeBool(false).storeUint(0b1010, 4).endCell().beginParse();

    val msg = MessageRelaxedGeneric {
        info: akIntInfo(),
        init: akNoInit(),
        body,
    };

    expect(msg.loadOpcode()).toBeNull();
}
"#,
        "integration/snapshots/test_std_agent_ak/ak_stdlib_load_opcode_returns_null_when_body_too_short.stdout.txt",
    );
}

#[test]
fn ak_stdlib_load_opcode_without_skip_bounce_returns_bounce_prefix() {
    run_message_case(
        "ak-stdlib-load-opcode-bounce-prefix-without-skip",
        r#"
get fun `test-ak-stdlib-load-opcode-bounce-prefix-without-skip`() {
    val body = beginCell()
        .storeBool(false)
        .storeUint(0xFFFFFFFF, 32)
        .storeUint(0x3456CDEF, 32)
        .endCell()
        .beginParse();

    val msg = MessageRelaxedGeneric {
        info: akIntInfo(),
        init: akNoInit(),
        body,
    };

    expect(msg.loadOpcode(false)).toEqual(0xFFFFFFFF);
}
"#,
        "integration/snapshots/test_std_agent_ak/ak_stdlib_load_opcode_without_skip_bounce_returns_bounce_prefix.stdout.txt",
    );
}

#[test]
fn ak_stdlib_load_opcode_with_skip_bounce_returns_nested_opcode() {
    run_message_case(
        "ak-stdlib-load-opcode-bounce-prefix-with-skip",
        r#"
get fun `test-ak-stdlib-load-opcode-bounce-prefix-with-skip`() {
    val body = beginCell()
        .storeBool(false)
        .storeUint(0xFFFFFFFF, 32)
        .storeUint(0x3456CDEF, 32)
        .endCell()
        .beginParse();

    val msg = MessageRelaxedGeneric {
        info: akIntInfo(),
        init: akNoInit(),
        body,
    };

    expect(msg.loadOpcode(true)).toEqual(0x3456CDEF);
}
"#,
        "integration/snapshots/test_std_agent_ak/ak_stdlib_load_opcode_with_skip_bounce_returns_nested_opcode.stdout.txt",
    );
}

#[test]
fn ak_stdlib_message_relaxed_load_body_returns_either_left_value() {
    run_message_case(
        "ak-stdlib-message-load-body-left",
        r#"
get fun `test-ak-stdlib-message-load-body-left`() {
    val payload = AkPayload {
        queryId: 11,
        amount: 22,
    };

    val msg = MessageRelaxed<AkPayload> {
        info: akIntInfo(),
        init: akNoInit(),
        body: Either<AkPayload, Cell<AkPayload>>.left(payload),
    };

    expect(msg.loadBody()).toEqual(payload);
}
"#,
        "integration/snapshots/test_std_agent_ak/ak_stdlib_message_relaxed_load_body_returns_either_left_value.stdout.txt",
    );
}

#[test]
fn ak_stdlib_message_relaxed_load_body_returns_either_right_cell_value() {
    run_message_case(
        "ak-stdlib-message-load-body-right",
        r#"
get fun `test-ak-stdlib-message-load-body-right`() {
    val payload = AkPayload {
        queryId: 77,
        amount: 88,
    };
    val payloadCell = payload.toCell() as Cell<AkPayload>;

    val msg = MessageRelaxed<AkPayload> {
        info: akIntInfo(),
        init: akNoInit(),
        body: Either<AkPayload, Cell<AkPayload>>.right(payloadCell),
    };

    expect(msg.loadBody()).toEqual(payload);
}
"#,
        "integration/snapshots/test_std_agent_ak/ak_stdlib_message_relaxed_load_body_returns_either_right_cell_value.stdout.txt",
    );
}
