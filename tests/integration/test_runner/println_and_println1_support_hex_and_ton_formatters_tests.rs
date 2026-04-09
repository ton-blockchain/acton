use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const TEST_IMPORTS: &str = r#"
import "../../lib/io"
"#;

fn run_stdlib_io_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let test_code = format!(
        r"
            {TEST_IMPORTS}

            {test_body}
        "
    );

    ProjectBuilder::new(project_name)
        .test_file("stdlib_io", &test_code)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn println_and_println1_support_hex_and_ton_formatters() {
    run_stdlib_io_case(
        "v-stdlib-println-and-println1-formatters",
        r#"
        get fun `test-println-and-println1-formatters`() {
            println(17);
            println("hex={:x}", 48879);
            println("ton={:ton}", 1000000000);
            println("plain={}", "ok");
        }
        "#,
        "integration/snapshots/test-runner/println_and_println1_support_hex_and_ton_formatters/println_and_println1_support_hex_and_ton_formatters.stdout.txt",
    );
}

#[test]
fn println2_to_println5_support_multi_argument_formatters() {
    run_stdlib_io_case(
        "v-stdlib-println2-to-println5-formatters",
        r#"
        get fun `test-println2-to-println5-formatters`() {
            println("{} + {}", "left", "right");
            println("hex={:x} ton={:ton} label={}", 255, 2500000000, "ok");
            println("{} {} {} {}", "a", "b", "c", "d");
            println("{} {} {} {} {}", 1, 2, 3, 4, 5);
            println("hello", "world");
            println("str", 1, 2);
            println("value {}!", 42, 100);
            println(1, 2);
            println("broken {", 1, 2);
        }
        "#,
        "integration/snapshots/test-runner/println_and_println1_support_hex_and_ton_formatters/println2_to_println5_support_multi_argument_formatters.stdout.txt",
    );
}

#[test]
fn eprintln_reports_into_test_stderr_block() {
    run_stdlib_io_case(
        "v-stdlib-eprintln-stderr-path",
        r#"
        get fun `test-eprintln-stderr-path`() {
            println("stdout-before");
            eprintln("stderr-line-1");
            eprintln("stderr-line-2");
            println("stdout-after");
        }
        "#,
        "integration/snapshots/test-runner/println_and_println1_support_hex_and_ton_formatters/eprintln_reports_into_test_stderr_block.stdout.txt",
    );
}
