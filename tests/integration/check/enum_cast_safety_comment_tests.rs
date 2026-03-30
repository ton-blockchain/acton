use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E030";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_enum_cast_safety_comment_missing_for_non_literal_cast() {
    run_simple_test(
        "enum_cast_safety_comment",
        r"
            enum Op {
                Add = 0,
                Sub = 1,
            }

            fun parse(v: int): Op {
                return v as Op;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_enum_cast_safety_comment_with_standalone_comment() {
    run_simple_test(
        "enum_cast_safety_comment",
        r"
            enum Op {
                Add = 0,
                Sub = 1,
            }

            fun parse(v: int): Op {
                // SAFETY: input is validated and guaranteed to be one of enum variants.
                return v as Op;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_enum_cast_safety_comment_numeric_literal_is_ignored() {
    run_simple_test(
        "enum_cast_safety_comment",
        r"
            enum Op {
                Add = 0,
                Sub = 1,
            }

            fun parse(): Op {
                return 1 as Op;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_enum_cast_safety_comment_signed_numeric_literal_is_ignored() {
    run_simple_test(
        "enum_cast_safety_comment",
        r"
            enum Op {
                Add = -1,
                Sub = 1,
            }

            fun parse(): Op {
                return (-1) as Op;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_enum_cast_safety_comment_non_enum_cast_is_ignored() {
    run_simple_test(
        "enum_cast_safety_comment",
        r"
            fun parse(v: int): int {
                return v as int;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_enum_cast_safety_comment_enum_alias_cast_is_reported() {
    run_simple_test(
        "enum_cast_safety_comment",
        r"
            enum Op {
                Add = 0,
                Sub = 1,
            }
            type ParsedOp = Op;

            fun parse(v: int): ParsedOp {
                return v as ParsedOp;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_enum_cast_safety_comment_blank_line_after_comment_is_reported() {
    run_simple_test(
        "enum_cast_safety_comment",
        r"
            enum Op {
                Add = 0,
                Sub = 1,
            }

            fun parse(v: int): Op {
                // SAFETY: this line should not suppress because of blank line below.

                return v as Op;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_enum_cast_safety_comment_hash_safety_word_is_accepted() {
    run_simple_test(
        "enum_cast_safety_comment",
        r"
            enum Op {
                Add = 0,
                Sub = 1,
            }

            fun parse(v: int): Op {
                // # Safety: value is validated by caller.
                return v as Op;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_enum_cast_safety_comment_signed_non_literal_is_reported() {
    run_simple_test(
        "enum_cast_safety_comment",
        r"
            enum Op {
                Add = -1,
                Sub = 1,
            }

            fun parse(v: int): Op {
                return (-v) as Op;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_enum_cast_safety_comment_hex_numeric_literal_is_ignored() {
    run_simple_test(
        "enum_cast_safety_comment",
        r"
            enum Op {
                Add = 0,
                Sub = 0x10,
            }

            fun parse(): Op {
                return 0x10 as Op;
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_enum_cast_safety_comment_unary_plus_numeric_literal_is_ignored() {
    run_simple_test(
        "enum_cast_safety_comment",
        r"
            enum Op {
                Add = 0,
                Sub = 1,
            }

            fun parse(): Op {
                return (+1) as Op;
            }
        ",
        function_name!(),
    );
}
