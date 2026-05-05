use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E026";

fn run_simple_test(content: &str, name: &str) {
    run_rule_test("unused_expression", RULE_CODE, content, name);
}

macro_rules! unused_expression_test {
    ($name:ident, $content:expr) => {
        #[test]
        #[named]
        fn $name() {
            run_simple_test($content, function_name!());
        }
    };
}

unused_expression_test!(
    test_check_unused_expression_skips_var_decl_lhs,
    r"
        fun main() {
            val a: int;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_skips_assignment_statement,
    r"
        fun update(mutate a: int) {
            a = 1;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_skips_set_assignment_statement,
    r"
        fun update(mutate a: int) {
            a += 1;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_skips_ternary_statement,
    r"
        fun main(flag: bool) {
            flag ? 1 : 2;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_skips_match_statement,
    r"
        fun main(a: int) {
            match (a) {
                1 => 2,
                else => 3,
            };
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_skips_call_statement,
    r"
        fun logValue(a: int) {
            debug.print(a);
        }

        fun main(a: int) {
            logValue(a);
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_paren_statement,
    r"
        fun main(a: int) {
            (a + 1);
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_comparison_statement,
    r"
        struct Storage {
            grams: coins
        }

        fun main(in: InMessage) {
            val storage = Storage { grams: 0 };
            storage.grams != in.valueCoins;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_arithmetic_statement,
    r"
        fun main(a: int) {
            a + 1;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_unary_statement,
    r"
        fun main(a: int) {
            -a;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_as_cast_statement,
    r"
        fun main(a: int) {
            a as int;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_is_type_statement,
    r"
        fun main(a: int) {
            a is int;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_not_null_statement,
    r"
        fun main(a: int?) {
            a!;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_dot_access_statement,
    r"
        struct Point {
            x: int
        }

        fun main() {
            val point = Point { x: 1 };
            point.x;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_instantiation_statement,
    r"
        fun identity<T>(x: T): T {
            return x;
        }

        fun main() {
            identity<int>;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_object_literal_statement,
    r"
        struct Point {
            x: int
        }

        fun main() {
            Point { x: 1 };
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_tensor_statement,
    r"
        fun main() {
            (1, 2);
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_tuple_statement,
    r"
        fun main() {
            [1, 2];
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_lambda_statement,
    r"
        fun main() {
            fun() {};
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_number_literal_statement,
    r"
        fun main() {
            1;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_string_literal_statement,
    r#"
        fun main() {
            "hello";
        }
    "#
);

unused_expression_test!(
    test_check_unused_expression_reports_bool_literal_statement,
    r"
        fun main() {
            true;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_null_literal_statement,
    r"
        fun main() {
            null;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_ident_statement,
    r"
        fun main(a: int) {
            a;
        }
    "
);

unused_expression_test!(
    test_check_unused_expression_reports_underscore_statement,
    r"
        fun main() {
            _;
        }
    "
);
