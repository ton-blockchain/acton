//! Reserved for agent-dh.
//! Prefix: dh_stdlib_
//! Ownership: this file and tests/integration/snapshots/test-runner/test_runner_stdlib_dh_create_tests/**
//! Agent-owned tests for createExternalMessage src override and no-state-init encoding.

use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const DH_NETWORK_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"
import "../../lib/tlb/either"
import "../../lib/tlb/maybe"
import "../../lib/types/message"

struct (0xD8000001) DhTriggerExternal {
    queryId: uint64
}

fun dhExternalAddress(tag: uint32): any_address {
    return beginCell()
        .storeUint(0b01, 2)
        .storeUint(32, 9)
        .storeUint(tag, 32)
        .endCell()
        .beginParse()
        .loadAddressAny();
}
"#;

fn run_dh_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{DH_NETWORK_IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .test_file("dh_create_external_message", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn create_external_message_honors_explicit_external_src_and_omits_state_init() {
    run_dh_success_case(
        "dh-stdlib-create-external-src-override-no-init",
        r#"
get fun `test-dh-create-external-message-src-override-no-state-init`() {
    val dest = net.randomAddress("dh_dest_src_override");
    val src = dhExternalAddress(0xD1000001);

    val msg = createExternalMessage(
        dest,
        DhTriggerExternal { queryId: 41 },
        null,
        src,
    );

    val parsed = (msg.messageCell as Cell<Message<DhTriggerExternal, ExternalInMessageInfo>>)
        .load({ assertEndAfterReading: false });

    expect(parsed.info.dest).toEqual(dest);
    expect(parsed.info.src).toEqual(src);
    expect(parsed.info.importFee).toEqual(ton("0.1"));
    expect(parsed.init).toEqual(Maybe<Either<StateInit, Cell<StateInit>>>.none());
    expect(parsed.loadBody()).toEqual(DhTriggerExternal { queryId: 41 });
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_dh_create_tests/dh_stdlib_create_external_message_honors_explicit_external_src_and_omits_state_init.stdout.txt",
    );
}

#[test]
fn create_external_message_defaults_src_to_none_and_keeps_state_init_absent() {
    run_dh_success_case(
        "dh-stdlib-create-external-default-src-no-init",
        r#"
get fun `test-dh-create-external-message-default-src-no-state-init`() {
    val dest = net.randomAddress("dh_dest_default_src");

    val msg = createExternalMessage(
        dest,
        DhTriggerExternal { queryId: 99 },
    );

    val parsed = (msg.messageCell as Cell<Message<DhTriggerExternal, ExternalInMessageInfo>>)
        .load({ assertEndAfterReading: false });

    expect(parsed.info.dest).toEqual(dest);
    expect(parsed.info.src).toEqual(createAddressNone());
    expect(parsed.info.importFee).toEqual(ton("0.1"));
    expect(parsed.init).toEqual(Maybe<Either<StateInit, Cell<StateInit>>>.none());

    val body = parsed.loadBody();
    expect(body.queryId).toEqual(99);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_dh_create_tests/dh_stdlib_create_external_message_defaults_src_to_none_and_keeps_state_init_absent.stdout.txt",
    );
}
