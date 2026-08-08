use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E029";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_create_message_body_to_cell_on_object_literal() {
    run_simple_test(
        "create_message_body_to_cell",
        r#"
            struct Transfer {
                amount: uint32
            }

            fun send(dest: address) {
                val msg = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest,
                    body: Transfer { amount: 1 }.toCell(),
                });
                msg.send(SEND_MODE_REGULAR);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_create_message_body_to_cell_on_multiline_object_literal() {
    run_simple_test(
        "create_message_body_to_cell",
        r#"
            struct Transfer {
                amount: uint32,
                queryId: uint64
            }

            fun send(dest: address) {
                val msg = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest,
                    body: Transfer {
                        amount: 1,
                        queryId: 2,
                    }.toCell(),
                });
                msg.send(SEND_MODE_REGULAR);
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_create_message_body_to_cell_skips_raw_cell_body() {
    run_simple_test(
        "create_message_body_to_cell",
        r#"
            fun send(dest: address) {
                val msg = createMessage({
                    bounce: false,
                    value: ton("0.1"),
                    dest,
                    body: beginCell().storeUint(0xDEADBEEF, 32).endCell(),
                });
                msg.send(SEND_MODE_REGULAR);
            }
        "#,
        function_name!(),
    );
}
