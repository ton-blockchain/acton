#![allow(clippy::needless_raw_string_hashes)]

use super::support::MarkedSource;
use expect_test::{Expect, expect};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{DocumentUri, LanguageService, LanguageServiceConfig};

const EXIT_CODES_URL: &str = "https://docs.ton.org/v3/documentation/tvm/tvm-exit-codes";

fn hover(source: &str) -> Option<String> {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    service
        .hover(&uri, marked.marker("caret").position)
        .expect("hover request should succeed")
        .map(|hover| hover.contents)
}

fn check_exit_code(code: &str, expect: Expect) {
    let contents = hover(&format!("fun main() {{ throw <caret>{code}; }}"))
        .expect("a standard exit code should have hover documentation");
    let (description, rest) = contents
        .split_once("\n\n**Phase**: ")
        .expect("exit code hover should contain its phase");
    let (phase, url) = rest
        .split_once("\n\nLearn more about exit codes in documentation: ")
        .expect("exit code hover should contain a documentation link");
    expect.assert_eq(&format!(
        "{description}\nphase: {phase}\ndocumentation link: {}",
        if url == EXIT_CODES_URL {
            "valid"
        } else {
            "invalid"
        }
    ));
}

macro_rules! exit_code_tests {
    ($(($name:ident, $code:literal, $expect:literal)),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                check_exit_code($code, expect![$expect]);
            }
        )+
    };
}

exit_code_tests!(
    (
        exit_code_0,
        "0",
        "Standard successful execution exit code.\nphase: Compute and action phases\ndocumentation link: valid"
    ),
    (
        exit_code_1,
        "1",
        "Alternative successful execution exit code. Reserved, but doesn’t occur.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_2,
        "2",
        "Stack underflow.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_3,
        "3",
        "Stack overflow.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_4,
        "4",
        "Integer overflow.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_5,
        "5",
        "Range check error — some integer is out of its expected range.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_6,
        "6",
        "Invalid TVM opcode.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_7,
        "7",
        "Type check error.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_8,
        "8",
        "Cell overflow.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_9,
        "9",
        "Cell underflow.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_10,
        "10",
        "Dictionary error.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_11,
        "11",
        "Described in TVM docs as “Unknown error, may be thrown by user programs”.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_12,
        "12",
        "Fatal error. Thrown by TVM in situations deemed impossible.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_13,
        "13",
        "Out of gas error.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_negative_14,
        "-14",
        "Same as 13. Negative, so that it cannot be faked.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_14,
        "14",
        "VM virtualization error. Reserved, but never thrown.\nphase: Compute phase\ndocumentation link: valid"
    ),
    (
        exit_code_32,
        "32",
        "Action list is invalid.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_33,
        "33",
        "Action list is too long.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_34,
        "34",
        "Action is invalid or not supported.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_35,
        "35",
        "Invalid source address in outbound message.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_36,
        "36",
        "Invalid destination address in outbound message.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_37,
        "37",
        "Not enough Toncoin.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_38,
        "38",
        "Not enough extra currencies.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_39,
        "39",
        "Outbound message does not fit into a cell after rewriting.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_40,
        "40",
        "Cannot process a message — not enough funds, the message is too large or its Merkle depth is too big.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_41,
        "41",
        "Library reference is null during library change action.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_42,
        "42",
        "Library change action error.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_43,
        "43",
        "Exceeded maximum number of cells in the library or the maximum depth of the Merkle tree.\nphase: Action phase\ndocumentation link: valid"
    ),
    (
        exit_code_50,
        "50",
        "Account state size exceeded limits.\nphase: Action phase\ndocumentation link: valid"
    ),
);

fn check_annotation(source: &str, expect: Expect) {
    expect
        .assert_eq(&hover(source).expect("a supported annotation should have hover documentation"));
}

macro_rules! annotation_tests {
    ($(($name:ident, $source:literal, $expect:literal)),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                check_annotation($source, expect![$expect]);
            }
        )+
    };
}

annotation_tests!(
    (
        annotation_inline,
        "@<caret>inline\nfun f() {}",
        "Function with this annotation will be automatically inlined during compilation"
    ),
    (
        annotation_inline_ref,
        "@<caret>inline_ref\nfun f() {}",
        "Function with this annotation will be automatically inlined by reference during compilation"
    ),
    (
        annotation_noinline,
        "@<caret>noinline\nfun f() {}",
        "Function with this annotation will not be inlined even if compiler can inline it"
    ),
    (
        annotation_pure,
        "@<caret>pure\nfun f() {}",
        "Function with this annotation has no side effects and can be optimized away by the compiler"
    ),
    (
        annotation_deprecated,
        "@<caret>deprecated(\"use replacement\")\nfun f() {}",
        "Symbol with this annotation is deprecated and should not be used in new code. First string argument is a reason for deprecation as a string literal."
    ),
    (
        annotation_overflow_policy,
        "@<caret>overflow1023_policy(\"suppress\")\nfun f() {}",
        "Defines the policy for handling potential builder overflow. Right now, only `\"suppress\"` value is supported. See <https://docs.ton.org/v3/documentation/smart-contracts/tolk/tolk-vs-func/pack-to-from-cells#what-if-data-exceeds-1023-bits> for more details"
    ),
    (
        annotation_bounced_policy,
        "@<caret>on_bounced_policy(\"manual\")\nfun f() {}",
        "Defines the policy for handling bounced messages. Right now, only `\"manual\"` value is supported."
    ),
    (
        annotation_method_id,
        "@<caret>method_id(1)\nfun f() {}",
        "Specifies the method ID (as a number literal) for the function in smart contract interface. See <https://docs.ton.org/v3/guidelines/smart-contracts/get-methods> for more details"
    ),
    (
        annotation_abi,
        "@<caret>abi\nstruct Message {}",
        "Describes ABI metadata for a declaration."
    ),
    (
        annotation_abi_minimal_value,
        "@<caret>abi.minimalMsgValue(1)\nstruct Message {}",
        "Defines the minimal message value for a message struct in ABI metadata."
    ),
    (
        annotation_abi_send_mode,
        "@<caret>abi.preferredSendMode(0)\nstruct Message {}",
        "Defines the preferred send mode for a message struct in ABI metadata."
    ),
    (
        annotation_abi_client_type,
        "struct Message { @<caret>abi.clientType(Cell) body: cell }",
        "Overrides the client-facing ABI type for a struct field. This is useful when generated wrappers should expose a different representation than the serialized Tolk field type."
    ),
    (
        annotation_test,
        "@<caret>test\nfun f() {}",
        "Describes additional metadata for a test function, such as skipping, TODO state, expected exit code, gas limit, or fuzzing configuration."
    ),
    (
        annotation_test_skip,
        "@<caret>test.skip\nfun f() {}",
        "Marks the test as skipped."
    ),
    (
        annotation_test_todo,
        "@<caret>test.todo(\"later\")\nfun f() {}",
        "Marks the test as TODO. Use `@test.todo(\"...\")` to attach a description."
    ),
    (
        annotation_test_fail_with,
        "@<caret>test.fail_with(42)\nfun f() {}",
        "Declares the expected exit code for the test."
    ),
    (
        annotation_test_gas_limit,
        "@<caret>test.gas_limit(1000)\nfun f() {}",
        "Overrides the per-test gas limit."
    ),
    (
        annotation_test_fuzz,
        "@<caret>test.fuzz(64)\nfun f(value: int) {}",
        "Enables fuzzing for parameterized tests. Supports `@test.fuzz`, `@test.fuzz(64)`, and `@test.fuzz({ ... })`."
    ),
);

fn check_contract_field(field: &str, expect: Expect) {
    let contents = hover(&format!("contract Demo {{ <caret>{field}: Value }}"))
        .expect("a contract header field should have hover documentation");
    let documentation = contents
        .rsplit_once("```\n")
        .expect("contract field hover should contain a presentation")
        .1
        .trim();
    expect.assert_eq(documentation);
}

macro_rules! contract_field_tests {
    ($(($name:ident, $field:literal, $expect:literal)),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                check_contract_field($field, expect![$expect]);
            }
        )+
    };
}

contract_field_tests!(
    (contract_field_name, "contractName", "Name of the contract."),
    (contract_field_author, "author", "Author of the contract."),
    (
        contract_field_version,
        "version",
        "Version of the contract."
    ),
    (
        contract_field_description,
        "description",
        "Description of the contract."
    ),
    (
        contract_field_incoming_messages,
        "incomingMessages",
        "Defines the type of allowed incoming internal messages. Usually a union type of all supported message structs."
    ),
    (
        contract_field_incoming_external,
        "incomingExternal",
        "Defines the type of allowed incoming external messages."
    ),
    (
        contract_field_storage,
        "storage",
        "Defines the persistent storage structure for the contract. This field usually points to a struct type."
    ),
    (
        contract_field_deployment_storage,
        "storageAtDeployment",
        "Defines the storage structure at the moment of deployment."
    ),
    (
        contract_field_force_abi_export,
        "forceAbiExport",
        "List of types to additionally export to ABI."
    ),
);

fn check_tlb_type(name: &str, expect: Expect) {
    let contents = hover(&format!(
        "type intN = builtin; type uintN = builtin; struct Value {{ value: <caret>{name} }}"
    ))
    .unwrap_or_default();
    let summary = contents.find("- **Range**:").map_or_else(
        || "no TL-B documentation".to_owned(),
        |start| {
            contents[start..]
                .lines()
                .take(3)
                .collect::<Vec<_>>()
                .join("\n")
        },
    );
    expect.assert_eq(&summary);
}

macro_rules! tlb_type_tests {
    ($(($name:ident, $ty:literal, $expect:literal)),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                check_tlb_type($ty, expect![$expect]);
            }
        )+
    };
}

tlb_type_tests!(
    (
        tlb_uint8,
        "uint8",
        "- **Range**: 0 to 255 (2^8 - 1)\n- **Size**: 8 bits = 1 byte\n- **TL-B**: uint8"
    ),
    (
        tlb_uint16,
        "uint16",
        "- **Range**: 0 to 65,535 (2^16 - 1)\n- **Size**: 16 bits = 2 bytes\n- **TL-B**: uint16"
    ),
    (
        tlb_uint32,
        "uint32",
        "- **Range**: 0 to 4,294,967,295 (2^32 - 1)\n- **Size**: 32 bits = 4 bytes\n- **TL-B**: uint32"
    ),
    (
        tlb_uint64,
        "uint64",
        "- **Range**: 0 to 2^64 - 1\n- **Size**: 64 bits = 8 bytes\n- **TL-B**: uint64"
    ),
    (
        tlb_uint128,
        "uint128",
        "- **Range**: 0 to 2^128 - 1\n- **Size**: 128 bits = 16 bytes\n- **TL-B**: uint128"
    ),
    (
        tlb_uint256,
        "uint256",
        "- **Range**: 0 to 2^256 - 1\n- **Size**: 256 bits = 32 bytes\n- **TL-B**: uint256"
    ),
    (
        tlb_int8,
        "int8",
        "- **Range**: -128 to 127 (-2^7 to 2^7 - 1)\n- **Size**: 8 bits = 1 byte\n- **TL-B**: int8"
    ),
    (
        tlb_int16,
        "int16",
        "- **Range**: -32,768 to 32,767 (-2^15 to 2^15 - 1)\n- **Size**: 16 bits = 2 bytes\n- **TL-B**: int16"
    ),
    (
        tlb_int32,
        "int32",
        "- **Range**: -2^31 to 2^31 - 1\n- **Size**: 32 bits = 4 bytes\n- **TL-B**: int32"
    ),
    (
        tlb_int64,
        "int64",
        "- **Range**: -2^63 to 2^63 - 1\n- **Size**: 64 bits = 8 bytes\n- **TL-B**: int64"
    ),
    (
        tlb_int128,
        "int128",
        "- **Range**: -2^127 to 2^127 - 1\n- **Size**: 128 bits = 16 bytes\n- **TL-B**: int128"
    ),
    (
        tlb_int256,
        "int256",
        "- **Range**: -2^255 to 2^255 - 1\n- **Size**: 256 bits = 32 bytes\n- **TL-B**: int256"
    ),
    (
        tlb_int257,
        "int257",
        "- **Range**: -2^256 to 2^256 - 1\n- **Size**: 257 bits = 32 bytes + 1 bit\n- **TL-B**: int257"
    ),
    (
        tlb_varuint16,
        "varuint16",
        "- **Range**: 0 to 2^120 - 1\n- **Size**: 4 to 124 bits\n- **TL-B**: varuint16"
    ),
    (
        tlb_varint16,
        "varint16",
        "- **Range**: -2^119 to 2^119 - 1\n- **Size**: 4 to 124 bits\n- **TL-B**: varint16"
    ),
    (
        tlb_varuint32,
        "varuint32",
        "- **Range**: 0 to 2^248 - 1\n- **Size**: 5 to 253 bits\n- **TL-B**: varuint32"
    ),
    (
        tlb_varint32,
        "varint32",
        "- **Range**: -2^247 to 2^247 - 1\n- **Size**: 5 to 253 bits\n- **TL-B**: varint32"
    ),
    (
        tlb_arbitrary_uint1,
        "uint1",
        "- **Range**: 0 to 2^1 - 1\n- **Size**: 1 bits\n- **TL-B**: uint1"
    ),
    (
        tlb_arbitrary_uint2,
        "uint2",
        "- **Range**: 0 to 2^2 - 1\n- **Size**: 2 bits\n- **TL-B**: uint2"
    ),
    (
        tlb_arbitrary_uint7,
        "uint7",
        "- **Range**: 0 to 2^7 - 1\n- **Size**: 7 bits\n- **TL-B**: uint7"
    ),
    (
        tlb_arbitrary_uint9,
        "uint9",
        "- **Range**: 0 to 2^9 - 1\n- **Size**: 9 bits\n- **TL-B**: uint9"
    ),
    (
        tlb_arbitrary_uint24,
        "uint24",
        "- **Range**: 0 to 2^24 - 1\n- **Size**: 24 bits\n- **TL-B**: uint24"
    ),
    (
        tlb_arbitrary_uint63,
        "uint63",
        "- **Range**: 0 to 2^63 - 1\n- **Size**: 63 bits\n- **TL-B**: uint63"
    ),
    (
        tlb_arbitrary_uint127,
        "uint127",
        "- **Range**: 0 to 2^127 - 1\n- **Size**: 127 bits\n- **TL-B**: uint127"
    ),
    (
        tlb_arbitrary_uint255,
        "uint255",
        "- **Range**: 0 to 2^255 - 1\n- **Size**: 255 bits\n- **TL-B**: uint255"
    ),
    (
        tlb_arbitrary_int1,
        "int1",
        "- **Range**: -2^0 to 2^0 - 1\n- **Size**: 1 bits\n- **TL-B**: int1"
    ),
    (
        tlb_arbitrary_int2,
        "int2",
        "- **Range**: -2^1 to 2^1 - 1\n- **Size**: 2 bits\n- **TL-B**: int2"
    ),
    (
        tlb_arbitrary_int7,
        "int7",
        "- **Range**: -2^6 to 2^6 - 1\n- **Size**: 7 bits\n- **TL-B**: int7"
    ),
    (
        tlb_arbitrary_int9,
        "int9",
        "- **Range**: -2^8 to 2^8 - 1\n- **Size**: 9 bits\n- **TL-B**: int9"
    ),
    (
        tlb_arbitrary_int24,
        "int24",
        "- **Range**: -2^23 to 2^23 - 1\n- **Size**: 24 bits\n- **TL-B**: int24"
    ),
    (tlb_invalid_uint_zero, "uint0", "no TL-B documentation"),
    (
        tlb_invalid_uint_too_wide,
        "uint257",
        "no TL-B documentation"
    ),
    (tlb_invalid_int_zero, "int0", "no TL-B documentation"),
    (tlb_invalid_int_too_wide, "int258", "no TL-B documentation"),
    (tlb_invalid_uint_suffix, "uintx", "no TL-B documentation"),
    (tlb_invalid_int_suffix, "intx", "no TL-B documentation"),
);

fn check_import_documentation(source: &str, expect: Expect) {
    let marked = MarkedSource::parse("import \"<caret>library\"");
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .add_source_file(LANGUAGE_ID, "file:///fixture/library.tolk", source)
        .expect("imported file should be added");
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let contents = service
        .hover(&uri, marked.marker("caret").position)
        .expect("hover request should succeed")
        .expect("import hover should exist")
        .contents;
    let documentation = contents
        .split_once("\n```")
        .expect("import hover should contain the resolved path")
        .1
        .trim();
    expect.assert_eq(if documentation.is_empty() {
        "no file documentation"
    } else {
        documentation
    });
}

macro_rules! import_documentation_tests {
    ($(($name:ident, $source:literal, $expect:literal)),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                check_import_documentation($source, expect![$expect]);
            }
        )+
    };
}

import_documentation_tests!(
    (
        import_single_line_file_docs,
        "/// Reusable helpers.\n\nfun helper() {}\n",
        "Reusable helpers."
    ),
    (
        import_multiline_file_docs,
        "/// Reusable helpers.\n///\n/// Used by contracts.\n\nfun helper() {}\n",
        "Reusable helpers.\n\nUsed by contracts."
    ),
    (
        import_single_line_block_file_docs,
        "/** Reusable block helpers. */\n\nfun helper() {}\n",
        "Reusable block helpers."
    ),
    (
        import_multiline_block_file_docs,
        "/**\n * Reusable helpers.\n * Used by contracts.\n */\n\nfun helper() {}\n",
        "* Reusable helpers.\n * Used by contracts."
    ),
    (
        import_plain_header_is_not_docs,
        "// Generated file.\n\nfun helper() {}\n",
        "no file documentation"
    ),
    (
        import_line_docs_attached_to_declaration,
        "/// Documents helper.\nfun helper() {}\n",
        "no file documentation"
    ),
    (
        import_block_docs_attached_to_declaration,
        "/** Documents helper. */\nfun helper() {}\n",
        "no file documentation"
    ),
    (
        import_file_docs_after_leading_whitespace,
        "\n  /// Reusable helpers.\n\nfun helper() {}\n",
        "Reusable helpers."
    ),
);
