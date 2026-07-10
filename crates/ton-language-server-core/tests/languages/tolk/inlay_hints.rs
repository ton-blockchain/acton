#[path = "../../support/snapshots.rs"]
mod snapshots;
#[path = "../../support/mod.rs"]
mod support;
#[path = "inlay_hints/upstream.rs"]
mod upstream;

use expect_test::{Expect, expect};
use snapshots::assert_file_snapshot;
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, InlayHint, LanguageService, LanguageServiceConfig, Position, ProfileSummary,
    Range, TextIndex,
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
    source_with_inline_hints(marked.source(), &hints)
}

fn source_with_inline_hints(source: &str, hints: &[InlayHint]) -> String {
    let index = TextIndex::new(source);
    let mut insertions = hints
        .iter()
        .enumerate()
        .map(|(order, hint)| {
            let offset = index.position_to_offset(source, hint.position);
            let label = hint.label.trim();
            let text = if label.starts_with("/*") && label.ends_with("*/") {
                label.to_owned()
            } else {
                format!("/* {label} */")
            };

            (offset, order, text)
        })
        .collect::<Vec<_>>();
    insertions.sort_by_key(|(offset, order, _)| std::cmp::Reverse((*offset, *order)));

    let mut annotated = source.to_owned();
    for (offset, _, text) in insertions {
        annotated.insert_str(offset, &text);
    }
    annotated
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
        expect![[r#"
            const COMPUTED/* : int */ = 1 + 2/* = 3 (0x3) */

            fun answer()/* : int */ {
                val result/* : int */ = 40 + 2;
                return result;
            }"#]],
    );
}

#[test]
fn shows_types_for_stdlib_methods_on_string_literals() {
    case_tolk_inlay_hints(
        r#"
            fun main() {
                val valid = "abc-123".beginParse();
                val cell = beginCell().storeSlice("ce".beginParse()).endCell();
            }
        "#,
        full_document_range(),
        expect![[r#"
            fun main()/* : void */ {
                val valid/* : slice */ = "abc-123".beginParse();
                val cell/* : cell */ = beginCell().storeSlice("ce".beginParse()).endCell();
            }"#]],
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
        expect![[r#"
            fun apply(f: (int) -> int): int {
                return f(1);
            }

            fun main(): int {
                return apply(fun(value/* : int */) {
                    val _ignored = value;
                    return value + 1;
                });
            }"#]],
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
        expect![[r#"
            struct Payload {
                value: int
            }

            const EXPLICIT: int = 1
            const OBJECT = Payload { value: 1 }

            fun typed(value: int): int {
                val local: int = value;
                return local;
            }"#]],
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
        expect![[r#"
            fun answer() {
                val result/* : int */ = 40 + 2;
                return result;
            }"#]],
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
        expect![[r#"
            get/* (0x18762) */ fun data(): int { return 0 }

            @method_id(0x100)
            get fun explicitId(): int { return 0 }

            @method_id()
            get fun emptyId(): int { return 0 }

            @foo()
            get/* (0x17cf5) */ fun annotated(): int { return 0 }

            get fun `test foo`(): int { return 0 }
            get fun `test-foo`(): int { return 0 }
            get fun `test_foo`(): int { return 0 }"#]],
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
        expect![[r#"
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
            }"#]],
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
        expect![[r#"
            const CRC16/* : int */ = stringCrc16("hello")/* = 50018 (0xC362) */
            const CRC32/* : int */ = stringCrc32("hello")/* = 907060870 (0x3610A686) */
            const SHA32/* : int */ = stringSha256_32("hello")/* = 754077114 (0x2CF24DBA) */
            const BASE256/* : int */ = stringToBase256("TON")/* = 5525326 (0x544F4E) */
            const CYCLE_A/* : int */ = CYCLE_B + 1
            const CYCLE_B/* : int */ = CYCLE_A + 1"#]],
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
        expect![[r#"
            const ARITHMETIC/* : int */ = ((1 + 2) * 3 << 2) | 1/* = 37 (0x25) */
            const NEGATIVE/* : int */ = -(ARITHMETIC + 1)/* = 0x-26 */
            const LOGIC/* : bool */ = !false && ARITHMETIC >= 37/* = true */
            const CASTED/* : int */ = (0x10 as int) + 1/* = 17 (0x11) */"#]],
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
        expect![[r#"
            const BASE/* : int */ = 10;

            enum Other {
                Item = 3,
            }

            enum Color {
                Red/* = 0 */,
                Green = 5,
                Blue/* = 6 */,
                Negative = -1,
                Next/* = 0 */,
                Hex = 0x10/* = 16 */,
                Truthy = 10 > 0/* = -1 */,
                Falsy = 10 < 0/* = 0 */,
                AfterFalsy/* = 1 */,
                FromConst = BASE + 1/* = 11 */,
                FromOther = Other.Item + 2/* = 5 */,
            }

            fun notConst(): int { return 10 }

            enum Broken {
                A = 1,
                B = notConst(),
                C,
                Reset = 4,
                Last/* = 5 */,
            }

            enum SameEnumReference {
                Base = 100,
                Next = SameEnumReference.Base + 1,
                After,
                Reset = 7,
                Last/* = 8 */,
            }"#]],
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
        expect![[r#"
            fun main(): void {
                val (first/* : int */, second/* : bool */, third/* : string */) = (100, true, "");
                val [fourth/* : int */, fifth/* : bool */, sixth/* : string */] = [200, false, "ok"];
            }"#]],
    );
}

#[test]
fn range_filters_value_hints_independently() {
    case_tolk_inlay_hints(
        "const COMPUTED = 1 + 2",
        Range::new(Position::new(0, 0), Position::new(0, 14)),
        expect!["const COMPUTED/* : int */ = 1 + 2"],
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
        expect![[r#"
            fun recover()/* : void */ {
                try {
                    throw 1;
                } catch (exitCode/* : int */, argument/* : unknown */) {
                    return;
                }
            }"#]],
    );
}

#[test]
fn records_inlay_hint_profile_spans() {
    let uri = DocumentUri::from("file:///workspace/profiled.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .open_document(
            uri.clone(),
            LANGUAGE_ID,
            1,
            "fun main() { val value = 1 + 2; }",
        )
        .expect("Tolk document should open");

    let hints = service
        .inlay_hints(&uri, full_document_range())
        .expect("inlay hints request should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "hints={} inlay={} tolk.inlay={}",
        hints.len(),
        event_count(summary, "inlay_hints"),
        event_count(summary, "tolk.inlay_hints"),
    );
    expect!["hints=2 inlay=1 tolk.inlay=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
