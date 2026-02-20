use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EN_IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/testing/expect"

struct (0xEE100001) EnExternalNotice {
    queryId: uint64
}
"#;

const EN_NOOP_CONTRACT: &str = r#"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

fn run_find_external_out_message_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{EN_IMPORTS}\n{test_body}\n");

    ProjectBuilder::new(project_name)
        .contract("noop", EN_NOOP_CONTRACT)
        .test_file("find_external_out_message_compile_failure", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn find_external_out_message_typed_branch_reports_tuple_get_type_argument_compile_error() {
    run_find_external_out_message_success(
        "en-stdlib-find-external-out-message-typed-branch-bug",
        r#"
get fun `test-en-find-external-out-message-typed-branch-bug`() {
    val txs: SendResultList = [];

    val found = txs.findExternalOutMessage<EnExternalNotice>({});
    expect(found).toBeNone();
}
"#,
        "integration/snapshots/test-runner/find_external_out_message_typed_branch_reports_tuple_get_type_argument_compile_error/find_external_out_message_typed_branch_reports_tuple_get_type_argument_compile_error.stdout.txt",
    );
}

#[test]
fn find_external_out_message_default_branch_reports_tuple_get_type_argument_compile_error() {
    run_find_external_out_message_success(
        "en-stdlib-find-external-out-message-default-branch-bug",
        r#"
get fun `test-en-find-external-out-message-default-branch-bug`() {
    val txs: SendResultList = [];

    val found = txs.findExternalOutMessage({});
    expect(found).toBeNone();
}
"#,
        "integration/snapshots/test-runner/find_external_out_message_typed_branch_reports_tuple_get_type_argument_compile_error/find_external_out_message_default_branch_reports_tuple_get_type_argument_compile_error.stdout.txt",
    );
}
