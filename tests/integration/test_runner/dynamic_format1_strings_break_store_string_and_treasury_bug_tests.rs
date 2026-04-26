use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const IMPORTS: &str = r#"
import "../../lib/emulation/network"
import "../../lib/emulation/testing"
import "../../lib/fmt"
import "../../lib/io"
"#;

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

fn run_dynamic_string_success_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!("{IMPORTS}\n{test_body}\n");
    ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("dynamic_string_bug", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(2)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn format1_result_can_be_stored_with_builder_store_string() {
    run_dynamic_string_success_case(
        "bz-dynamic-format1-store-string-regression",
        r#"
get fun `test bz literal store string works`() {
    val literal = "hello world";
    val stored = beginCell().storeString(literal).endCell();
    println(stored);
}

get fun `test bz format1 store string not a cell`() {
    val dynamic = format("hello {}", "world");
    val stored = beginCell().storeString(dynamic).endCell();
    println(stored);
}
"#,
        "integration/snapshots/test-runner/dynamic_format1_strings_break_store_string_and_treasury_bug/format1_result_breaks_builder_store_string_bug.stdout.txt",
    );
}

#[test]
fn format1_result_can_be_used_as_treasury_name() {
    run_dynamic_string_success_case(
        "bz-dynamic-format1-treasury-name-regression",
        r#"
get fun `test bz static treasury name works`() {
    val treasury = testing.treasury("bz_static_treasury");
    println(treasury.address);
}

get fun `test bz format1 treasury name not a cell`() {
    val treasuryName = format("bz_dynamic_{}", 1);
    val treasury = testing.treasury(treasuryName);
    println(treasury.address);
}
"#,
        "integration/snapshots/test-runner/dynamic_format1_strings_break_store_string_and_treasury_bug/format1_result_breaks_treasury_name_bug.stdout.txt",
    );
}
