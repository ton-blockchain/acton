mod exclusions;

use exclusions::{
    GLOBAL_RESOLUTION_EXCLUSIONS, RESOLUTION_EXCLUSIONS, SYNTAX_EXCLUSIONS,
    TYPE_EXPECTATION_EXCLUSIONS,
};
use expect_test::expect;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use tolk_syntax::{Annotation, Call, Expr, TryFromNode};
use ton_language_server_core::languages::tolk::{LANGUAGE_ID, TolkLanguage};
use ton_language_server_core::{
    DocumentUri, LanguageService, LanguageServiceConfig, TextIndex, WorkspaceConfig,
};
use tree_sitter::Node;

const FIXTURES_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/languages/tolk/compiler/fixtures"
);
const CORPUS_ROOT_URI: &str = "file:///tolk-compiler-corpus";

#[test]
fn positive_compiler_fixtures_match_language_server_semantics() -> anyhow::Result<()> {
    let fixtures = positive_fixtures()?;
    if fixtures.is_empty() {
        return Ok(());
    }

    let checked_fixtures = fixtures
        .iter()
        .map(|fixture| fixture.strip_prefix(FIXTURES_DIR).map(path_to_slashes))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut failures = Vec::new();
    let mut used_syntax_exclusions = BTreeSet::new();
    let mut used_type_exclusions = BTreeSet::new();
    let mut used_resolution_exclusions = BTreeSet::new();
    let mut type_expectation_count = 0;
    let mut resolution_count = 0;

    for fixture in &fixtures {
        let fixture_path = fixture.strip_prefix(FIXTURES_DIR)?;
        let fixture_name = path_to_slashes(fixture_path);
        let display_path = fixture_display_path(&fixture_name);
        let source = fs::read_to_string(fixture)?;
        let parsed = tolk_syntax::parse(&source)?;
        eprintln!("checking {display_path}");

        if !parsed.errors().is_empty() {
            if let Some((index, exclusion)) = SYNTAX_EXCLUSIONS
                .iter()
                .enumerate()
                .find(|(_, exclusion)| exclusion.fixture == fixture_name)
            {
                used_syntax_exclusions.insert(index);
                eprintln!("excluded {display_path}: {}", exclusion.reason);
            } else {
                for error in parsed.errors() {
                    record_failure(
                        &mut failures,
                        format!(
                            "{display_path}:{}:{}: {}",
                            error.span.start.row + 1,
                            error.span.start.column + 1,
                            error.message,
                        ),
                    );
                }
            }
            continue;
        }

        let uri = fixture_uri(fixture_path);
        let (mut service, language) = open_fixture(&uri, &source)?;
        let text_index = TextIndex::new(&source);

        for node in syntax_nodes(parsed.root_node()) {
            if let Ok(call) = Call::try_from_node(node)
                && let Some(expectation) = type_expectation(call, &source)
            {
                type_expectation_count += 1;
                check_type_expectation(
                    &language,
                    &uri,
                    &text_index,
                    &fixture_name,
                    &display_path,
                    &source,
                    expectation,
                    &mut used_type_exclusions,
                    &mut failures,
                )?;
            }

            if !matches!(node.kind(), "identifier" | "type_identifier")
                || tolk_syntax::is_declaration_name_node(node)
                || is_inside_annotation(node)
            {
                continue;
            }

            let symbol = node.utf8_text(source.as_bytes())?;
            if globally_excluded_symbol(symbol) {
                continue;
            }

            resolution_count += 1;
            check_resolution(
                &mut service,
                &uri,
                &text_index,
                &fixture_name,
                &display_path,
                &source,
                node,
                &mut used_resolution_exclusions,
                &mut failures,
            )?;
        }
    }

    report_unused_exclusions(
        &checked_fixtures,
        &used_syntax_exclusions,
        &used_type_exclusions,
        &used_resolution_exclusions,
        &mut failures,
    );

    if !failures.is_empty() {
        panic!(
            "Tolk compiler corpus has {} mismatches; see the paths above",
            failures.len()
        );
    }

    let summary = format!(
        "vendored_fixtures={} checked_fixtures={} type_expectations={} symbol_usages={}",
        all_tolk_files_under(Path::new(FIXTURES_DIR))?.len(),
        fixtures.len(),
        type_expectation_count,
        resolution_count,
    );
    if std::env::var_os("TOLK_CORPUS_FIXTURE").is_none() {
        expect![
            "vendored_fixtures=645 checked_fixtures=105 type_expectations=838 symbol_usages=31480"
        ]
        .assert_eq(&summary);
    }

    Ok(())
}

struct TypeExpectation<'tree> {
    expression: Expr<'tree>,
    expected: &'tree str,
}

fn type_expectation<'tree>(
    call: Call<'tree>,
    source: &'tree str,
) -> Option<TypeExpectation<'tree>> {
    let callee = call.callee()?;
    if callee.syntax().utf8_text(source.as_bytes()).ok()? != "__expect_type" {
        return None;
    }

    let mut arguments = call.arguments();
    let expression = arguments.next()?.expr()?;
    let Expr::StringLit(expected) = arguments.next()?.expr()? else {
        return None;
    };

    Some(TypeExpectation {
        expression,
        expected: expected.content(source),
    })
}

#[allow(clippy::too_many_arguments)]
fn check_type_expectation(
    language: &TolkLanguage,
    uri: &DocumentUri,
    text_index: &TextIndex,
    fixture: &str,
    display_path: &str,
    source: &str,
    expectation: TypeExpectation<'_>,
    used_exclusions: &mut BTreeSet<usize>,
    failures: &mut Vec<String>,
) -> anyhow::Result<()> {
    let syntax = expectation.expression.syntax();
    let position = text_index.offset_to_position(source, syntax.start_byte());
    let actual = language
        .type_of_range(uri, syntax.start_byte()..syntax.end_byte())
        .unwrap_or_else(|| "<none>".to_owned());

    if actual == expectation.expected {
        return Ok(());
    }

    let expression = compact_text(syntax.utf8_text(source.as_bytes())?);
    let line = position.line + 1;
    let exclusion = TYPE_EXPECTATION_EXCLUSIONS
        .iter()
        .enumerate()
        .find(|(_, exclusion)| {
            exclusion.fixture == fixture
                && exclusion.line == line
                && exclusion.expression == expression
                && exclusion.expected == expectation.expected
                && exclusion.actual == actual
        });

    if let Some((index, _)) = exclusion {
        used_exclusions.insert(index);
    } else {
        record_failure(
            failures,
            format!(
                "{display_path}:{line}: type of `{expression}`: expected `{}`, got `{actual}`",
                expectation.expected,
            ),
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_resolution(
    service: &mut LanguageService,
    uri: &DocumentUri,
    text_index: &TextIndex,
    fixture: &str,
    display_path: &str,
    source: &str,
    node: Node<'_>,
    used_exclusions: &mut BTreeSet<usize>,
    failures: &mut Vec<String>,
) -> anyhow::Result<()> {
    let position = text_index.offset_to_position(source, node.start_byte());
    if !service.definition(uri, position)?.is_empty() {
        return Ok(());
    }

    let symbol = node.utf8_text(source.as_bytes())?;
    let line = position.line + 1;
    let character = position.character + 1;
    let exclusion = RESOLUTION_EXCLUSIONS
        .iter()
        .enumerate()
        .find(|(_, exclusion)| {
            exclusion.fixture == fixture
                && exclusion.line == line
                && exclusion.character == character
                && exclusion.symbol == symbol
        });

    if let Some((index, _)) = exclusion {
        used_exclusions.insert(index);
    } else {
        record_failure(
            failures,
            format!("{display_path}:{line}:{character}: unresolved symbol `{symbol}`"),
        );
    }

    Ok(())
}

fn report_unused_exclusions(
    checked_fixtures: &BTreeSet<String>,
    used_syntax_exclusions: &BTreeSet<usize>,
    used_type_exclusions: &BTreeSet<usize>,
    used_resolution_exclusions: &BTreeSet<usize>,
    failures: &mut Vec<String>,
) {
    for (index, exclusion) in SYNTAX_EXCLUSIONS.iter().enumerate() {
        if checked_fixtures.contains(exclusion.fixture) && !used_syntax_exclusions.contains(&index)
        {
            record_failure(
                failures,
                format!(
                    "{}: unused syntax exclusion: {}",
                    fixture_display_path(exclusion.fixture),
                    exclusion.reason,
                ),
            );
        }
    }

    for (index, exclusion) in TYPE_EXPECTATION_EXCLUSIONS.iter().enumerate() {
        if checked_fixtures.contains(exclusion.fixture) && !used_type_exclusions.contains(&index) {
            record_failure(
                failures,
                format!(
                    "{}:{}: unused type exclusion `{}`: {}",
                    fixture_display_path(exclusion.fixture),
                    exclusion.line,
                    exclusion.expression,
                    exclusion.reason,
                ),
            );
        }
    }

    for (index, exclusion) in RESOLUTION_EXCLUSIONS.iter().enumerate() {
        if checked_fixtures.contains(exclusion.fixture)
            && !used_resolution_exclusions.contains(&index)
        {
            record_failure(
                failures,
                format!(
                    "{}:{}:{}: unused resolution exclusion `{}`: {}",
                    fixture_display_path(exclusion.fixture),
                    exclusion.line,
                    exclusion.character,
                    exclusion.symbol,
                    exclusion.reason,
                ),
            );
        }
    }
}

fn open_fixture(
    uri: &DocumentUri,
    source: &str,
) -> anyhow::Result<(LanguageService, TolkLanguage)> {
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    let language = TolkLanguage::new();
    service.register_language(language.clone());
    service.set_workspace_config(
        LANGUAGE_ID,
        WorkspaceConfig::new(
            CORPUS_ROOT_URI,
            None,
            r#"
                [import-mappings]
                "@custom-folder" = "./imports"
            "#,
        ),
    )?;

    if source.contains("imports/") || source.contains("@custom-folder/") {
        add_import_fixtures(&mut service)?;
    }

    service.open_document(uri.clone(), LANGUAGE_ID, 1, source.to_owned())?;
    Ok((service, language))
}

fn add_import_fixtures(service: &mut LanguageService) -> anyhow::Result<()> {
    let imports_dir = Path::new(FIXTURES_DIR).join("imports");
    for path in tolk_files_in(&imports_dir)? {
        let relative = path.strip_prefix(FIXTURES_DIR)?;
        service.add_source_file(
            LANGUAGE_ID,
            fixture_uri(relative),
            fs::read_to_string(path)?,
        )?;
    }

    Ok(())
}

fn positive_fixtures() -> anyhow::Result<Vec<PathBuf>> {
    let root = Path::new(FIXTURES_DIR);
    let mut fixtures = tolk_files_in(root)?;
    fixtures.extend(tolk_files_in(&root.join("warnings-not-errors"))?);
    fixtures.sort();
    if let Some(filter) = std::env::var_os("TOLK_CORPUS_FIXTURE") {
        let filter = filter.to_string_lossy();
        fixtures.retain(|fixture| path_to_slashes(fixture).ends_with(filter.as_ref()));
    }
    Ok(fixtures)
}

fn tolk_files_in(directory: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(files),
        Err(error) => return Err(error.into()),
    };

    for entry in entries {
        let path = entry?.path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension == "tolk")
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn all_tolk_files_under(directory: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut pending = vec![directory.to_owned()];

    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else if path
                .extension()
                .is_some_and(|extension| extension == "tolk")
            {
                files.push(path);
            }
        }
    }

    files.sort();
    Ok(files)
}

fn syntax_nodes(root: Node<'_>) -> Vec<Node<'_>> {
    let mut nodes = Vec::new();
    let mut pending = vec![root];

    while let Some(node) = pending.pop() {
        nodes.push(node);

        let mut cursor = node.walk();
        let mut children = node.named_children(&mut cursor).collect::<Vec<_>>();
        children.reverse();
        pending.extend(children);
    }

    nodes
}

fn is_inside_annotation(mut node: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if Annotation::try_from_node(parent).is_ok() {
            return true;
        }
        node = parent;
    }

    false
}

fn fixture_uri(relative_path: &Path) -> DocumentUri {
    DocumentUri::from(format!(
        "{CORPUS_ROOT_URI}/{}",
        path_to_slashes(relative_path)
    ))
}

fn path_to_slashes(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn fixture_display_path(fixture: &str) -> String {
    format!("crates/ton-language-server-core/tests/languages/tolk/compiler/fixtures/{fixture}")
}

fn globally_excluded_symbol(symbol: &str) -> bool {
    GLOBAL_RESOLUTION_EXCLUSIONS
        .iter()
        .any(|(excluded, _)| *excluded == symbol)
}

fn compact_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn record_failure(failures: &mut Vec<String>, failure: String) {
    eprintln!("{failure}");
    failures.push(failure);
}
