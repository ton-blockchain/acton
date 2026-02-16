use crate::integration::check::run_simple_test;
use function_name::named;

#[test]
#[named]
fn test_check_message_should_be_named() {
    run_simple_test(
        "message_entity_naming",
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
    )
}

#[test]
#[named]
fn test_check_message_should_be_named_skips_proper_name() {
    run_simple_test(
        "message_entity_naming",
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
    )
}

#[test]
#[named]
fn test_check_create_message_inline_send() {
    run_simple_test(
        "message_entity_naming",
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
    )
}

#[test]
#[named]
fn test_check_create_message_inline_send_skips_other_factories() {
    run_simple_test(
        "message_entity_naming",
        r#"
            fun onInternalMessage(_: InMessage) {
                createExternalLogMessage({
                    dest: createAddressNone(),
                }).send(SEND_MODE_REGULAR);
            }
        "#,
        function_name!(),
    )
}
