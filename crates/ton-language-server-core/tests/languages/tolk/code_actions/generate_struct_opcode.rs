use super::support::CodeActionTest;
use expect_test::expect;

#[test]
fn generates_opcode_for_plain_and_generic_structs() {
    // The opcode is the CRC32 of the struct name.
    CodeActionTest::new(
        "
            struct <caret>Message {
                value: int;
            }
        ",
    )
    .check_applied(
        "Generate 32-bit opcode",
        expect![[r"
            struct (0x790009e3) Message {
                value: int;
            }"]],
    );

    // Type parameters do not participate in the opcode calculation.
    CodeActionTest::new(
        "
            struct Generic<caret>Box<T> {
                value: T;
            }
        ",
    )
    .check_applied(
        "Generate 32-bit opcode",
        expect![[r"
            struct (0xa109575c) GenericBox<T> {
                value: T;
            }"]],
    );
}

#[test]
fn is_unavailable_for_prefixed_structs_and_fields() {
    // An explicit prefix is never overwritten.
    CodeActionTest::new("struct (0xb942c196) <caret>Transfer { value: int }")
        .check_titles(expect!["<none>"]);

    // The action is scoped to the declaration header, not its fields.
    CodeActionTest::new("struct Transfer { <caret>value: int }").check_titles(expect!["<none>"]);
}
