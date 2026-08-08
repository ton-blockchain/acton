use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E011";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_asm_safety_comment_missing_for_function() {
    run_simple_test(
        "asm_safety_comment",
        r#"
            fun readIntFromSlice(src: slice): int asm "LDI";
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_asm_safety_comment_missing_for_method() {
    run_simple_test(
        "asm_safety_comment",
        r#"
            struct Loader {}

            fun Loader.read(src: slice): int asm "LDI";
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_asm_safety_comment_with_standalone_comment() {
    run_simple_test(
        "asm_safety_comment",
        r#"
            // SAFETY: caller ensures that `src` contains enough bits.
            fun readIntFromSlice(src: slice): int asm "LDI";
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_asm_safety_comment_with_doc_comment() {
    run_simple_test(
        "asm_safety_comment",
        r#"
            /// # Safety
            /// Caller must provide a slice with enough bits for `LDI`.
            fun readIntFromSlice(src: slice): int asm "LDI";
        "#,
        function_name!(),
    );
}
