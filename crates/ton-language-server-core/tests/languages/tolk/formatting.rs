#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, Range, TextIndex, WorkspaceConfig,
};

fn check_formatting(source: &str, range: Option<Range>, manifest: &str, expect: Expect) {
    let marked = MarkedSource::parse(source);
    check_formatting_exact(marked.source(), range, manifest, expect);
}

fn check_formatting_exact(source: &str, range: Option<Range>, manifest: &str, expect: Expect) {
    let uri = DocumentUri::from("file:///workspace/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    service
        .set_workspace_config(
            LANGUAGE_ID,
            WorkspaceConfig::new("file:///workspace", None, manifest),
        )
        .expect("workspace configuration should be accepted");
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, source.to_owned())
        .expect("Tolk document should open");

    let edits = service
        .formatting(&uri, range)
        .expect("formatting request should succeed");
    let formatted = edits.first().map_or_else(
        || source.to_owned(),
        |edit| {
            let index = TextIndex::new(source);
            let mut result = source.to_owned();
            let start = index.position_to_offset(source, edit.range.start);
            let end = index.position_to_offset(source, edit.range.end);
            result.replace_range(start..end, &edit.new_text);
            result
        },
    );

    expect.assert_eq(&format!("edits: {}\n{formatted}", edits.len()));
}

#[test]
fn formats_the_full_document() {
    check_formatting(
        r"
            fun main(){
            val value=1+2;
            }
        ",
        None,
        "",
        expect![[r#"
            edits: 1
            fun main() {
                val value = 1 + 2;
            }
        "#]],
    );
}

#[test]
fn formats_only_the_requested_range_after_unicode_text() {
    let marked = MarkedSource::parse(
        r#"
            fun main() {
                val untouched   =   "😀";
                <caret:start>val   target   =   1+2;<target:end>
            }
        "#,
    );
    let start = marked.marker("caret:start").position;
    let end = marked.marker("target:end").position;

    check_formatting(
        marked.source(),
        Some(Range::new(start, end)),
        "",
        expect![[r#"
            edits: 1
            fun main() {
                val untouched   =   "😀";
                val target = 1 + 2;
            }
        "#]],
    );
}

#[test]
fn returns_no_edits_for_formatted_source() {
    check_formatting_exact(
        concat!(
            r"fun main() {
    return;
}",
            "\n",
        ),
        None,
        "",
        expect![[r#"
            edits: 0
            fun main() {
                return;
            }
        "#]],
    );
}

#[test]
fn formatter_settings_do_not_rebuild_semantic_analysis() {
    let uri = DocumentUri::from("file:///workspace/main.tolk");
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .set_workspace_config(
            LANGUAGE_ID,
            WorkspaceConfig::new("file:///workspace", None, ""),
        )
        .expect("initial workspace configuration should be accepted");
    service
        .open_document(uri, LANGUAGE_ID, 1, "fun main() {}")
        .expect("Tolk document should open");
    let before = service
        .profiler()
        .summary()
        .events
        .iter()
        .filter(|event| event.name == "tolk.snapshot.rebuild")
        .count();

    service
        .set_workspace_config(
            LANGUAGE_ID,
            WorkspaceConfig::new(
                "file:///workspace",
                None,
                r"
                    [fmt]
                    width = 72
                    separate-import-groups = true
                ",
            ),
        )
        .expect("formatter workspace configuration should be accepted");
    let after = service
        .profiler()
        .summary()
        .events
        .iter()
        .filter(|event| event.name == "tolk.snapshot.rebuild")
        .count();

    expect![[r#"
        rebuilds before: 1
        rebuilds after: 1"#]]
    .assert_eq(&format!(
        "rebuilds before: {before}\nrebuilds after: {after}"
    ));
}
