use crate::integration::check::run_rule_test;
use function_name::named;

const RULE_CODE: &str = "E029";

fn run_simple_test(group: &str, content: &str, name: &str) {
    run_rule_test(group, RULE_CODE, content, name);
}

#[test]
#[named]
fn test_check_incoming_messages_duplicate_opcode_reports_inline_union() {
    run_simple_test(
        "incoming_messages_duplicate_opcode",
        r#"
            struct (0x1001) IncreaseCounter {
                value: int
            }

            struct (0x1001) DecreaseCounter {
                value: int
            }

            contract Counter {
                incomingMessages: IncreaseCounter | DecreaseCounter,
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_incoming_messages_duplicate_opcode_reports_alias_union() {
    run_simple_test(
        "incoming_messages_duplicate_opcode",
        r#"
            struct (0x1001) IncreaseCounter {
                value: int
            }

            struct (0x1001) DecreaseCounter {
                value: int
            }

            type Incoming = IncreaseCounter | DecreaseCounter;

            contract Counter {
                incomingMessages: Incoming,
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_incoming_messages_duplicate_opcode_skips_unique_opcodes() {
    run_simple_test(
        "incoming_messages_duplicate_opcode",
        r#"
            struct (0x1001) IncreaseCounter {
                value: int
            }

            struct (0x1002) DecreaseCounter {
                value: int
            }

            contract Counter {
                incomingMessages: IncreaseCounter | DecreaseCounter,
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_incoming_messages_duplicate_opcode_skips_same_value_with_different_width() {
    run_simple_test(
        "incoming_messages_duplicate_opcode",
        r#"
            struct (0x001) IncreaseCounter {
                value: int
            }

            struct (0x01) DecreaseCounter {
                value: int
            }

            contract Counter {
                incomingMessages: IncreaseCounter | DecreaseCounter,
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_incoming_messages_duplicate_opcode_reports_hex_and_binary_same_width() {
    run_simple_test(
        "incoming_messages_duplicate_opcode",
        r#"
            struct (0x01) IncreaseCounter {
                value: int
            }

            struct (0b00000001) DecreaseCounter {
                value: int
            }

            contract Counter {
                incomingMessages: IncreaseCounter | DecreaseCounter,
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_incoming_messages_duplicate_opcode_skips_hex_and_binary_different_width() {
    run_simple_test(
        "incoming_messages_duplicate_opcode",
        r#"
            struct (0x01) IncreaseCounter {
                value: int
            }

            struct (0b1) DecreaseCounter {
                value: int
            }

            contract Counter {
                incomingMessages: IncreaseCounter | DecreaseCounter,
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_incoming_messages_duplicate_opcode_reports_multiline_inline_union_with_many_messages()
{
    run_simple_test(
        "incoming_messages_duplicate_opcode",
        r#"
            struct (0x1001) IncreaseCounter {
                value: int
            }

            struct (0x1002) DecreaseCounter {
                value: int
            }

            struct (0x1001) NotifyCounter {
                value: int
            }

            struct (0x1003) ResetCounter {
                value: int
            }

            contract Counter {
                incomingMessages:
                    IncreaseCounter
                    | DecreaseCounter
                    | NotifyCounter
                    | ResetCounter,
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_incoming_messages_duplicate_opcode_reports_alias_union_with_many_messages() {
    run_simple_test(
        "incoming_messages_duplicate_opcode",
        r#"
            struct (0x2001) IncreaseCounter {
                value: int
            }

            struct (0x2002) DecreaseCounter {
                value: int
            }

            struct (0x2002) NotifyCounter {
                value: int
            }

            struct (0x2003) ResetCounter {
                value: int
            }

            type Incoming =
                IncreaseCounter
                | DecreaseCounter
                | NotifyCounter
                | ResetCounter;

            contract Counter {
                incomingMessages: Incoming,
            }
        "#,
        function_name!(),
    )
}
