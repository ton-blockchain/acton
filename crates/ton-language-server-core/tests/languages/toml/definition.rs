#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support.rs"]
mod support;

use expect_test::{Expect, expect};
use std::fs;
use tempfile::tempdir;
use ton_language_server_core::languages::toml::{LANGUAGE_ID, TomlLanguage};
use ton_language_server_core::{DocumentUri, LanguageService, LanguageServiceConfig, Location};

fn check_definition(source: &str, relative_file: &str, expected: Expect) {
    let (workspace, locations) = definition_locations(source, relative_file, TargetKind::File);
    expected.assert_eq(&render_locations(&locations, workspace.path()));
}

fn definition_locations(
    source: &str,
    relative_path: &str,
    target_kind: TargetKind,
) -> (tempfile::TempDir, Vec<Location>) {
    let marked = support::MarkedSource::parse(source);
    let workspace = tempdir().expect("temporary workspace should be created");
    let target = workspace.path().join(relative_path);
    match target_kind {
        TargetKind::File => {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).expect("target parent should be created");
            }
            fs::write(&target, "").expect("target should be created");
        }
        TargetKind::Directory => {
            fs::create_dir_all(&target).expect("target directory should be created");
        }
    }

    let manifest = workspace.path().join("Acton.toml");
    let manifest_uri = url::Url::from_file_path(&manifest)
        .expect("manifest URI should be created")
        .to_string();
    let uri = DocumentUri::from(manifest_uri);
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TomlLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source())
        .expect("TOML document should open");

    let locations = service
        .definition(&uri, marked.marker("caret").position)
        .expect("definition request should succeed");
    (workspace, locations)
}

fn render_locations(locations: &[Location], workspace: &std::path::Path) -> String {
    let root_uri = url::Url::from_directory_path(workspace)
        .expect("workspace URI should be created")
        .to_string();
    locations
        .iter()
        .map(|location| {
            format!(
                "{} {}",
                location
                    .uri
                    .as_str()
                    .replace(root_uri.trim_end_matches('/'), "file:///$ROOT"),
                location.range.start.line
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn resolves_contract_source_relative_to_manifest() {
    check_definition(
        r#"
            [contracts.counter]
            src = "<caret>./contracts/../contracts/counter.tolk"
        "#,
        "contracts/counter.tolk",
        expect![[r"
            file:///$ROOT/contracts/counter.tolk 0"]],
    );
}

#[test]
fn resolves_mapping_and_dependency_paths() {
    check_definition(
        r#"
            [import-mappings]
            "@contracts" = "<caret>contracts"
        "#,
        "contracts",
        expect![[r"
            file:///$ROOT/contracts 0"]],
    );

    check_definition(
        r#"
            [contracts.app]
            depends = [{ name = "common", path = "<caret>deps/common.tolk" }]
        "#,
        "deps/common.tolk",
        expect![[r"
            file:///$ROOT/deps/common.tolk 0"]],
    );
}

#[test]
fn resolves_nested_and_global_path_fields() {
    check_definition(
        r#"
            [contracts.counter]
            src = "contracts/counter.tolk"
            wrappers = { tolk = { output-dir = "<caret>generated/wrappers" } }
        "#,
        "generated/wrappers",
        expect![[r"
            file:///$ROOT/generated/wrappers 0"]],
    );
}

#[test]
fn resolves_directory_path() {
    let (workspace, locations) = definition_locations(
        r#"
            [import-mappings]
            "@contracts" = "<caret>contracts"
        "#,
        "contracts",
        TargetKind::Directory,
    );
    expect![[r"
        file:///$ROOT/contracts 0"]]
    .assert_eq(&render_locations(&locations, workspace.path()));
}

#[test]
fn origin_selection_range_covers_string_content() {
    let (_workspace, locations) = definition_locations(
        r#"
            [contracts.counter]
            src = "<caret>contracts/counter.tolk"
        "#,
        "contracts/counter.tolk",
        TargetKind::File,
    );
    expect![[r"1:7-1:29"]].assert_eq(&render_origin_ranges(&locations));
}

#[test]
fn resolves_test_output_path() {
    check_definition(
        r#"
            [test.coverage]
            output-file = "coverage/<caret>lcov.info"
        "#,
        "coverage/lcov.info",
        expect![[r"
            file:///$ROOT/coverage/lcov.info 0"]],
    );
}

#[test]
fn ignores_non_path_values() {
    check_definition(
        r#"
            [package]
            name = "<caret>counter"
        "#,
        "package.json",
        expect![""],
    );
}

fn render_origin_ranges(locations: &[Location]) -> String {
    locations
        .iter()
        .filter_map(|location| location.origin_selection_range)
        .map(|range| {
            format!(
                "{}:{}-{}:{}",
                range.start.line, range.start.character, range.end.line, range.end.character
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Copy)]
enum TargetKind {
    File,
    Directory,
}
