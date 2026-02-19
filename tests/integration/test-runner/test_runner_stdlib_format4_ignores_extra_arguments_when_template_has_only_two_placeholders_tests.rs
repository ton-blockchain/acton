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

fn run_fmt_failure(project_name: &str, test_body: &str, snapshot_path: &str, contains: &[&str]) {
    let source = wrap_fmt_test_source(test_body);
    let output = ProjectBuilder::new(project_name)
        .test_file("fmt_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .failure();

    output.assert_failed(1);
    for needle in contains {
        output.assert_contains(needle);
    }
    output.assert_snapshot_matches(snapshot_path);
}

#[test]
fn format4_ignores_extra_arguments_when_template_has_only_two_placeholders() {
    run_fmt_success(
        "eb-stdlib-format4-extra-args-ignored",
        r#"
get fun `test-eb-stdlib-format4-extra-args-ignored`() {
    val rendered = format4("{}: {:ton}", "alpha", 2500000000, "unused", 255);
    expect(rendered).toEqual("alpha: 2.5 TON");
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders_tests/format4_ignores_extra_arguments_when_template_has_only_two_placeholders.stdout.txt",
    );
}

#[test]
fn format4_leaves_unmatched_placeholder_when_template_has_five_slots() {
    run_fmt_success(
        "eb-stdlib-format4-missing-placeholder-slot",
        r#"
get fun `test-eb-stdlib-format4-missing-placeholder-slot`() {
    val rendered = format4("a={} b={} c={} d={} e={}", 1, 2, 3, 4);
    expect(rendered).toEqual("a=1 b=2 c=3 d=4 e={}");
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders_tests/format4_leaves_unmatched_placeholder_when_template_has_five_slots.stdout.txt",
    );
}

#[test]
fn format1_rejects_unknown_modifier_in_placeholder() {
    run_fmt_failure(
        "eb-stdlib-format1-unknown-modifier",
        r#"
get fun `test-eb-stdlib-format1-unknown-modifier`() {
    format1("value={:hex}", 255);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders_tests/format1_rejects_unknown_modifier_in_placeholder.stdout.txt",
        &["Invalid format string", "unknown format modifier 'hex'"],
    );
}

#[test]
fn format1_rejects_empty_modifier_in_placeholder() {
    run_fmt_failure(
        "eb-stdlib-format1-empty-modifier",
        r#"
get fun `test-eb-stdlib-format1-empty-modifier`() {
    format1("value={:}", 255);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders_tests/format1_rejects_empty_modifier_in_placeholder.stdout.txt",
        &["Invalid format string", "unknown format modifier ''"],
    );
}

#[test]
fn format1_rejects_unsupported_placeholder_payload() {
    run_fmt_failure(
        "eb-stdlib-format1-unsupported-placeholder-payload",
        r#"
get fun `test-eb-stdlib-format1-unsupported-placeholder-payload`() {
    format1("value={name}", 255);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders_tests/format1_rejects_unsupported_placeholder_payload.stdout.txt",
        &["Invalid format string", "unsupported placeholder {name}"],
    );
}

#[test]
fn format1_rejects_unclosed_open_brace() {
    run_fmt_failure(
        "eb-stdlib-format1-unclosed-open-brace",
        r#"
get fun `test-eb-stdlib-format1-unclosed-open-brace`() {
    format1("value={", 255);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders_tests/format1_rejects_unclosed_open_brace.stdout.txt",
        &["Invalid format string", "unclosed '{' placeholder"],
    );
}

#[test]
fn format1_rejects_unmatched_closing_brace() {
    run_fmt_failure(
        "eb-stdlib-format1-unmatched-closing-brace",
        r#"
get fun `test-eb-stdlib-format1-unmatched-closing-brace`() {
    format1("value=}", 255);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders_tests/format1_rejects_unmatched_closing_brace.stdout.txt",
        &["Invalid format string", "unmatched '}'"],
    );
}

#[test]
fn format2_escaped_braces_are_treated_as_literals() {
    run_fmt_success(
        "eb-stdlib-format2-escaped-braces-literals",
        r#"
get fun `test-eb-stdlib-format2-escaped-braces-literals`() {
    val rendered = format2("literal={{}} value={}", 42, 999);
    expect(rendered).toEqual("literal={} value=42");
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders_tests/format2_escaped_braces_are_treated_as_literals.stdout.txt",
    );
}

#[test]
fn format1_supports_utf8_literals_with_plain_placeholder() {
    run_fmt_success(
        "eb-stdlib-format1-utf8-literal-placeholder",
        r#"
get fun `test-eb-stdlib-format1-utf8-literal-placeholder`() {
    val rendered = format1("привет🙂 {}", 7);
    expect(rendered).toEqual("привет🙂 7");
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders_tests/format1_supports_utf8_literals_with_plain_placeholder.stdout.txt",
    );
}

#[test]
fn format1_escaped_braces_without_placeholders_render_as_literals() {
    run_fmt_success(
        "eb-stdlib-format1-escaped-only-literals",
        r#"
get fun `test-eb-stdlib-format1-escaped-only-literals`() {
    val rendered = format1("{{}} and }}{{", 42);
    expect(rendered).toEqual("{} and }{");
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders_tests/format1_escaped_braces_without_placeholders_render_as_literals.stdout.txt",
    );
}

#[test]
fn format1_rejects_invalid_modifier_payload() {
    run_fmt_failure(
        "eb-stdlib-format1-invalid-modifier-payload",
        r#"
get fun `test-eb-stdlib-format1-invalid-modifier-payload`() {
    format1("value={:x:}", 255);
}
"#,
        "integration/snapshots/test-runner/test_runner_stdlib_format4_ignores_extra_arguments_when_template_has_only_two_placeholders_tests/format1_rejects_invalid_modifier_payload.stdout.txt",
        &["Invalid format string", "unknown format modifier 'x:'"],
    );
}
