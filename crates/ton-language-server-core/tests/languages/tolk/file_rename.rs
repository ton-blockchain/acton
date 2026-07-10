#![allow(clippy::needless_raw_string_hashes)]

use expect_test::{Expect, expect};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentEdits, DocumentUri, FileRename, LanguageService, LanguageServiceConfig, ProfileSummary,
    TextIndex,
};

fn service_with_files(files: &[(&str, &str)], open_uri: &str) -> LanguageService {
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    for (uri, source) in files {
        service
            .add_source_file(LANGUAGE_ID, *uri, *source)
            .expect("Tolk source should be added");
    }
    let source = files
        .iter()
        .find_map(|(uri, source)| (*uri == open_uri).then_some(*source))
        .expect("open file should be in fixture");
    service
        .open_document(open_uri, LANGUAGE_ID, 1, source)
        .expect("Tolk document should open");
    service
}

fn check_rename_edits(files: &[(&str, &str)], open_uri: &str, rename: FileRename, expect: Expect) {
    let mut service = service_with_files(files, open_uri);
    let edit = service
        .will_rename_files(&[rename])
        .expect("will rename files should succeed")
        .expect("file rename should update imports");
    let actual = edit
        .documents
        .iter()
        .map(|document| {
            let source = files
                .iter()
                .find_map(|(uri, source)| (*uri == document.uri.as_str()).then_some(*source))
                .expect("edited file should have source text");
            format!(
                "{}\n{}",
                document.uri,
                apply_document_edits(source, document)
            )
        })
        .collect::<Vec<_>>()
        .join("\n---\n");

    expect.assert_eq(&actual);
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
fn updates_importers_when_target_file_is_renamed() {
    check_rename_edits(
        &[
            (
                "file:///fixture/main.tolk",
                "import \"lib/old\"\nfun main() { helper(); }",
            ),
            ("file:///fixture/lib/old.tolk", "fun helper() {}"),
        ],
        "file:///fixture/main.tolk",
        FileRename::new(
            DocumentUri::from("file:///fixture/lib/old.tolk"),
            DocumentUri::from("file:///fixture/lib/new.tolk"),
        ),
        expect![[r#"
            file:///fixture/main.tolk
            import "lib/new"
            fun main() { helper(); }"#]],
    );
}

#[test]
fn updates_relative_imports_inside_a_moved_file() {
    check_rename_edits(
        &[
            (
                "file:///fixture/main.tolk",
                "import \"lib/helper\"\nfun main() { helper(); }",
            ),
            ("file:///fixture/lib/helper.tolk", "fun helper() {}"),
        ],
        "file:///fixture/main.tolk",
        FileRename::new(
            DocumentUri::from("file:///fixture/main.tolk"),
            DocumentUri::from("file:///fixture/contracts/main.tolk"),
        ),
        expect![[r#"
            file:///fixture/main.tolk
            import "../lib/helper"
            fun main() { helper(); }"#]],
    );
}

#[test]
fn ignores_non_tolk_renames_and_unchanged_stdlib_imports() {
    let files = [(
        "file:///fixture/main.tolk",
        "import \"@stdlib/common\"\nfun main() {}",
    )];
    let mut service = service_with_files(&files, "file:///fixture/main.tolk");
    let edit = service
        .will_rename_files(&[FileRename::new(
            DocumentUri::from("file:///fixture/readme.md"),
            DocumentUri::from("file:///fixture/docs/readme.md"),
        )])
        .expect("non-Tolk rename should succeed");
    expect!["false"].assert_eq(&edit.is_some().to_string());

    let edit = service
        .will_rename_files(&[FileRename::new(
            DocumentUri::from("file:///fixture/main.tolk"),
            DocumentUri::from("file:///fixture/contracts/main.tolk"),
        )])
        .expect("Tolk rename should succeed");
    expect!["false"].assert_eq(&edit.is_some().to_string());
}

#[test]
fn skips_unresolved_imports_and_updates_the_matching_later_import() {
    let files = [
        (
            "file:///fixture/main.tolk",
            "import \"missing\"\nimport \"old\"\nfun main() { helper(); }",
        ),
        ("file:///fixture/old.tolk", "fun helper() {}"),
    ];
    let mut service = service_with_files(&files, "file:///fixture/main.tolk");

    let unrelated = service
        .will_rename_files(&[FileRename::new(
            DocumentUri::from("file:///fixture/unrelated.tolk"),
            DocumentUri::from("file:///fixture/renamed-unrelated.tolk"),
        )])
        .expect("unrelated Tolk rename should succeed");
    service
        .did_rename_files(&[
            FileRename::new(
                DocumentUri::from("file:///fixture/readme.md"),
                DocumentUri::from("file:///fixture/renamed-readme.md"),
            ),
            FileRename::new(
                DocumentUri::from("file:///fixture/unrelated.tolk"),
                DocumentUri::from("file:///fixture/renamed-unrelated.tolk"),
            ),
        ])
        .expect("untracked rename notifications should be ignored");
    let edit = service
        .will_rename_files(&[FileRename::new(
            DocumentUri::from("file:///fixture/old.tolk"),
            DocumentUri::from("file:///fixture/new.tolk"),
        )])
        .expect("target rename should succeed")
        .expect("target rename should update its import");
    let updated = apply_document_edits(files[0].1, &edit.documents[0]);
    let actual = format!("unrelated={}\n{updated}", unrelated.is_some());

    expect![[r#"
        unrelated=false
        import "missing"
        import "new"
        fun main() { helper(); }"#]]
    .assert_eq(&actual);
}

#[test]
fn moves_open_only_and_provider_only_files() {
    // Open-only documents have no provider URI to update.
    let old_open_uri = DocumentUri::from("file:///fixture/open-old.tolk");
    let new_open_uri = DocumentUri::from("file:///fixture/open-new.tolk");
    let mut open_service = LanguageService::new(LanguageServiceConfig::default());
    open_service.register_language(TolkLanguage::new());
    open_service
        .open_document(old_open_uri.clone(), LANGUAGE_ID, 1, "fun OpenOnly() {}")
        .expect("open-only document should open");
    open_service
        .did_rename_files(&[FileRename::new(old_open_uri, new_open_uri.clone())])
        .expect("open-only document should move");
    let open_symbol = open_service
        .document_symbols(&new_open_uri)
        .expect("renamed open-only document should remain addressable");

    // Provider-only files have no open-document URI to update.
    let old_provider_uri = DocumentUri::from("file:///fixture/provider-old.tolk");
    let new_provider_uri = DocumentUri::from("file:///fixture/provider-new.tolk");
    let mut provider_service = LanguageService::new(LanguageServiceConfig::default());
    provider_service.register_language(TolkLanguage::new());
    provider_service
        .add_source_file(
            LANGUAGE_ID,
            old_provider_uri.clone(),
            "fun ProviderOnly() {}",
        )
        .expect("provider-only source should be added");
    provider_service
        .did_rename_files(&[FileRename::new(old_provider_uri, new_provider_uri)])
        .expect("provider-only source should move");
    let provider_symbol = provider_service
        .workspace_symbols("ProviderOnly")
        .expect("renamed provider-only source should remain indexed");

    let actual = format!(
        "open={} provider={} uri={}",
        open_symbol
            .first()
            .map_or("<none>", |symbol| symbol.name.as_str()),
        provider_symbol
            .first()
            .map_or("<none>", |symbol| symbol.name.as_str()),
        provider_symbol
            .first()
            .map_or("<none>", |symbol| symbol.location.uri.as_str()),
    );
    expect!["open=OpenOnly provider=ProviderOnly uri=file:///fixture/provider-new.tolk"]
        .assert_eq(&actual);
}

#[test]
fn moves_open_document_state_after_rename_notification() {
    let old_uri = DocumentUri::from("file:///fixture/old.tolk");
    let new_uri = DocumentUri::from("file:///fixture/new.tolk");
    let mut service = service_with_files(
        &[(old_uri.as_str(), "fun RenamedDocumentSymbol() {}")],
        old_uri.as_str(),
    );
    service
        .did_rename_files(&[FileRename::new(old_uri.clone(), new_uri.clone())])
        .expect("did rename files should succeed");
    let symbols = service
        .document_symbols(&new_uri)
        .expect("renamed open document should remain addressable");
    let old_error = service
        .document_symbols(&old_uri)
        .expect_err("old document URI should no longer be open");
    let actual = format!(
        "new={} old={}",
        symbols
            .first()
            .map_or("<none>", |symbol| symbol.name.as_str()),
        old_error,
    );

    expect!["new=RenamedDocumentSymbol old=document not open: file:///fixture/old.tolk"]
        .assert_eq(&actual);
}

#[test]
fn records_file_rename_profile_spans() {
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .add_source_file(
            LANGUAGE_ID,
            "file:///fixture/main.tolk",
            "import \"old\"\nfun main() {}",
        )
        .expect("main source should be added");
    service
        .add_source_file(LANGUAGE_ID, "file:///fixture/old.tolk", "fun helper() {}")
        .expect("target source should be added");
    service
        .open_document(
            "file:///fixture/main.tolk",
            LANGUAGE_ID,
            1,
            "import \"old\"\nfun main() {}",
        )
        .expect("main document should open");
    let rename = FileRename::new(
        DocumentUri::from("file:///fixture/old.tolk"),
        DocumentUri::from("file:///fixture/new.tolk"),
    );
    let edit = service
        .will_rename_files(std::slice::from_ref(&rename))
        .expect("will rename should succeed");
    service
        .did_rename_files(&[rename])
        .expect("did rename should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "edit={} prepare={} tolk.prepare={} did={}",
        edit.is_some(),
        event_count(summary, "files.rename.prepare"),
        event_count(summary, "tolk.files.rename.prepare"),
        summary.counters.get("files.rename").copied().unwrap_or(0),
    );

    expect!["edit=true prepare=1 tolk.prepare=1 did=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
