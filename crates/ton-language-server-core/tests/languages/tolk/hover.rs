#[path = "../../support/mod.rs"]
mod support;
#[path = "hover/upstream.rs"]
mod upstream;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, ProfileSummary,
};

fn case_tolk_hover(source: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");

    let actual = marked
        .markers()
        .iter()
        .map(|marker| {
            service
                .hover(&uri, marker.position)
                .expect("hover request should succeed")
                .map_or_else(|| "no documentation".to_owned(), |hover| hover.contents)
        })
        .collect::<Vec<_>>()
        .join("\n");
    expect.assert_eq(&actual);
}

#[test]
fn shows_function_signatures_and_documentation() {
    case_tolk_hover(
        r"
            /// Computes an answer.
            @pure
            fun <caret>answer(value: int) {
                return value + 1;
            }
        ",
        expect![[r#"
            ```tolk
            @pure
            fun answer(value: int): int
            ```
            Computes an answer."#]],
    );
}

#[test]
fn shows_inferred_local_types() {
    case_tolk_hover(
        r#"
            fun main() {
                val <caret>valid = "abc-123".beginParse();
            }
        "#,
        expect![[r#"
            ```tolk
            val valid: slice = "abc-123".beginParse()
            ```"#]],
    );
}

#[test]
fn resolves_hover_for_stdlib_methods() {
    case_tolk_hover(
        r#"
            fun main() {
                val valid = "abc-123".<caret>beginParse();
            }
        "#,
        expect![[r#"
            ```tolk
            @pure
            fun string.beginParse(self): slice
            ```
            Begins parsing the string content. Use for accessing raw bytes.
            Remember that a string may be a snake: not just "ab", but "a" + (ref "b")."#]],
    );
}

#[test]
fn shows_struct_enum_alias_and_value_declarations() {
    case_tolk_hover(
        r"
            struct Foo {
                private readonly <caret>value: int = 100,
            }
            enum Color: uint8 {
                <caret>Red = 10,
            }
            type <caret>IntOrString = int | string;
            const <caret>ANSWER = 42;
            global <caret>counter: int;
        ",
        expect![[r#"
            ```tolk
            struct Foo
            private readonly value: int = 100
            ```
            ```tolk
            enum Color
            Red = 10
            ```
            ```tolk
            type IntOrString =
                | int
                | string
            ```
            ```tolk
            const ANSWER: int = 42
            ```
            ```tolk
            global counter: int
            ```"#]],
    );
}

#[test]
fn shows_exit_code_documentation_only_for_exception_arguments() {
    case_tolk_hover(
        r"
            fun main() {
                throw <caret>1;
                assert (true) throw <caret>5;
                assert(true, <caret>10);
                assert(<caret>10, 10);
            }
        ",
        expect![[r#"
            Alternative successful execution exit code. Reserved, but doesn’t occur.

            **Phase**: Compute phase

            Learn more about exit codes in documentation: https://docs.ton.org/v3/documentation/tvm/tvm-exit-codes
            Range check error — some integer is out of its expected range.

            **Phase**: Compute phase

            Learn more about exit codes in documentation: https://docs.ton.org/v3/documentation/tvm/tvm-exit-codes
            Dictionary error.

            **Phase**: Compute phase

            Learn more about exit codes in documentation: https://docs.ton.org/v3/documentation/tvm/tvm-exit-codes
            no documentation"#]],
    );
}

#[test]
fn shows_annotation_documentation() {
    case_tolk_hover(
        r#"
            @<caret>deprecated("use replacement")
            fun old() {}
        "#,
        expect![[r"
            Symbol with this annotation is deprecated and should not be used in new code. First string argument is a reason for deprecation as a string literal."]],
    );
}

#[test]
fn shows_documentation_for_all_supported_annotations() {
    case_tolk_hover(
        r#"
            @<caret>inline
            @<caret>inline_ref
            @<caret>noinline
            @<caret>on_bounced_policy("manual")
            @<caret>overflow1023_policy("suppress")
            @<caret>pure
            @<caret>test.skip
            fun foo() {}

            struct Message {
                @<caret>abi.clientType(Cell)
                body: cell
            }
        "#,
        expect![[r#"
            Function with this annotation will be automatically inlined during compilation
            Function with this annotation will be automatically inlined by reference during compilation
            Function with this annotation will not be inlined even if compiler can inline it
            Defines the policy for handling bounced messages. Right now, only `"manual"` value is supported.
            Defines the policy for handling potential builder overflow. Right now, only `"suppress"` value is supported. See <https://docs.ton.org/v3/documentation/smart-contracts/tolk/tolk-vs-func/pack-to-from-cells#what-if-data-exceeds-1023-bits> for more details
            Function with this annotation has no side effects and can be optimized away by the compiler
            Marks the test as skipped.
            Overrides the client-facing ABI type for a struct field. This is useful when generated wrappers should expose a different representation than the serialized Tolk field type."#]],
    );
}

#[test]
fn shows_contract_and_contract_field_documentation() {
    case_tolk_hover(
        r#"
            /// Contract docs.
            contract <caret>MyContract {
                /// Custom author docs.
                <caret>author: "me",
                version: "1.0.0"
            }
        "#,
        expect![[r#"
            ```tolk
            contract MyContract {
                author: "me"
                version: "1.0.0"
            }
            ```
            Contract docs.
            ```tolk
            contract MyContract
            author
            ```
            Author of the contract.

            Custom author docs."#]],
    );
}

#[test]
fn shows_field_comments_and_modifiers() {
    case_tolk_hover(
        r"
            struct Foo {
                readonly <caret>first: int, // inline docs
                // preceding docs
                private readonly <caret>second: bool,
                <caret>third: int, /* first */ /* ignored */
            }
        ",
        expect![[r#"
            ```tolk
            struct Foo
            readonly first: int
            ```
            inline docs
            ```tolk
            struct Foo
            private readonly second: bool
            ```
            preceding docs
            ```tolk
            struct Foo
            third: int
            ```
            first"#]],
    );
}

#[test]
fn shows_local_parameter_catch_and_type_parameter_presentations() {
    case_tolk_hover(
        r"
            struct Generic<TName = int> {
                field: <caret>TName,
            }

            fun int.convert<TValue>(self, mutate <caret>parameter: int = 10): <caret>TValue {
                val <caret>value = 10;
                val [<caret>first, second] = [10, 20];
                try {} catch (<caret>error, <caret>data) {}
                return value;
            }
        ",
        expect![[r#"
            ```tolk
            struct Generic
            TName = int
            ```
            ```tolk
            mutate parameter: int = 10
            ```
            ```tolk
            fun int.convert
            TValue
            ```
            ```tolk
            val value: int = 10
            ```
            ```tolk
            val [first, second] = [10, 20]
            ```
            ```tolk
            catch (error)
            ```
            ```tolk
            catch (data)
            ```"#]],
    );
}

#[test]
fn shows_get_method_ids_and_inferred_return_types() {
    case_tolk_hover(
        r#"
            get fun <caret>computed() {}

            /// Explicit getter.
            @method_id(0x100)
            get fun <caret>explicit() {}

            fun <caret>union(cond: bool) {
                if (cond) {
                    return "hello";
                }
                return 10;
            }
        "#,
        expect![[r#"
            ```tolk
            get fun computed()
            ```
            Method ID: `0x152e6`
            ```tolk
            @method_id(0x100)
            get fun explicit()
            ```
            Method ID: `0x100`

            Explicit getter.
            ```tolk
            fun union(cond: bool): string | int
            ```"#]],
    );
}

#[test]
fn shows_tlb_documentation_for_fixed_and_arbitrary_integers() {
    case_tolk_hover(
        r"
            type intN = builtin;
            type uintN = builtin;

            struct Foo {
                signed: <caret>int32,
                unsigned: <caret>uint32,
                narrow: <caret>int24,
                wide: <caret>uint244,
                invalid: <caret>uint9999,
            }
        ",
        expect![[r#"
            ```tolk
            type int32 = builtin
            ```

            - **Range**: -2^31 to 2^31 - 1
            - **Size**: 32 bits = 4 bytes
            - **TL-B**: int32
            ```tolk
            type uint32 = builtin
            ```

            - **Range**: 0 to 4,294,967,295 (2^32 - 1)
            - **Size**: 32 bits = 4 bytes
            - **TL-B**: uint32
            ```tolk
            type int24 = builtin
            ```

            - **Range**: -2^23 to 2^23 - 1
            - **Size**: 24 bits
            - **TL-B**: int24

            Arbitrary bit-width signed integer type
            ```tolk
            type uint244 = builtin
            ```

            - **Range**: 0 to 2^244 - 1
            - **Size**: 244 bits
            - **TL-B**: uint244

            Arbitrary bit-width unsigned integer type
            ```tolk
            type uint9999 = builtin
            ```"#]],
    );
}

#[test]
fn shows_serialization_sizes_for_supported_type_shapes() {
    case_tolk_hover(
        r"
            struct <caret>Fixed {
                a: uint32
                b: int32
            }

            struct (0x7e8764ef) <caret>Prefixed {
                a: uint32
                b: int32
            }

            struct <caret>Optional {
                value: uint32?
            }

            struct <caret>References {
                amount: coins
                body: cell
            }

            struct <caret>Dynamic {
                value: builder
            }

            struct <caret>Binary {
                bits: bits32
                bytes: bytes32
            }

            type <caret>Either = uint32 | int64;
        ",
        expect![[r#"
            ```tolk
            struct Fixed {
                a: uint32
                b: int32
            }
            ```
            **Size:** 64 bits.

            ---
            ```tolk
            struct (0x7e8764ef) Prefixed {
                a: uint32
                b: int32
            }
            ```
            **Size:** 96 bits.

            ---
            ```tolk
            struct Optional {
                value: uint32?
            }
            ```
            **Size:** 33 bits.

            ---
            ```tolk
            struct References {
                amount: coins
                body: cell
            }
            ```
            **Size:** 4..124 bits, 1 ref.

            ---
            ```tolk
            struct Dynamic {
                value: builder
            }
            ```
            **Size:** 0..9999 bits, 0..4 refs.

            ---
            ```tolk
            struct Binary {
                bits: bits32
                bytes: bytes32
            }
            ```
            **Size:** 288 bits.

            ---
            ```tolk
            type Either =
                | uint32
                | int64
            ```
            **Size:** 33..65 bits, 0 refs.

            ---"#]],
    );
}

#[test]
fn shows_serialization_sizes_for_nested_unions_and_generics() {
    case_tolk_hover(
        r"
            struct Inner {
                value: int32
            }

            type Alias = int32;

            struct <caret>Composite {
                address: address
                inner: Inner
                alias: Alias
                tensor: (int32, bool)
                tuple: [int32, bool, cell]
                maybeCell: Cell<int32>?
            }

            struct (0x7e8764ef) Increase {
                queryId: uint64
                increaseBy: uint32
            }

            struct (0x3a) Reset {
                queryId: uint64
                action: Cell<Inner>
            }

            type <caret>Message = Increase | Reset;
            type <caret>PrimitiveUnion = int32 | int64 | bool;

            struct <caret>GenericUse {
                value: Wrapper<uint32>
            }

            struct <caret>Wrapper<T> {
                value: T
            }

            type <caret>GenericAlias = Identity<int32>;
            type Identity<T> = T;
        ",
        expect![[r#"
            ```tolk
            struct Composite {
                address: address
                inner: Inner
                alias: Alias
                tensor: (int32, bool)
                tuple: [int32, bool, cell]
                maybeCell: Cell<int32>?
            }
            ```
            **Size:** 133..398 bits, 1..2 refs.

            ---
            ```tolk
            type Message =
                | Increase
                | Reset
            ```
            **Size:** 72..128 bits, 0..1 refs.

            ---
            ```tolk
            type PrimitiveUnion =
                | int32
                | int64
                | bool
            ```
            **Size:** 3..66 bits, 0 refs.

            ---
            ```tolk
            struct GenericUse {
                value: Wrapper<uint32>
            }
            ```
            **Size:** 32 bits.

            ---
            ```tolk
            struct Wrapper<T> {
                value: T
            }
            ```
            **Size:** 0..9999 bits, 0..4 refs.

            ---
            ```tolk
            type GenericAlias = Identity<int32>
            ```
            **Size:** 32 bits.

            ---"#]],
    );
}

#[test]
fn shows_evaluated_constant_values() {
    case_tolk_hover(
        r"
            const <caret>BASE = 10;
            const <caret>COMPUTED = BASE + 5;
        ",
        expect![[r#"
            ```tolk
            const BASE: int = 10
            ```
            ```tolk
            const COMPUTED: int = BASE + 5 // 15 (0xF)
            ```"#]],
    );
}

#[test]
fn shows_documentation_across_generic_map_expressions() {
    case_tolk_hover(
        r#"
            import "@stdlib/tvm-dicts";

            struct Map<K, <caret>V> {
                data: <caret>dict,
            }

            fun emptyMap<K, V>() {
                return Map<K, <caret>V> {
                    <caret>data: null
                };
            }

            fun Map<K, V>.set(self,
                <caret>key: K,
                value: <caret>V) {}

            fun Map<int32, int>.set(mutate self, key: int, value: int) {
                self.<caret>data.iDictSetBuilder(32, key,
                    <caret>beginCell()
                    .<caret>storeInt(value, 257));
            }

            fun Map<int32, int>.has(mutate self, key: int) {
                return <caret>self.data.
                    <caret>iDictGet(32, key).1;
            }

            fun main() {
                var map = <caret>emptyMap<int32, int>();
                map.<caret>set(1, 10);

                if (<caret>map
                    .<caret>has(1)) {
                    return;
                }
            }
        "#,
        expect![[r#"
            ```tolk
            struct Map
            V
            ```
            ```tolk
            type dict = cell?
            ```
            **Size:** 1 bit, 0..1 refs.

            ---

            Think of it as "a map with unknown keys and unknown values".
            Prefer using `map<K, V>`, not `dict`.
            ```tolk
            fun emptyMap
            V
            ```
            ```tolk
            struct Map
            data: dict
            ```
            ```tolk
            key: K
            ```
            ```tolk
            fun Map<K, V>.set
            V
            ```
            ```tolk
            struct Map
            data: dict
            ```
            ```tolk
            @pure
            fun beginCell(): builder
            ```
            Creates a new empty builder.
            ```tolk
            @pure
            fun builder.storeInt(mutate self, x: int, len: int): self
            ```
            Stores a signed len-bit integer into a builder (`0 ≤ len ≤ 257`).
            ```tolk
            mutate self: Map<int32, int>
            ```
            ```tolk
            @pure
            fun dict.iDictGet(self, keyLen: int, key: int): (slice?, bool)
            ```
            ```tolk
            fun emptyMap<K, V>(): Map<K, V>
            ```
            ```tolk
            fun Map<int32, int>.set(mutate self, key: int, value: int): void
            ```
            ```tolk
            var map: Map<int32, int> = emptyMap<int32, int>()
            ```
            ```tolk
            fun Map<int32, int>.has(mutate self, key: int): bool
            ```"#]],
    );
}

#[test]
fn shows_resolved_import_path() {
    case_tolk_hover(
        r#"import "<caret>@stdlib/common""#,
        expect![[r#"
            ```tolk
            import "/__tolk_stdlib__/common.tolk"
            ```"#]],
    );
}

#[test]
fn records_hover_profile_spans() {
    let marked = MarkedSource::parse("fun <caret>main() {}\n");
    let uri = DocumentUri::from("file:///fixture/profiled.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let hover = service
        .hover(&uri, marked.marker("caret").position)
        .expect("hover request should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "hover={} hover.span={} tolk.hover={}",
        hover.is_some(),
        event_count(summary, "hover"),
        event_count(summary, "tolk.hover"),
    );
    expect!["hover=true hover.span=1 tolk.hover=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
