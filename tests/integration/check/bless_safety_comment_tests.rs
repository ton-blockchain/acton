use crate::integration::check::{run_rule_check_test_with_files, run_rule_test};
use function_name::named;

const RULE_CODE: &str = "E023";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

fn run_check_test_with_files(group: &str, main_content: &str, files: &[(&str, &str)], name: &str) {
    run_rule_check_test_with_files(group, RULE_CODE, main_content, files, name);
}

#[test]
#[named]
fn test_check_bless_safety_comment_missing_for_transform_slice_to_continuation() {
    run_simple_test(
        "bless_safety_comment",
        r#"
            // SAFETY: wrapper is used only with trusted code slices.
            fun transformSliceToContinuation(code: slice): continuation asm "BLESS";

            fun convert(code: slice): continuation {
                return transformSliceToContinuation(code);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_bless_safety_comment_with_standalone_comment() {
    run_simple_test(
        "bless_safety_comment",
        r#"
            import "@stdlib/tvm-lowlevel"

            fun convert(code: slice): continuation {
                // SAFETY: code comes from trusted on-chain storage and has validated layout.
                return transformSliceToContinuation(code);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_bless_safety_comment_missing_for_stdlib_bless_asm_function_call() {
    run_simple_test(
        "bless_safety_comment",
        r#"
            import "@stdlib/tvm-lowlevel"

            fun convert(code: slice): continuation {
                return transformSliceToContinuation(code);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_bless_safety_comment_missing_for_user_defined_bless_asm_function_call() {
    run_simple_test(
        "bless_safety_comment",
        r#"
            // SAFETY: wrapper is used only with trusted code slices.
            fun toCont(code: slice): continuation
                asm "BLESS";

            fun convert(code: slice): continuation {
                return toCont(code);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_bless_safety_comment_non_bless_asm_function_call_is_ignored() {
    run_simple_test(
        "bless_safety_comment",
        r#"
            // SAFETY: wrapper reads a fixed-width integer from trusted data.
            fun read32(code: slice): int asm "32 LDU";

            fun convert(code: slice): int {
                return read32(code);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_bless_safety_comment_missing_when_bless_is_not_first_asm_instruction() {
    run_simple_test(
        "bless_safety_comment",
        r#"
            // SAFETY: wrapper is used only with trusted code slices.
            fun toCont(code: slice): continuation
                asm "NOP"
                    "BLESS";

            fun convert(code: slice): continuation {
                return toCont(code);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_bless_safety_comment_missing_when_bless_is_in_multi_opcode_string() {
    run_simple_test(
        "bless_safety_comment",
        r#"
            // SAFETY: wrapper is used only with trusted code slices.
            fun toCont(code: slice): continuation asm """
                NOP
                SWAP
                BLESS
            """;

            fun convert(code: slice): continuation {
                return toCont(code);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_bless_safety_comment_cross_file_shows_help_diagnostic_on_bless_source() {
    run_check_test_with_files(
        "bless_safety_comment",
        r#"
            import "./helpers.tolk";

            fun convert(code: slice): continuation {
                return toCont(code);
            }
        "#,
        &[(
            "contracts/helpers",
            r#"
                // SAFETY: wrapper is used only with trusted code slices.
                fun toCont(code: slice): continuation asm "BLESS";
            "#,
        )],
        function_name!(),
    );
}
