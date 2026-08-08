use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use acton_config::color::ColorMode;

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
        get fun `test println and println1 formatters`() {
            println(17);
            println("hex={:x}", 48879);
            println("grams={:grams}", 1000000000);
            println("plain={}", "ok");
        }
        "#,
        "integration/snapshots/test-runner/println_and_println1_support_hex_and_ton_formatters/println_and_println1_support_hex_and_ton_formatters.stdout.txt",
    );
}

#[test]
fn println_supports_constant_width_and_padding() {
    run_stdlib_io_case(
        "v-stdlib-println-width-padding",
        r#"
        get fun `test println width padding`() {
            println("flag={{:5}} value=|{:5}|", "hi");
            println("flag={{:>5}} value=|{:>5}|", "hi");
            println("flag={{:_>5}} value=|{:_>5}|", "hi");
            println("flag={{:>>5}} value=|{:>>5}|", "hi");
            println("flag={{:.^7}} value=|{:.^7}|", "hi");
            println("flag={{:05}} value=|{:05}|", 42);
            println("flag={{:06x}} value=|{:06x}|", 255);
            println("flag={{:X}} value=|{:X}|", 255);
            println("flag={{:08X}} value=|{:08X}|", 255);
            println("flag={{:b}} value=|{:b}|", 10);
            println("flag={{:B}} value=|{:B}|", 10);
            println("flag={{:08B}} value=|{:08B}|", 10);
            println("flag={{:0>6x}} value=|{:0>6x}|", -255);
            println("flag={{:>12ton}} value=|{:>12ton}|", 1500000000);
            println("flag={{:0>8:x}} value=|{:0>8:x}|", 255);
            println("flag={{:*>6}} value=|{:*>6}|", "ok", "after");
            println("missing {{:_>6}} -> {:_>6}");
        }
        "#,
        "integration/snapshots/test-runner/println_and_println1_support_hex_and_ton_formatters/println_supports_constant_width_and_padding.stdout.txt",
    );
}

#[test]
fn println_width_padding_uses_visible_length_for_colored_default_values() {
    let script_body = r#"
        println("flag={{:>6}} value=|{:>6}|", 42);
        println("flag={{:06}} value=|{:06}|", 42);
        println("flag={{:06}} value=|{:06}|", -42);
        println("flag={{:<6}} value=|{:<6}|", true);
        println("flag={{:^6}} value=|{:^6}|", false);
    "#;
    let script_code = format!(
        r"
        {TEST_IMPORTS}

        fun main() {{
            {script_body}
        }}
        "
    );

    ProjectBuilder::new("v-stdlib-println-colored-width-padding")
        .script_file("stdlib_io", &script_code)
        .build()
        .acton()
        .script("scripts/stdlib_io.tolk")
        .keep_color_env()
        .color_mode(ColorMode::Always)
        .run()
        .success()
        .assert_stdout_svg_snapshot_matches(
            "integration/snapshots/test-runner/println_and_println1_support_hex_and_ton_formatters/println_width_padding_uses_visible_length_for_colored_default_values.stdout.svg",
        );
}

#[test]
fn println_supports_cell_tree_formatter() {
    run_stdlib_io_case(
        "v-stdlib-println-cell-tree-formatter",
        r#"
        struct TreePayload {
            value: uint16,
            child: Cell<uint8>,
        }

        get fun `test println cell tree formatter`() {
            val child = beginCell().storeUint(0xAB, 8).endCell();
            val typedChild = child as Cell<uint8>;
            val root = beginCell()
                .storeUint(0xCAFE, 16)
                .storeRef(child)
                .endCell();
            val typedRoot: Cell<TreePayload> = TreePayload {
                value: 0xCAFE,
                child: typedChild,
            }.toCell();

            println("cell:\n{:cell-tree}", root);
            println("typed:\n{:cell-tree}", typedRoot);
            println("missing {:cell-tree}");
        }
        "#,
        "integration/snapshots/test-runner/println_and_println1_support_hex_and_ton_formatters/println_supports_cell_tree_formatter.stdout.txt",
    );
}

#[test]
fn println2_to_println5_support_multi_argument_formatters() {
    run_stdlib_io_case(
        "v-stdlib-println2-to-println5-formatters",
        r#"
        get fun `test println2 to println5 formatters`() {
            println("{} + {}", "left", "right");
            println("hex={:x} grams={:grams} label={}", 255, 2500000000, "ok");
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
        get fun `test eprintln stderr path`() {
            println("stdout-before");
            eprintln("stderr-line-1");
            eprintln("stderr-line-2");
            println("stdout-after");
        }
        "#,
        "integration/snapshots/test-runner/println_and_println1_support_hex_and_ton_formatters/eprintln_reports_into_test_stderr_block.stdout.txt",
    );
}
