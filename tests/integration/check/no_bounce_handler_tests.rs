use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E007";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_no_bounce_handler() {
    run_simple_test(
        "no_bounce_handler",
        r#"
            fun sendReply(dest: address) {
                val reply = createMessage({
                    bounce: BounceMode.Only256BitsOfBody,
                    value: ton("0.1"),
                    dest,
                });
                reply.send(SEND_MODE_REGULAR);
            }

            fun onInternalMessage(in: InMessage) {
                sendReply(in.senderAddress);
            }
        "#,
        function_name!(),
    );
}
