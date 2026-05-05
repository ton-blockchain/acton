use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E016";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_dangerous_send_mode_safety_comment_missing_for_all_balance() {
    run_simple_test(
        "dangerous_send_mode_safety_comment",
        r#"
            fun onInternalMessage(in: InMessage) {
                val outMsg = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                outMsg.send(SEND_MODE_CARRY_ALL_BALANCE);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_dangerous_send_mode_safety_comment_missing_for_destroy() {
    run_simple_test(
        "dangerous_send_mode_safety_comment",
        r#"
            fun onInternalMessage(in: InMessage) {
                val outMsg = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                outMsg.send(SEND_MODE_DESTROY | SEND_MODE_CARRY_ALL_BALANCE);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_dangerous_send_mode_safety_comment_missing_for_numeric_literal() {
    run_simple_test(
        "dangerous_send_mode_safety_comment",
        r#"
            fun onInternalMessage(in: InMessage) {
                val outMsg = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                outMsg.send(128);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_dangerous_send_mode_safety_comment_with_standalone_comment() {
    run_simple_test(
        "dangerous_send_mode_safety_comment",
        r#"
            fun onInternalMessage(in: InMessage) {
                val outMsg = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                // SAFETY: this call is used only for controlled migration flow.
                outMsg.send(SEND_MODE_CARRY_ALL_BALANCE);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_dangerous_send_mode_safety_comment_with_safety_word_comment() {
    run_simple_test(
        "dangerous_send_mode_safety_comment",
        r#"
            fun onInternalMessage(in: InMessage) {
                val outMsg = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                // # Safety: this path is guarded by explicit admin checks.
                outMsg.send(SEND_MODE_DESTROY + SEND_MODE_CARRY_ALL_BALANCE);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_dangerous_send_mode_safety_comment_regular_mode_is_ignored() {
    run_simple_test(
        "dangerous_send_mode_safety_comment",
        r#"
            fun onInternalMessage(in: InMessage) {
                val outMsg = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                outMsg.send(SEND_MODE_REGULAR);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_dangerous_send_mode_safety_comment_send_raw_message_is_ignored() {
    run_simple_test(
        "dangerous_send_mode_safety_comment",
        r"
            fun onInternalMessage(_: InMessage) {
                sendRawMessage(beginCell().endCell(), SEND_MODE_CARRY_ALL_BALANCE);
            }
        ",
        function_name!(),
    );
}
