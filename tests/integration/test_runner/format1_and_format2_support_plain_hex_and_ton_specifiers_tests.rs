use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const FMT_TEST_IMPORTS: &str = r#"
import "../../lib/fmt"
import "../../lib/testing/expect"
"#;

fn wrap_fmt_test_source(test_body: &str) -> String {
    format!("{FMT_TEST_IMPORTS}\n{test_body}\n")
}

fn run_fmt_success(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = wrap_fmt_test_source(test_body);
    ProjectBuilder::new(project_name)
        .test_file("fmt_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn format1_and_format2_support_plain_hex_and_ton_specifiers() {
    run_fmt_success(
        "y-stdlib-format1-format2-specifiers",
        r#"
get fun `test-y-stdlib-format1-format2-specifiers`() {
    val plain = format("hello {}", "world");
    expect(plain).toEqual("hello world");

    val hex = format("0x{:x}", 255);
    expect(hex).toEqual("0xff");

    val ton = format("{} {:ton}", "balance", 1500000000);
    expect(ton).toEqual("balance 1.5 TON");
}
"#,
        "integration/snapshots/test-runner/format1_and_format2_support_plain_hex_and_ton_specifiers/format1_and_format2_support_plain_hex_and_ton_specifiers.stdout.txt",
    );
}

#[test]
fn format3_and_format4_support_mixed_plain_hex_and_ton() {
    run_fmt_success(
        "y-stdlib-format3-format4-mixed-specifiers",
        r#"
get fun `test-y-stdlib-format3-format4-mixed-specifiers`() {
    val formatted3 = format("hex={:x} ton={:ton} label={}", 255, 2500000000, "ok");
    expect(formatted3).toEqual("hex=ff ton=2.5 TON label=ok");

    val formatted4 = format("a={} b={:x} c={:ton} d={}", "left", 16, 1230000000, "right");
    expect(formatted4).toEqual("a=left b=10 c=1.23 TON d=right");
}
"#,
        "integration/snapshots/test-runner/format1_and_format2_support_plain_hex_and_ton_specifiers/format3_and_format4_support_mixed_plain_hex_and_ton.stdout.txt",
    );
}

#[test]
fn format5_should_respect_placeholder_order_for_plain_hex_and_ton_bug() {
    run_fmt_success(
        "y-stdlib-format5-placeholder-order-bug",
        r#"
get fun `test-y-stdlib-format5-placeholder-order-bug`() {
    val rendered = format("{} | {:x} | {:ton} | {} | {}", 255, 16, 1500000000, "left", "right");
    expect(rendered).toEqual("255 | 10 | 1.5 TON | left | right");
}
"#,
        "integration/snapshots/test-runner/format1_and_format2_support_plain_hex_and_ton_specifiers/format5_should_respect_placeholder_order_for_plain_hex_and_ton_bug.stdout.txt",
    );
}
