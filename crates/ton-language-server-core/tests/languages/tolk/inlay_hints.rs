#[path = "../../support/snapshots.rs"]
mod snapshots;
#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use snapshots::assert_file_snapshot;
use support::{MarkedSource, render_inlay_hints};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, Position, Range,
};

fn case_tolk_inlay_hints(source: &str, range: Range, expect: Expect) {
    expect.assert_eq(&tolk_inlay_hints(source, range));
}

fn tolk_inlay_hints(source: &str, range: Range) -> String {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///workspace/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");

    let hints = service
        .inlay_hints(&uri, range)
        .expect("inlay hints request should succeed");
    render_inlay_hints(&hints)
}

const fn full_document_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(u32::MAX, u32::MAX))
}

#[test]
fn shows_inferred_types() {
    case_tolk_inlay_hints(
        r"
            const COMPUTED = 1 + 2

            fun answer() {
                val result = 40 + 2;
                return result;
            }
        ",
        full_document_range(),
        expect![[r"
            0:14 kind=type      label=: int
            0:22 kind=none      label= /* = 3 (0x3) */ tooltip=Evaluated value: 3 (0x3)
            2:12 kind=type      label=: int
            3:14 kind=type      label=: int"]],
    );
}

#[test]
fn shows_inferred_parameter_type_and_skips_underscored_locals() {
    case_tolk_inlay_hints(
        r"
            fun apply(f: (int) -> int): int {
                return f(1);
            }

            fun main(): int {
                return apply(fun(value) {
                    val _ignored = value;
                    return value + 1;
                });
            }
        ",
        full_document_range(),
        expect!["5:26 kind=type      label=: int"],
    );
}

#[test]
fn skips_explicit_and_obvious_constant_types() {
    case_tolk_inlay_hints(
        r"
            struct Payload {
                value: int
            }

            const EXPLICIT: int = 1
            const OBJECT = Payload { value: 1 }

            fun typed(value: int): int {
                val local: int = value;
                return local;
            }
        ",
        full_document_range(),
        expect!["<none>"],
    );
}

#[test]
fn respects_requested_range() {
    case_tolk_inlay_hints(
        r"
            fun answer() {
                val result = 40 + 2;
                return result;
            }
        ",
        Range::new(Position::new(1, 0), Position::new(1, u32::MAX)),
        expect!["1:14 kind=type      label=: int"],
    );
}

#[test]
fn complete_hint_set_matches_file_snapshot() -> anyhow::Result<()> {
    let source = include_str!("../../fixtures/languages/tolk/inlay_hints.tolk");
    let actual = tolk_inlay_hints(source, full_document_range());
    assert_file_snapshot("languages/tolk/inlay_hints.snap", &actual)
}

#[test]
fn handles_method_id_annotations_and_test_names() {
    case_tolk_inlay_hints(
        r"
            get fun data(): int { return 0 }

            @method_id(0x100)
            get fun explicitId(): int { return 0 }

            @method_id()
            get fun emptyId(): int { return 0 }

            @foo()
            get fun annotated(): int { return 0 }

            get fun `test foo`(): int { return 0 }
            get fun `test-foo`(): int { return 0 }
            get fun `test_foo`(): int { return 0 }
        ",
        full_document_range(),
        expect![[r"
            0:3 kind=type      label=(0x18762)
            9:3 kind=type      label=(0x17cf5)"]],
    );
}

#[test]
fn suppresses_redundant_parameter_hints() {
    case_tolk_inlay_hints(
        r"
            struct Payload {
                sender: int
                value: int
            }

            fun sender(): int { return 1 }
            fun sameName(value: int): void {}
            fun sameField(sender: int): void {}
            fun sameCall(sender: int): void {}
            fun shortName(x: int): void {}
            fun stringArg(constString: int): void {}
            fun objectArg(payload: Payload): void {}
            fun println(message: int): void {}

            fun main(): void {
                val value: int = 1;
                val payload: Payload = Payload { sender: 1, value: 2 };
                sameName(value);
                sameField(payload.sender);
                sameCall(sender());
                shortName(1);
                stringArg(1);
                objectArg(Payload { sender: 1, value: 2 });
                println(1);
            }
        ",
        full_document_range(),
        expect!["<none>"],
    );
}

#[test]
fn evaluates_compile_time_functions_and_skips_cycles() {
    case_tolk_inlay_hints(
        r#"
            const CRC16 = stringCrc16("hello")
            const CRC32 = stringCrc32("hello")
            const SHA32 = stringSha256_32("hello")
            const BASE256 = stringToBase256("TON")
            const CYCLE_A = CYCLE_B + 1
            const CYCLE_B = CYCLE_A + 1
        "#,
        full_document_range(),
        expect![[r"
            0:11 kind=type      label=: int
            0:34 kind=none      label= /* = 50018 (0xC362) */ tooltip=Evaluated value: 50018 (0xC362)
            1:11 kind=type      label=: int
            1:34 kind=none      label= /* = 907060870 (0x3610A686) */ tooltip=Evaluated value: 907060870 (0x3610A686)
            2:11 kind=type      label=: int
            2:38 kind=none      label= /* = 754077114 (0x2CF24DBA) */ tooltip=Evaluated value: 754077114 (0x2CF24DBA)
            3:13 kind=type      label=: int
            3:38 kind=none      label= /* = 5525326 (0x544F4E) */ tooltip=Evaluated value: 5525326 (0x544F4E)
            4:13 kind=type      label=: int
            5:13 kind=type      label=: int"]],
    );
}

#[test]
fn evaluates_operators_and_casts() {
    case_tolk_inlay_hints(
        r"
            const ARITHMETIC = ((1 + 2) * 3 << 2) | 1
            const NEGATIVE = -(ARITHMETIC + 1)
            const LOGIC = !false && ARITHMETIC >= 37
            const CASTED = (0x10 as int) + 1
        ",
        full_document_range(),
        expect![[r"
            0:16 kind=type      label=: int
            0:41 kind=none      label= /* = 37 (0x25) */ tooltip=Evaluated value: 37 (0x25)
            1:14 kind=type      label=: int
            1:34 kind=none      label= /* = 0x-26 */ tooltip=Evaluated value: 0x-26
            2:11 kind=type      label=: bool
            2:40 kind=none      label= /* = true */ tooltip=Evaluated value: true
            3:12 kind=type      label=: int
            3:32 kind=none      label= /* = 17 (0x11) */ tooltip=Evaluated value: 17 (0x11)"]],
    );
}

#[test]
fn evaluates_enum_values_like_tolk() {
    case_tolk_inlay_hints(
        r"
            const BASE = 10;

            enum Other {
                Item = 3,
            }

            enum Color {
                Red,
                Green = 5,
                Blue,
                Negative = -1,
                Next,
                Hex = 0x10,
                Truthy = 10 > 0,
                Falsy = 10 < 0,
                AfterFalsy,
                FromConst = BASE + 1,
                FromOther = Other.Item + 2,
            }

            fun notConst(): int { return 10 }

            enum Broken {
                A = 1,
                B = notConst(),
                C,
                Reset = 4,
                Last,
            }

            enum SameEnumReference {
                Base = 100,
                Next = SameEnumReference.Base + 1,
                After,
                Reset = 7,
                Last,
            }
        ",
        full_document_range(),
        expect![[r"
            0:10 kind=type      label=: int
            7:7 kind=parameter label= = 0 tooltip=Enum value: 0
            9:8 kind=parameter label= = 6 tooltip=Enum value: 6
            11:8 kind=parameter label= = 0 tooltip=Enum value: 0
            12:14 kind=parameter label= = 16 tooltip=Enum value: 16
            13:19 kind=parameter label= = -1 tooltip=Enum value: -1
            14:18 kind=parameter label= = 0 tooltip=Enum value: 0
            15:14 kind=parameter label= = 1 tooltip=Enum value: 1
            16:24 kind=parameter label= = 11 tooltip=Enum value: 11
            17:30 kind=parameter label= = 5 tooltip=Enum value: 5
            27:8 kind=parameter label= = 5 tooltip=Enum value: 5
            35:8 kind=parameter label= = 8 tooltip=Enum value: 8"]],
    );
}

#[test]
fn shows_destructured_variable_types() {
    case_tolk_inlay_hints(
        r#"
            fun main(): void {
                val (first, second, third) = (100, true, "");
                val [fourth, fifth, sixth] = [200, false, "ok"];
            }
        "#,
        full_document_range(),
        expect![[r"
            1:14 kind=type      label=: int
            1:22 kind=type      label=: bool
            1:29 kind=type      label=: string
            2:15 kind=type      label=: int
            2:22 kind=type      label=: bool
            2:29 kind=type      label=: string"]],
    );
}

#[test]
fn range_filters_value_hints_independently() {
    case_tolk_inlay_hints(
        "const COMPUTED = 1 + 2",
        Range::new(Position::new(0, 0), Position::new(0, 14)),
        expect!["0:14 kind=type      label=: int"],
    );
}

#[test]
fn shows_catch_variable_types() {
    case_tolk_inlay_hints(
        r"
            fun recover() {
                try {
                    throw 1;
                } catch (exitCode, argument) {
                    return;
                }
            }
        ",
        full_document_range(),
        expect![[r"
            0:13 kind=type      label=: void
            3:21 kind=type      label=: int
            3:31 kind=type      label=: unknown"]],
    );
}
