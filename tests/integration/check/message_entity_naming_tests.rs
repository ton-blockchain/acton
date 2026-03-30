use crate::integration::check::run_rule_test;
use function_name::named;

fn run_message_should_be_named_test(content: &str, name: &str) {
    run_rule_test("message_entity_naming", "E011", content, name);
}

fn run_create_message_inline_send_test(content: &str, name: &str) {
    run_rule_test("message_entity_naming", "E012", content, name);
}

#[test]
#[named]
fn test_check_message_should_be_named() {
    run_message_should_be_named_test(
        r#"
            fun onInternalMessage(in: InMessage) {
                val msg = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                msg.send(SEND_MODE_REGULAR);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_message_should_be_named_skips_proper_name() {
    run_message_should_be_named_test(
        r#"
            fun onInternalMessage(in: InMessage) {
                val deployMessage = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                });
                deployMessage.send(SEND_MODE_REGULAR);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_create_message_inline_send() {
    run_create_message_inline_send_test(
        r#"
            fun onInternalMessage(in: InMessage) {
                createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest: in.senderAddress,
                }).send(SEND_MODE_REGULAR);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_create_message_inline_send_skips_other_factories() {
    run_create_message_inline_send_test(
        r"
            fun onInternalMessage(_: InMessage) {
                createExternalLogMessage({
                    dest: createAddressNone(),
                }).send(SEND_MODE_REGULAR);
            }
        ",
        function_name!(),
    );
}
