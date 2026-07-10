#![allow(clippy::needless_raw_string_hashes)]

use expect_test::{Expect, expect};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentSymbol, DocumentUri, LanguageService, LanguageServiceConfig, Position, ProfileSummary,
};

fn case_document_symbols(source: &str, expect: Expect) {
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, source.to_owned())
        .expect("Tolk document should open");
    let symbols = service
        .document_symbols(&uri)
        .expect("document symbols request should succeed");
    expect.assert_eq(&render_symbols(&symbols));
}

fn render_symbols(symbols: &[DocumentSymbol]) -> String {
    let mut lines = Vec::new();
    for symbol in symbols {
        render_symbol(symbol, 0, &mut lines);
    }
    lines.join("\n")
}

fn render_symbol(symbol: &DocumentSymbol, depth: usize, lines: &mut Vec<String>) {
    lines.push(format!(
        "{}{}{} ({:?}) [{}-{}]",
        "  ".repeat(depth),
        symbol.name,
        symbol
            .detail
            .as_deref()
            .map(str::to_owned)
            .unwrap_or_default(),
        symbol.kind,
        position(symbol.range.start),
        position(symbol.range.end),
    ));
    for child in &symbol.children {
        render_symbol(child, depth + 1, lines);
    }
}

fn position(position: Position) -> String {
    format!("{}:{}", position.line, position.character)
}

#[test]
fn shows_nested_struct_and_enum_members() {
    case_document_symbols(
        r"struct Foo {
    foo: int,
    bar: string,
}

enum Color {
    Red = 10,
    Blue = 200 + 100,
}",
        expect![[r"
            Foo (Struct) [0:0-3:1]
              foo: int (Field) [1:4-1:12]
              bar: string (Field) [2:4-2:15]
            Color (Enum) [5:0-8:1]
              Red = 10 (EnumMember) [6:4-6:12]
              Blue = 200 + 100 (EnumMember) [7:4-7:20]"]],
    );
}

#[test]
fn shows_all_top_level_declaration_kinds_in_source_order() {
    case_document_symbols(
        r#"import "@stdlib/tvm-dicts";
import "./constants";

type Int = int;
struct Foo { foo: int }
const FOO: int = 100;
global bar: int;
fun foo() {}
fun Int.add(self, other: Int): Int {}
get method_id(): int {}"#,
        expect![[r#"
            import "@stdlib/tvm-dicts" (Module) [0:0-0:26]
            import "./constants" (Module) [1:0-1:20]
            Int (TypeParameter) [3:0-3:15]
            Foo (Struct) [4:0-4:23]
              foo: int (Field) [4:13-4:21]
            FOO: int = 100 (Constant) [5:0-5:21]
            bar: int (Variable) [6:0-6:16]
            foo() (Function) [7:0-7:12]
            Int.add(self, other: Int): Int (Method) [8:0-8:37]
            get method_id(): int (Event) [9:0-9:23]"#]],
    );
}

#[test]
fn shows_generic_function_and_inferred_value_details() {
    case_document_symbols(
        r"const ANSWER = 42;
global value: int;
fun identity<T>(value: T): T { return value; }
enum Empty { Member }",
        expect![[r"
            ANSWER: unknown = 42 (Constant) [0:0-0:18]
            value: int (Variable) [1:0-1:18]
            identity<T>(value: T): T (Function) [2:0-2:46]
            Empty (Enum) [3:0-3:21]
              Member (EnumMember) [3:13-3:19]"]],
    );
}

#[test]
fn empty_document_has_no_symbols() {
    case_document_symbols("", expect![""]);
}

#[test]
fn records_document_symbol_profile_spans() {
    let uri = DocumentUri::from("file:///fixture/profiled.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, "fun main() {}".to_owned())
        .expect("Tolk document should open");

    let symbols = service
        .document_symbols(&uri)
        .expect("document symbols request should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "symbols={} document_symbols={} tolk.document_symbols={}",
        symbols.len(),
        event_count(summary, "document_symbols"),
        event_count(summary, "tolk.document_symbols"),
    );
    expect!["symbols=1 document_symbols=1 tolk.document_symbols=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
