use crate::integration::check::{run_rule_fix_test, run_rule_test};
use function_name::named;

const RULE_CODE: &str = "E016";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

fn run_fix_test(before: &str, after: &str, name: &str) {
    run_rule_fix_test(RULE_CODE, before, after, name);
}

#[test]
#[named]
fn test_check_send_mode_literal_single_number() {
    run_simple_test(
        "send_mode_literal",
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(3);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_send_mode_literal_addition() {
    run_simple_test(
        "send_mode_literal",
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(1 + 2);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_send_mode_literal_unmappable_single_number() {
    run_simple_test(
        "send_mode_literal",
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(4);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_send_mode_literal_non_additive_expression() {
    run_simple_test(
        "send_mode_literal",
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(1 | 2);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_send_mode_literal_addition_with_unmappable_literal() {
    run_simple_test(
        "send_mode_literal",
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(SEND_MODE_IGNORE_ERRORS + 4);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_send_mode_literal_constants_only() {
    run_simple_test(
        "send_mode_literal",
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(SEND_MODE_PAY_FEES_SEPARATELY + SEND_MODE_IGNORE_ERRORS);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_send_mode_literal_single_number() {
    run_fix_test(
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(3);
            }
        "#,
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(SEND_MODE_PAY_FEES_SEPARATELY + SEND_MODE_IGNORE_ERRORS);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_send_mode_literal_addition() {
    run_fix_test(
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(1 + 2);
            }
        "#,
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(SEND_MODE_PAY_FEES_SEPARATELY + SEND_MODE_IGNORE_ERRORS);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_send_mode_literal_unmappable_single_number() {
    run_fix_test(
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(4);
            }
        "#,
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(4);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_send_mode_literal_non_additive_expression() {
    run_fix_test(
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(1 | 2);
            }
        "#,
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(1 | 2);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_send_mode_literal_addition_with_unmappable_literal() {
    run_fix_test(
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(SEND_MODE_IGNORE_ERRORS + 4);
            }
        "#,
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(SEND_MODE_IGNORE_ERRORS + 4);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_send_mode_literal_mixed_constant_and_literal_left_const() {
    run_simple_test(
        "send_mode_literal",
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(SEND_MODE_IGNORE_ERRORS + 1);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_send_mode_literal_mixed_constant_and_literal_right_const() {
    run_simple_test(
        "send_mode_literal",
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(1 + SEND_MODE_IGNORE_ERRORS);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_send_mode_literal_mixed_constant_and_literal_left_const() {
    run_fix_test(
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(SEND_MODE_IGNORE_ERRORS + 1);
            }
        "#,
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(SEND_MODE_IGNORE_ERRORS + SEND_MODE_PAY_FEES_SEPARATELY);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_send_mode_literal_mixed_constant_and_literal_right_const() {
    run_fix_test(
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(1 + SEND_MODE_IGNORE_ERRORS);
            }
        "#,
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(SEND_MODE_PAY_FEES_SEPARATELY + SEND_MODE_IGNORE_ERRORS);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_send_mode_literal_send_raw_message() {
    run_simple_test(
        "send_mode_literal",
        r#"
            fun onInternalMessage(_: InMessage) {
                sendRawMessage(beginCell().endCell(), 3);
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_fix_send_mode_literal_send_raw_message() {
    run_fix_test(
        r#"
            fun onInternalMessage(_: InMessage) {
                sendRawMessage(beginCell().endCell(), 3);
            }
        "#,
        r#"
            fun onInternalMessage(_: InMessage) {
                sendRawMessage(beginCell().endCell(), SEND_MODE_PAY_FEES_SEPARATELY + SEND_MODE_IGNORE_ERRORS);
            }
        "#,
        function_name!(),
    )
}
