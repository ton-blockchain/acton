#![allow(clippy::needless_raw_string_hashes)]

use expect_test::{Expect, expect};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    LanguageService, LanguageServiceConfig, ProfileSummary, WorkspaceSymbol,
};

fn render_symbols(symbols: &[WorkspaceSymbol]) -> String {
    if symbols.is_empty() {
        return "<none>".to_owned();
    }

    symbols
        .iter()
        .map(|symbol| {
            format!(
                "{} ({:?}) {} {}:{}",
                symbol.name,
                symbol.kind,
                symbol.location.uri,
                symbol.location.range.start.line,
                symbol.location.range.start.character,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn case_workspace_symbols(files: &[(&str, &str)], query: &str, expect: Expect) {
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TolkLanguage::new());
    for (uri, source) in files {
        service
            .add_source_file(LANGUAGE_ID, *uri, *source)
            .expect("Tolk workspace source should be added");
    }
    let symbols = service
        .workspace_symbols(query)
        .expect("workspace symbol request should succeed");

    expect.assert_eq(&render_symbols(&symbols));
}

#[test]
fn finds_all_supported_top_level_symbol_kinds() {
    case_workspace_symbols(
        &[(
            "file:///fixture/main.tolk",
            r"global WorkspaceSearchGlobal: int;
fun WorkspaceSearchFunction() {}
struct WorkspaceSearchStruct { WorkspaceSearchField: int }
fun WorkspaceSearchStruct.WorkspaceSearchMethod() {}
get WorkspaceSearchGetter(): int {}
enum WorkspaceSearchEnum { WorkspaceSearchMember }
type WorkspaceSearchAlias = int;
const WorkspaceSearchConstant = 1;",
        )],
        "workspacesearch",
        expect![[r"
            WorkspaceSearchAlias (TypeParameter) file:///fixture/main.tolk 6:5
            WorkspaceSearchConstant (Constant) file:///fixture/main.tolk 7:6
            WorkspaceSearchEnum (Enum) file:///fixture/main.tolk 5:5
            WorkspaceSearchEnum.WorkspaceSearchMember (EnumMember) file:///fixture/main.tolk 5:27
            WorkspaceSearchFunction (Function) file:///fixture/main.tolk 1:4
            WorkspaceSearchGlobal (Variable) file:///fixture/main.tolk 0:7
            WorkspaceSearchStruct (Struct) file:///fixture/main.tolk 2:7
            WorkspaceSearchStruct.WorkspaceSearchMethod (Method) file:///fixture/main.tolk 3:26
            get WorkspaceSearchGetter (Event) file:///fixture/main.tolk 4:4"]],
    );
}

#[test]
fn searches_case_insensitively_across_workspace_files() {
    case_workspace_symbols(
        &[
            (
                "file:///fixture/one.tolk",
                "fun CrossFileWorkspaceNeedleOne() {}",
            ),
            (
                "file:///fixture/two.tolk",
                "fun CrossFileWorkspaceNeedleTwo() {}",
            ),
        ],
        "workspaceneedle",
        expect![[r"
            CrossFileWorkspaceNeedleOne (Function) file:///fixture/one.tolk 0:4
            CrossFileWorkspaceNeedleTwo (Function) file:///fixture/two.tolk 0:4"]],
    );
}

#[test]
fn excludes_struct_fields_and_non_matching_symbols() {
    case_workspace_symbols(
        &[(
            "file:///fixture/main.tolk",
            "struct Container { WorkspaceFieldOnlyNeedle: int } fun Other() {}",
        )],
        "WorkspaceFieldOnlyNeedle",
        expect!["<none>"],
    );
}

#[test]
fn records_workspace_symbol_profile_spans() {
    let mut service = LanguageService::new(LanguageServiceConfig {
        enable_profiling: true,
    });
    service.register_language(TolkLanguage::new());
    service
        .add_source_file(
            LANGUAGE_ID,
            "file:///fixture/main.tolk",
            "fun ProfiledWorkspaceSymbol() {}",
        )
        .expect("Tolk source should be added");
    let symbols = service
        .workspace_symbols("ProfiledWorkspaceSymbol")
        .expect("workspace symbols should succeed");
    let summary = service.profiler().summary();
    let actual = format!(
        "symbols={} workspace_symbols={} tolk.workspace_symbols={}",
        symbols.len(),
        event_count(summary, "workspace_symbols"),
        event_count(summary, "tolk.workspace_symbols"),
    );

    expect!["symbols=1 workspace_symbols=1 tolk.workspace_symbols=1"].assert_eq(&actual);
}

fn event_count(summary: &ProfileSummary, name: &'static str) -> usize {
    summary
        .events
        .iter()
        .filter(|event| event.name == name)
        .count()
}
