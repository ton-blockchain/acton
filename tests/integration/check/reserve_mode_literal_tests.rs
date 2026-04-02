use crate::integration::check::{run_rule_fix_test, run_rule_test};
use function_name::named;

const RULE_CODE: &str = "E020";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

fn run_fix_test(before: &str, after: &str, name: &str) {
    run_rule_fix_test(RULE_CODE, before, after, name);
}

#[test]
#[named]
fn test_check_reserve_mode_literal_single_number() {
    run_simple_test(
        "reserve_mode_literal",
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), 3);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_reserve_mode_literal_addition() {
    run_simple_test(
        "reserve_mode_literal",
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), 1 + 2);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_reserve_mode_literal_unmappable_single_number() {
    run_simple_test(
        "reserve_mode_literal",
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), 32);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_reserve_mode_literal_non_additive_expression() {
    run_simple_test(
        "reserve_mode_literal",
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), 1 | 2);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_reserve_mode_literal_constants_only() {
    run_simple_test(
        "reserve_mode_literal",
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(
                    ton("0.1"),
                    RESERVE_MODE_ALL_BUT_AMOUNT + RESERVE_MODE_AT_MOST
                );
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_reserve_mode_literal_reserve_extra_currencies() {
    run_simple_test(
        "reserve_mode_literal",
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveExtraCurrenciesOnBalance(ton("0.1"), null, 3);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_reserve_mode_literal_single_number() {
    run_fix_test(
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), 3);
            }
        "#,
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), RESERVE_MODE_ALL_BUT_AMOUNT + RESERVE_MODE_AT_MOST);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_reserve_mode_literal_zero() {
    run_fix_test(
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), 0);
            }
        "#,
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), RESERVE_MODE_EXACT_AMOUNT);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_reserve_mode_literal_addition() {
    run_fix_test(
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), 1 + 2);
            }
        "#,
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), RESERVE_MODE_ALL_BUT_AMOUNT + RESERVE_MODE_AT_MOST);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_reserve_mode_literal_unmappable_single_number() {
    run_fix_test(
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), 32);
            }
        "#,
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), 32);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_reserve_mode_literal_non_additive_expression() {
    run_fix_test(
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), 1 | 2);
            }
        "#,
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), 1 | 2);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_reserve_mode_literal_mixed_constant_and_literal() {
    run_fix_test(
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), RESERVE_MODE_AT_MOST + 1);
            }
        "#,
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveToncoinsOnBalance(ton("0.1"), RESERVE_MODE_AT_MOST + RESERVE_MODE_ALL_BUT_AMOUNT);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_reserve_mode_literal_reserve_extra_currencies() {
    run_fix_test(
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveExtraCurrenciesOnBalance(ton("0.1"), null, 3);
            }
        "#,
        r#"
            fun onInternalMessage(_: InMessage) {
                reserveExtraCurrenciesOnBalance(ton("0.1"), null, RESERVE_MODE_ALL_BUT_AMOUNT + RESERVE_MODE_AT_MOST);
            }
        "#,
        function_name!(),
    );
}
