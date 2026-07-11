#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::expect;
use std::fmt::{Display, Formatter};
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, ProfileSummary,
};

#[test]
fn relocates_unchanged_inference_results_after_an_earlier_body_edit() {
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    let initial = MarkedSource::parse(
        "
            struct Storage {
                counter: int
            }

            fun first(): int { return 1; }

            fun shifted(storage: Storage): int {
                return storage.counter;
            }
        ",
    );
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, initial.source().to_owned())
        .expect("Tolk document should open");

    let before_growth = AnalysisCounters::capture(service.profiler().summary());
    let changed = MarkedSource::parse(
        "
            struct Storage {
                counter: int
            }

            fun first(): int { return 100000; }

            fun shifted(storage: Storage): int {
                return storage.<caret>counter;
            }
        ",
    );
    service
        .change_document(&uri, 2, changed.source().to_owned())
        .expect("Tolk document should change");

    let definitions = service
        .definition(&uri, changed.marker("caret").position)
        .expect("definition request should succeed");
    let type_at_position = service
        .type_at_position(&uri, changed.marker("caret").position)
        .expect("type-at-position request should succeed")
        .expect("shifted field should remain typed");
    let after_growth = AnalysisCounters::capture(service.profiler().summary());
    let growth = after_growth.delta(before_growth);

    let shrunk = MarkedSource::parse(
        "
            struct Storage {
                counter: int
            }

            fun first(): int { return 1; }

            fun shifted(storage: Storage): int {
                return storage.<caret>counter;
            }
        ",
    );
    service
        .change_document(&uri, 3, shrunk.source().to_owned())
        .expect("Tolk document should change");
    let definitions_after_shrink = service
        .definition(&uri, shrunk.marker("caret").position)
        .expect("definition request should succeed");
    let type_after_shrink = service
        .type_at_position(&uri, shrunk.marker("caret").position)
        .expect("type-at-position request should succeed")
        .expect("shifted field should remain typed");
    let shrink = AnalysisCounters::capture(service.profiler().summary()).delta(after_growth);

    let actual = format!(
        "growth: {growth}
growth result: definitions={} target={}:{} type={}
shrink: {shrink}
shrink result: definitions={} target={}:{} type={}
",
        definitions.len(),
        definitions[0].range.start.line,
        definitions[0].range.start.character,
        type_at_position.type_name,
        definitions_after_shrink.len(),
        definitions_after_shrink[0].range.start.line,
        definitions_after_shrink[0].range.start.character,
        type_after_shrink.type_name,
    );

    expect![[r#"
        growth: signatures=0 files=1 declarations=1 relocated=1 fallback=0
        growth result: definitions=1 target=1:4 type=int
        shrink: signatures=0 files=1 declarations=1 relocated=1 fallback=0
        shrink result: definitions=1 target=1:4 type=int
    "#]]
    .assert_eq(&actual);
}

#[test]
fn invalidates_dependents_only_when_an_inferred_return_type_changes() {
    let lib_uri = DocumentUri::from("file:///fixture/lib.tolk");
    let main_uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = profiling_service();
    service
        .open_document(
            lib_uri.clone(),
            LANGUAGE_ID,
            1,
            "fun helper() { return 1; }",
        )
        .expect("library should open");
    let main = MarkedSource::parse(
        r#"
            import "lib"

            fun main(): int {
                val <caret>result = helper();
                return result;
            }
        "#,
    );
    service
        .open_document(main_uri.clone(), LANGUAGE_ID, 1, main.source().to_owned())
        .expect("main file should open");

    let before_same_type = AnalysisCounters::capture(service.profiler().summary());
    service
        .change_document(&lib_uri, 2, "fun helper() { return 2; }")
        .expect("library should change");
    let after_same_type = AnalysisCounters::capture(service.profiler().summary());
    let same_type = service
        .type_at_position(&main_uri, main.marker("caret").position)
        .expect("type-at-position request should succeed")
        .expect("result should remain typed");

    service
        .change_document(&lib_uri, 3, "fun helper() { return true; }")
        .expect("library should change");
    let after_changed_type = AnalysisCounters::capture(service.profiler().summary());
    let changed_type = service
        .type_at_position(&main_uri, main.marker("caret").position)
        .expect("type-at-position request should succeed")
        .expect("result should remain typed");

    let actual = format!(
        "same type: {} result={}
changed type: {} result={}
",
        after_same_type.delta(before_same_type),
        same_type.type_name,
        after_changed_type.delta(after_same_type),
        changed_type.type_name,
    );
    expect![[r#"
        same type: signatures=0 files=1 declarations=1 relocated=0 fallback=0 result=int
        changed type: signatures=2 files=3 declarations=3 relocated=0 fallback=1 result=bool
    "#]]
    .assert_eq(&actual);
}

#[test]
fn skips_inference_for_trivia_outside_declarations() {
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = profiling_service();
    let initial = MarkedSource::parse(
        "
            fun main(): int {
                val value = 1;
                return value;
            }

            // before
        ",
    );
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, initial.source().to_owned())
        .expect("Tolk document should open");
    let before = AnalysisCounters::capture(service.profiler().summary());
    let changed = MarkedSource::parse(
        "
            fun main(): int {
                val <caret>value = 1;
                return value;
            }

            // after, with a different length
        ",
    );
    service
        .change_document(&uri, 2, changed.source().to_owned())
        .expect("Tolk document should change");
    let ty = service
        .type_at_position(&uri, changed.marker("caret").position)
        .expect("type-at-position request should succeed")
        .expect("cached declaration should remain typed");
    let delta = AnalysisCounters::capture(service.profiler().summary()).delta(before);

    expect!["signatures=0 files=0 declarations=0 relocated=0 fallback=0 result=int"]
        .assert_eq(&format!("{delta} result={}", ty.type_name));
}

#[test]
fn reanalyzes_the_current_file_when_its_import_target_changes() {
    let lib_uri = DocumentUri::from("file:///fixture/lib.tolk");
    let alt_uri = DocumentUri::from("file:///fixture/alt.tolk");
    let main_uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = profiling_service();
    service
        .open_document(lib_uri, LANGUAGE_ID, 1, "fun helper(): int { return 1; }")
        .expect("first library should open");
    service
        .open_document(
            alt_uri,
            LANGUAGE_ID,
            1,
            "fun helper(): bool { return true; }",
        )
        .expect("second library should open");
    let initial = MarkedSource::parse(
        r#"
            import "lib"

            fun main(): int {
                val <caret>result = helper();
                return 0;
            }
        "#,
    );
    service
        .open_document(
            main_uri.clone(),
            LANGUAGE_ID,
            1,
            initial.source().to_owned(),
        )
        .expect("main file should open");
    let before = AnalysisCounters::capture(service.profiler().summary());
    let changed = MarkedSource::parse(
        r#"
            import "alt"

            fun main(): int {
                val <caret>result = helper();
                return 0;
            }
        "#,
    );
    service
        .change_document(&main_uri, 2, changed.source().to_owned())
        .expect("main file should change");
    let ty = service
        .type_at_position(&main_uri, changed.marker("caret").position)
        .expect("type-at-position request should succeed")
        .expect("result should remain typed");
    let delta = AnalysisCounters::capture(service.profiler().summary()).delta(before);

    expect!["signatures=1 files=1 declarations=1 relocated=0 fallback=0 result=bool"]
        .assert_eq(&format!("{delta} result={}", ty.type_name));
}

#[test]
fn reanalyzes_trivia_changes_inside_a_declaration_to_refresh_spans() {
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = profiling_service();
    let initial = MarkedSource::parse(
        "
            struct Storage {
                counter: int
            }

            fun read(storage: Storage): int {
                return storage.counter;
            }
        ",
    );
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, initial.source().to_owned())
        .expect("Tolk document should open");
    let before = AnalysisCounters::capture(service.profiler().summary());
    let changed = MarkedSource::parse(
        "
            struct Storage {
                counter: int
            }

            fun read(storage: Storage): int {
                // Shift every expression below without changing the signature.
                return storage.<caret>counter;
            }
        ",
    );
    service
        .change_document(&uri, 2, changed.source().to_owned())
        .expect("Tolk document should change");
    let definitions = service
        .definition(&uri, changed.marker("caret").position)
        .expect("definition request should succeed");
    let delta = AnalysisCounters::capture(service.profiler().summary()).delta(before);
    let actual = format!(
        "{delta} definitions={} target={}:{}",
        definitions.len(),
        definitions[0].range.start.line,
        definitions[0].range.start.character,
    );

    expect![[r#"
        signatures=0 files=1 declarations=1 relocated=0 fallback=0 definitions=1 target=1:4"#]]
    .assert_eq(&actual);
}

#[test]
fn rebuilds_the_file_when_top_level_symbols_are_added_or_removed() {
    let uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = profiling_service();
    let initial = MarkedSource::parse(
        "
            fun existing(): int { return 1; }
            fun main(): int { return existing(); }
        ",
    );
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, initial.source().to_owned())
        .expect("Tolk document should open");
    let before_add = AnalysisCounters::capture(service.profiler().summary());
    let added = MarkedSource::parse(
        "
            fun inserted(): int { return 2; }
            fun existing(): int { return 1; }
            fun main(): int { return <caret>existing(); }
        ",
    );
    service
        .change_document(&uri, 2, added.source().to_owned())
        .expect("Tolk document should change");
    let definition_after_add = service
        .definition(&uri, added.marker("caret").position)
        .expect("definition request should succeed");
    let after_add = AnalysisCounters::capture(service.profiler().summary());

    let removed = MarkedSource::parse(
        "
            fun existing(): int { return 1; }
            fun main(): int { return <caret>existing(); }
        ",
    );
    service
        .change_document(&uri, 3, removed.source().to_owned())
        .expect("Tolk document should change");
    let definition_after_remove = service
        .definition(&uri, removed.marker("caret").position)
        .expect("definition request should succeed");
    let after_remove = AnalysisCounters::capture(service.profiler().summary());

    let actual = format!(
        "add: {} definitions={} target={}:{}
remove: {} definitions={} target={}:{}",
        after_add.delta(before_add),
        definition_after_add.len(),
        definition_after_add[0].range.start.line,
        definition_after_add[0].range.start.character,
        after_remove.delta(after_add),
        definition_after_remove.len(),
        definition_after_remove[0].range.start.line,
        definition_after_remove[0].range.start.character,
    );
    expect![[r#"
        add: signatures=1 files=1 declarations=3 relocated=0 fallback=0 definitions=1 target=1:4
        remove: signatures=1 files=1 declarations=2 relocated=0 fallback=0 definitions=1 target=0:4"#]]
    .assert_eq(&actual);
}

#[test]
fn propagates_changed_inferred_types_through_transitive_importers() {
    let leaf_uri = DocumentUri::from("file:///fixture/leaf.tolk");
    let middle_uri = DocumentUri::from("file:///fixture/middle.tolk");
    let main_uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = profiling_service();
    service
        .open_document(
            leaf_uri.clone(),
            LANGUAGE_ID,
            1,
            "fun helper() { return 1; }",
        )
        .expect("leaf file should open");
    let middle = MarkedSource::parse(
        r#"
            import "leaf"

            fun middle() { return helper(); }
        "#,
    );
    service
        .open_document(middle_uri, LANGUAGE_ID, 1, middle.source().to_owned())
        .expect("middle file should open");
    let main = MarkedSource::parse(
        r#"
            import "middle"

            fun main(): int {
                val <caret>result = middle();
                return 0;
            }
        "#,
    );
    service
        .open_document(main_uri.clone(), LANGUAGE_ID, 1, main.source().to_owned())
        .expect("main file should open");
    let before = AnalysisCounters::capture(service.profiler().summary());

    service
        .change_document(&leaf_uri, 2, "fun helper() { return true; }")
        .expect("leaf file should change");
    let ty = service
        .type_at_position(&main_uri, main.marker("caret").position)
        .expect("type-at-position request should succeed")
        .expect("transitive result should remain typed");
    let delta = AnalysisCounters::capture(service.profiler().summary()).delta(before);

    expect!["signatures=3 files=4 declarations=4 relocated=0 fallback=1 result=bool"]
        .assert_eq(&format!("{delta} result={}", ty.type_name));
}

#[test]
fn reuses_the_project_index_only_while_its_graph_shape_is_stable() {
    let lib_uri = DocumentUri::from("file:///fixture/lib.tolk");
    let alt_uri = DocumentUri::from("file:///fixture/alt.tolk");
    let main_uri = DocumentUri::from("file:///fixture/main.tolk");
    let mut service = profiling_service();
    service
        .open_document(lib_uri, LANGUAGE_ID, 1, "fun helper(): int { return 1; }")
        .expect("first library should open");
    service
        .open_document(
            alt_uri,
            LANGUAGE_ID,
            1,
            "fun alternative(): int { return 2; }",
        )
        .expect("second library should open");
    let initial = MarkedSource::parse(
        r#"
            import "lib"

            fun main(): int {
                return helper();
            }
        "#,
    );
    service
        .open_document(
            main_uri.clone(),
            LANGUAGE_ID,
            1,
            initial.source().to_owned(),
        )
        .expect("main file should open");

    let before_body = IndexCounters::capture(service.profiler().summary());
    let body_changed = MarkedSource::parse(
        r#"
            import "lib"

            fun main(): int {
                return helper() + 1;
            }
        "#,
    );
    service
        .change_document(&main_uri, 2, body_changed.source().to_owned())
        .expect("body should change");
    let after_body = IndexCounters::capture(service.profiler().summary());

    let signature_changed = MarkedSource::parse(
        r#"
            import "lib"

            fun main(unused: int): int {
                return helper() + unused;
            }
        "#,
    );
    service
        .change_document(&main_uri, 3, signature_changed.source().to_owned())
        .expect("signature should change");
    let after_signature = IndexCounters::capture(service.profiler().summary());

    let incomplete = MarkedSource::parse(
        r#"
            import "lib"

            fun main(unused: int): int {
                return helper() +
            }
        "#,
    );
    service
        .change_document(&main_uri, 4, incomplete.source().to_owned())
        .expect("incomplete source should remain analyzable");
    let after_incomplete = IndexCounters::capture(service.profiler().summary());

    let import_changed = MarkedSource::parse(
        r#"
            import "alt"

            fun main(unused: int): int {
                return alternative() + unused;
            }
        "#,
    );
    service
        .change_document(&main_uri, 5, import_changed.source().to_owned())
        .expect("import should change");
    let after_import = IndexCounters::capture(service.profiler().summary());

    let declaration_added = MarkedSource::parse(
        r#"
            import "alt"

            fun added(): int { return 1; }

            fun main(unused: int): int {
                return alternative() + unused;
            }
        "#,
    );
    service
        .change_document(&main_uri, 6, declaration_added.source().to_owned())
        .expect("declaration should be added");
    let after_declaration = IndexCounters::capture(service.profiler().summary());

    let actual = format!(
        "body: {}\nsignature: {}\nincomplete: {}\nimport: {}\ndeclaration: {}\n",
        after_body.delta(before_body),
        after_signature.delta(after_body),
        after_incomplete.delta(after_signature),
        after_import.delta(after_incomplete),
        after_declaration.delta(after_import),
    );
    expect![[r#"
        body: incremental=1 full=0
        signature: incremental=1 full=0
        incomplete: incremental=1 full=0
        import: incremental=0 full=1
        declaration: incremental=0 full=1
    "#]]
    .assert_eq(&actual);
}

fn profiling_service() -> LanguageService {
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
}

#[derive(Clone, Copy)]
struct AnalysisCounters {
    signatures: u64,
    files: u64,
    declarations: u64,
    relocated: u64,
    fallback: u64,
}

impl AnalysisCounters {
    fn capture(summary: &ProfileSummary) -> Self {
        Self {
            signatures: counter(summary, "tolk.type_signature.file"),
            files: counter(summary, "tolk.type_inference.file"),
            declarations: counter(summary, "tolk.type_inference.declaration"),
            relocated: counter(summary, "tolk.type_inference.relocated_declaration"),
            fallback: counter(summary, "tolk.type_inference.signature_fallback"),
        }
    }

    const fn delta(self, previous: Self) -> Self {
        Self {
            signatures: self.signatures - previous.signatures,
            files: self.files - previous.files,
            declarations: self.declarations - previous.declarations,
            relocated: self.relocated - previous.relocated,
            fallback: self.fallback - previous.fallback,
        }
    }
}

impl Display for AnalysisCounters {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "signatures={} files={} declarations={} relocated={} fallback={}",
            self.signatures, self.files, self.declarations, self.relocated, self.fallback,
        )
    }
}

#[derive(Clone, Copy)]
struct IndexCounters {
    incremental: u64,
    full: u64,
}

impl IndexCounters {
    fn capture(summary: &ProfileSummary) -> Self {
        Self {
            incremental: counter(summary, "tolk.snapshot.index.incremental"),
            full: span_count(summary, "tolk.snapshot.index.full"),
        }
    }

    const fn delta(self, previous: Self) -> Self {
        Self {
            incremental: self.incremental - previous.incremental,
            full: self.full - previous.full,
        }
    }
}

impl Display for IndexCounters {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "incremental={} full={}",
            self.incremental, self.full,
        )
    }
}

fn counter(summary: &ProfileSummary, name: &'static str) -> u64 {
    summary.counters.get(name).copied().unwrap_or_default()
}

fn span_count(summary: &ProfileSummary, name: &'static str) -> u64 {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count() as u64
}
