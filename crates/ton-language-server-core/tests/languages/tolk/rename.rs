#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;
#[path = "rename/upstream.rs"]
mod upstream;

use expect_test::{Expect, expect};
use std::collections::BTreeMap;
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentEdits, DocumentUri, LanguageService, LanguageServiceConfig, ProfileSummary, TextIndex,
};

fn check_rename(source: &str, new_name: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = open_service(&uri, &marked);
    let edit = service
        .rename(&uri, marked.marker("caret").position, new_name)
        .expect("rename request should succeed")
        .expect("rename should produce an edit");
    let document = edit
        .documents
        .iter()
        .find(|document| document.uri == uri)
        .expect("rename should edit the main document");
    let actual = apply_document_edits(marked.source(), document);

    expect.assert_eq(&actual);
}

fn check_rename_rejected(source: &str, new_name: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = open_service(&uri, &marked);
    let actual = match service.rename(&uri, marked.marker("caret").position, new_name) {
        Ok(None) => "not renameable".to_owned(),
        Ok(Some(_)) => "renameable".to_owned(),
        Err(error) => format!("error: {error}"),
    };

    expect.assert_eq(&actual);
}

fn open_service(uri: &DocumentUri, marked: &MarkedSource) -> LanguageService {
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    service
}

fn apply_document_edits(source: &str, document: &DocumentEdits) -> String {
    let index = TextIndex::new(source);
    let mut edits = document
        .edits
        .iter()
        .map(|edit| {
            (
                index.position_to_offset(source, edit.range.start),
                index.position_to_offset(source, edit.range.end),
                edit.new_text.as_str(),
            )
        })
        .collect::<Vec<_>>();
    edits.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));

    let mut result = source.to_owned();
    for (start, end, new_text) in edits {
        result.replace_range(start..end, new_text);
    }
    result
}

#[test]
fn renames_local_variables_without_crossing_scopes() {
    // All uses of a local variable in its lexical scope are renamed.
    check_rename(
        "
            fun test() {
                val num = 100;
                if (num == 10) {
                    throw <caret>num;
                }
            }
        ",
        "errno",
        expect![[r"
            fun test() {
                val errno = 100;
                if (errno == 10) {
                    throw errno;
                }
            }"]],
    );

    // A same-named local in a sibling block remains unchanged.
    check_rename(
        "
            fun test() {
                {
                    val num = 100;
                    throw <caret>num;
                }
                {
                    val num = 200;
                    throw num;
                }
            }
        ",
        "errno",
        expect![[r"
            fun test() {
                {
                    val errno = 100;
                    throw errno;
                }
                {
                    val num = 200;
                    throw num;
                }
            }"]],
    );

    // A tuple binding is handled as an independent local declaration.
    check_rename(
        "
            fun test() {
                val [num, <caret>other] = [100, 200];
                if (num == 10) {
                    throw other;
                }
            }
        ",
        "error",
        expect![[r"
            fun test() {
                val [num, error] = [100, 200];
                if (num == 10) {
                    throw error;
                }
            }"]],
    );
}

#[test]
fn renames_backticked_and_non_identifier_names() {
    // Existing backticks are replaced together with their content.
    check_rename(
        "
            fun test() {
                val `hello world` = 100;
                throw <caret>`hello world`;
            }
        ",
        "hello earth",
        expect![[r"
            fun test() {
                val `hello earth` = 100;
                throw `hello earth`;
            }"]],
    );

    // Names that are not plain identifiers are wrapped in backticks.
    check_rename(
        "
            fun test() {
                val foo = 100;
                throw <caret>foo;
            }
        ",
        "hello world",
        expect![[r"
            fun test() {
                val `hello world` = 100;
                throw `hello world`;
            }"]],
    );

    // Language keywords are also escaped with backticks.
    check_rename(
        "
            fun foo() {}
            fun test() {
                <caret>foo();
            }
        ",
        "return",
        expect![[r"
            fun `return`() {}
            fun test() {
                `return`();
            }"]],
    );

    // Contextual expression keywords are escaped by the same identifier policy.
    check_rename("fun <caret>foo() {}", "match", expect!["fun `match`() {}"]);
}

#[test]
fn renames_parameters_and_catch_bindings() {
    // Function parameters and all their uses share one local symbol.
    check_rename(
        "
            fun test(foo: int) {
                if (foo == 10) {
                    throw <caret>foo;
                }
            }
        ",
        "bar",
        expect![[r"
            fun test(bar: int) {
                if (bar == 10) {
                    throw bar;
                }
            }"]],
    );

    // The first catch binding is renamed independently.
    check_rename(
        "
            fun test() {
                try {} catch (error) {
                    val e = <caret>error as int;
                }
            }
        ",
        "err",
        expect![[r"
            fun test() {
                try {} catch (err) {
                    val e = err as int;
                }
            }"]],
    );

    // The second catch binding is indexed independently from the first.
    check_rename(
        "
            fun test() {
                try {} catch (error, <caret>data) {
                    val e = data as int;
                }
            }
        ",
        "d",
        expect![[r"
            fun test() {
                try {} catch (error, d) {
                    val e = d as int;
                }
            }"]],
    );
}

#[test]
fn renames_global_values_and_callable_declarations() {
    // Global variable references are updated across function bodies.
    check_rename(
        "
            global <caret>foo: int;
            fun test() { throw foo; }
            fun test2() { throw foo + 200; }
        ",
        "BAR",
        expect![[r"
            global BAR: int;
            fun test() { throw BAR; }
            fun test2() { throw BAR + 200; }"]],
    );

    // A function declaration and each call resolve to the same symbol.
    check_rename(
        "
            fun test() {}
            fun test2() {
                test();
                <caret>test();
                test();
            }
        ",
        "someFunction",
        expect![[r"
            fun someFunction() {}
            fun test2() {
                someFunction();
                someFunction();
                someFunction();
            }"]],
    );

    // Constants participate in value resolution like other globals.
    check_rename(
        "
            const <caret>FOO = 100;
            fun test() { throw FOO; }
        ",
        "BAR",
        expect![[r"
            const BAR = 100;
            fun test() { throw BAR; }"]],
    );
}

#[test]
fn renames_static_and_instance_methods() {
    // Static calls refer to the method declaration, not a same-named function.
    check_rename(
        "
            struct Foo {}
            fun Foo.<caret>test() {}
            fun test2() { Foo.test(); }
        ",
        "bar",
        expect![[r"
            struct Foo {}
            fun Foo.bar() {}
            fun test2() { Foo.bar(); }"]],
    );

    // Inferred instance calls resolve to the same method symbol.
    check_rename(
        "
            struct Foo {}
            fun Foo.<caret>test(self) {}
            fun test2() {
                val foo: Foo = {};
                foo.test();
            }
        ",
        "bar",
        expect![[r"
            struct Foo {}
            fun Foo.bar(self) {}
            fun test2() {
                val foo: Foo = {};
                foo.bar();
            }"]],
    );
}

#[test]
fn renames_types_and_receiver_references() {
    // Type aliases are renamed in field, parameter, and return annotations.
    check_rename(
        "
            type Int = int;
            struct Foo { field: <caret>Int }
            fun test(a: Int): Int {}
        ",
        "MyInt",
        expect![[r"
            type MyInt = int;
            struct Foo { field: MyInt }
            fun test(a: MyInt): MyInt {}"]],
    );

    // Struct names are updated in annotations and object literals.
    check_rename(
        "
            struct Foo { field: int }
            fun test(a: Foo): Foo {
                val foo: Foo = {};
                return <caret>Foo { field: 1 };
            }
        ",
        "Bar",
        expect![[r"
            struct Bar { field: int }
            fun test(a: Bar): Bar {
                val foo: Bar = {};
                return Bar { field: 1 };
            }"]],
    );

    // Method receiver declarations are references to their struct type.
    check_rename(
        "
            struct <caret>Storage {}
            fun Storage.load() { return Storage.fromCell(contract.getData()); }
            fun Storage.save(self) { contract.setData(self.toCell()); }
        ",
        "MyStorage",
        expect![[r"
            struct MyStorage {}
            fun MyStorage.load() { return MyStorage.fromCell(contract.getData()); }
            fun MyStorage.save(self) { contract.setData(self.toCell()); }"]],
    );
}

#[test]
fn renames_struct_fields_and_expands_shorthand_initializers() {
    // Field declarations, explicit initializers, and accesses share one symbol.
    check_rename(
        "
            struct Foo { <caret>field: int }
            fun test(a: Foo) {
                val foo: Foo = { field: 10 };
                foo.field;
                a.field;
            }
        ",
        "newField",
        expect![[r"
            struct Foo { newField: int }
            fun test(a: Foo) {
                val foo: Foo = { newField: 10 };
                foo.newField;
                a.newField;
            }"]],
    );

    // A cursor on the colon still selects the field immediately to its left.
    check_rename(
        "
            struct Foo { field<caret>: int }
            fun test(a: Foo) { a.field; }
        ",
        "newField",
        expect![[r"
            struct Foo { newField: int }
            fun test(a: Foo) { a.newField; }"]],
    );

    // Renaming a field preserves the value side of a shorthand initializer.
    check_rename(
        "
            struct Foo { <caret>field: int }
            fun test(field: int) {
                val foo: Foo = { field };
            }
        ",
        "newField",
        expect![[r"
            struct Foo { newField: int }
            fun test(field: int) {
                val foo: Foo = { newField: field };
            }"]],
    );

    // Renaming the local value preserves the field side of a shorthand initializer.
    check_rename(
        "
            struct Foo { field: int }
            fun test(<caret>field: int) {
                val foo: Foo = { field };
            }
        ",
        "value",
        expect![[r"
            struct Foo { field: int }
            fun test(value: int) {
                val foo: Foo = { field: value };
            }"]],
    );

    // A local declaration behaves like a parameter in a shorthand initializer.
    check_rename(
        "
            struct Foo { field: int }
            fun test() {
                val <caret>field = 0;
                val foo: Foo = { field };
            }
        ",
        "value",
        expect![[r"
            struct Foo { field: int }
            fun test() {
                val value = 0;
                val foo: Foo = { field: value };
            }"]],
    );
}

#[test]
fn renames_struct_field_from_explicit_object_literal_key() {
    let source = "
        struct MinPriceConfig {
            startMinPrice: int
            endMinPrice: int
        }

        fun getMinPriceConfig(domainCharCount: int): MinPriceConfig {
            var res = MinPriceConfig {
                startMinPrice: 10,
                <caret>endMinPrice: 1,
            };
        }
    ";

    check_rename(
        source,
        "minimumPrice",
        expect![[r"
            struct MinPriceConfig {
                startMinPrice: int
                minimumPrice: int
            }

            fun getMinPriceConfig(domainCharCount: int): MinPriceConfig {
                var res = MinPriceConfig {
                    startMinPrice: 10,
                    minimumPrice: 1,
                };
            }"]],
    );

    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = open_service(&uri, &marked);
    let prepare = service
        .prepare_rename(&uri, marked.marker("caret").position)
        .expect("prepare rename should succeed")
        .expect("object literal field should be renameable");
    let actual = format!(
        "{}:{}-{}:{} placeholder={}",
        prepare.range.start.line,
        prepare.range.start.character,
        prepare.range.end.line,
        prepare.range.end.character,
        prepare.placeholder,
    );

    expect!["8:8-8:19 placeholder=endMinPrice"].assert_eq(&actual);
}

#[test]
fn escapes_names_that_cannot_start_plain_identifiers() {
    check_rename("fun <caret>foo() {}", "1foo", expect!["fun `1foo`() {}"]);

    check_rename("fun <caret>foo() {}", "", expect!["fun ``() {}"]);
}

#[test]
fn renames_enums_and_members() {
    // Enum type references and qualifiers are all updated.
    check_rename(
        "
            enum <caret>Color { Red = 10, Blue = 20 }
            fun main() {
                val color: Color = Color.Red;
                match (color) { Color.Red => {} Color.Blue => {} }
            }
        ",
        "MyColor",
        expect![[r"
            enum MyColor { Red = 10, Blue = 20 }
            fun main() {
                val color: MyColor = MyColor.Red;
                match (color) { MyColor.Red => {} MyColor.Blue => {} }
            }"]],
    );

    // One member can be renamed without affecting sibling members.
    check_rename(
        "
            enum Color { <caret>Red = 10, Blue = 20 }
            fun main() {
                val color: Color = Color.Red;
                match (color) { Color.Red => {} Color.Blue => {} }
            }
        ",
        "MyRed",
        expect![[r"
            enum Color { MyRed = 10, Blue = 20 }
            fun main() {
                val color: Color = Color.MyRed;
                match (color) { Color.MyRed => {} Color.Blue => {} }
            }"]],
    );
}

#[test]
fn renames_method_type_parameters() {
    // Receiver type parameters are local to the method declaration.
    check_rename(
        "
            struct Foo<T> {}
            fun Foo<TName>.foo(): <caret>TName {}
        ",
        "TValue",
        expect![[r"
            struct Foo<T> {}
            fun Foo<TValue>.foo(): TValue {}"]],
    );

    // An implicit generic receiver is renamed from a return-type occurrence.
    check_rename(
        "fun T.foo(): <caret>T {}",
        "TName",
        expect!["fun TName.foo(): TName {}"],
    );

    // The receiver declaration itself is also a valid rename target.
    check_rename(
        "fun <caret>T.foo(): T {}",
        "TName",
        expect!["fun TName.foo(): TName {}"],
    );
}

#[test]
fn prepare_rename_reports_range_and_rejects_non_symbols() {
    let marked = MarkedSource::parse("struct Foo { field<caret>: int }");
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = open_service(&uri, &marked);
    let prepare = service
        .prepare_rename(&uri, marked.marker("caret").position)
        .expect("prepare rename should succeed")
        .expect("field should be renameable");
    let actual = format!(
        "{}:{}-{}:{} placeholder={}",
        prepare.range.start.line,
        prepare.range.start.character,
        prepare.range.end.line,
        prepare.range.end.character,
        prepare.placeholder,
    );
    expect!["0:13-0:18 placeholder=field"].assert_eq(&actual);

    let marked = MarkedSource::parse("fun test() { <caret>throw 10; }");
    let mut service = open_service(&uri, &marked);
    let prepare = service
        .prepare_rename(&uri, marked.marker("caret").position)
        .expect("prepare rename should succeed");
    expect!["false"].assert_eq(&prepare.is_some().to_string());

    let rename = service
        .rename(&uri, marked.marker("caret").position, "renamed")
        .expect("rename request should succeed");
    expect!["false"].assert_eq(&rename.is_some().to_string());
}

#[test]
fn rejects_standard_library_symbols() {
    let marked = MarkedSource::parse("fun test(): <caret>int { return 1; }");
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = open_service(&uri, &marked);
    let error = service
        .prepare_rename(&uri, marked.marker("caret").position)
        .expect_err("stdlib type should not be renameable");

    expect!["cannot rename an element from the Tolk standard library"]
        .assert_eq(&error.to_string());

    let marked = MarkedSource::parse("fun test() { <caret>minMax(); }");
    let mut service = open_service(&uri, &marked);
    let error = service
        .prepare_rename(&uri, marked.marker("caret").position)
        .expect_err("stdlib function should not be renameable");
    expect!["cannot rename an element from the Tolk standard library"]
        .assert_eq(&error.to_string());
}

#[test]
fn renames_imported_symbols_across_workspace_files() {
    let main = MarkedSource::parse(
        r#"
            import "lib"
            fun main() { <caret>helper(); }
        "#,
    );
    let main_uri = DocumentUri::from("file:///fixture/main.tolk");
    let lib_uri = DocumentUri::from("file:///fixture/lib.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .add_source_file(LANGUAGE_ID, lib_uri.clone(), "fun helper() {}")
        .expect("library source should be added");
    service
        .open_document(main_uri.clone(), LANGUAGE_ID, 1, main.source().to_owned())
        .expect("main document should open");
    let edit = service
        .rename(&main_uri, main.marker("caret").position, "renamedHelper")
        .expect("rename should succeed")
        .expect("rename should produce edits");
    let mut sources = BTreeMap::from([
        (main_uri.as_str(), main.source()),
        (lib_uri.as_str(), "fun helper() {}"),
    ]);
    let actual = edit
        .documents
        .iter()
        .map(|document| {
            let source = sources
                .remove(document.uri.as_str())
                .expect("edited document should have source text");
            format!(
                "{}\n{}",
                document.uri,
                apply_document_edits(source, document)
            )
        })
        .collect::<Vec<_>>()
        .join("\n---\n");

    expect![[r#"
        file:///fixture/lib.tolk
        fun renamedHelper() {}
        ---
        file:///fixture/main.tolk
        import "lib"
        fun main() { renamedHelper(); }"#]]
    .assert_eq(&actual);
}

#[test]
fn records_rename_profile_spans() {
    let marked = MarkedSource::parse("fun foo() {} fun main() { <caret>foo(); }");
    let uri = DocumentUri::from("file:///fixture/profiled.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source().to_owned())
        .expect("Tolk document should open");
    let prepare = service
        .prepare_rename(&uri, marked.marker("caret").position)
        .expect("prepare rename should succeed");
    let rename = service
        .rename(&uri, marked.marker("caret").position, "bar")
        .expect("rename should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "prepare={} rename={} prepare.span={} rename.span={} tolk.prepare={} tolk.rename={}",
        prepare.is_some(),
        rename.is_some(),
        event_count(summary, "rename.prepare"),
        event_count(summary, "rename"),
        event_count(summary, "tolk.rename.prepare"),
        event_count(summary, "tolk.rename"),
    );

    expect!["prepare=true rename=true prepare.span=1 rename.span=1 tolk.prepare=1 tolk.rename=1"]
        .assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
