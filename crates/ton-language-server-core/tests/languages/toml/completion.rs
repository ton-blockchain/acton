#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::toml::{LANGUAGE_ID, TomlLanguage};
use ton_language_server_core::{
    CompletionItem, CompletionList, DocumentUri, LanguageService, LanguageServiceConfig, Position,
    TextIndex,
};

fn complete(source: &str) -> (MarkedSource, CompletionList) {
    complete_at_uri("file:///workspace/Acton.toml", source)
}

fn complete_at_uri(uri: &str, source: &str) -> (MarkedSource, CompletionList) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from(uri);
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TomlLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source())
        .expect("TOML document should open");
    let completion = service
        .completion(
            &uri,
            marked.marker("caret").position,
            ton_language_server_core::CompletionTrigger::invoked(),
        )
        .expect("completion request should succeed");
    (marked, completion)
}

fn check_labels(source: &str, expected: Expect) {
    let (_, completion) = complete(source);
    expected.assert_eq(&render_labels(&completion));
}

fn render_labels(completion: &CompletionList) -> String {
    completion
        .items
        .iter()
        .map(|item| {
            format!(
                "{} {:?} {}",
                item.label,
                item.kind,
                item.detail.as_deref().unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn check_applied(source: &str, label: &str, expected: Expect) {
    let (marked, completion) = complete(source);
    let item = completion
        .items
        .iter()
        .find(|item| item.label == label)
        .unwrap_or_else(|| panic!("completion {label:?} should exist"));
    expected.assert_eq(&apply_item(marked.source(), item));
}

fn apply_item(source: &str, item: &CompletionItem) -> String {
    let edit = item
        .text_edit
        .as_ref()
        .expect("completion should have text edit");
    let index = TextIndex::new(source);
    let start = index.position_to_offset(source, edit.range.start);
    let end = index.position_to_offset(source, edit.range.end);
    let mut result = source.to_owned();
    result.replace_range(start..end, &edit.new_text);
    result
}

#[test]
fn completes_root_tables_from_the_acton_schema() {
    check_labels(
        "<caret>",
        expect![[r"
            build Some(Field) object
            contracts Some(Field) object
            fmt Some(Field) object
            import-mappings Some(Field) object
            lint Some(Field) object
            localnet Some(Field) object
            networks Some(Field) object
            package Some(Field) Required, object
            scripts Some(Field) object
            test Some(Field) object
            toolchain Some(Field) object
            wrappers Some(Field) object"]],
    );
}

#[test]
fn filters_existing_root_tables_and_accepts_case_insensitive_manifest_names() {
    // A table that already exists must not be offered at an earlier root insertion point.
    check_labels(
        r#"
            <caret>

            [package]
            name = "app"
            version = "1.0.0"
            description = "Example"
        "#,
        expect![[r"
            build Some(Field) object
            contracts Some(Field) object
            fmt Some(Field) object
            import-mappings Some(Field) object
            lint Some(Field) object
            localnet Some(Field) object
            networks Some(Field) object
            scripts Some(Field) object
            test Some(Field) object
            toolchain Some(Field) object
            wrappers Some(Field) object"]],
    );

    // File-name matching follows the native file-system behavior on case-insensitive systems.
    let (_, completion) = complete_at_uri("file:///workspace/acton.TOML", "<caret>");
    expect!["12"].assert_eq(&completion.items.len().to_string());

    // Unknown root scalars do not hide any schema-owned root tables.
    check_labels(
        "
            custom-setting = true
            <caret>
        ",
        expect![[r"
            build Some(Field) object
            contracts Some(Field) object
            fmt Some(Field) object
            import-mappings Some(Field) object
            lint Some(Field) object
            localnet Some(Field) object
            networks Some(Field) object
            package Some(Field) Required, object
            scripts Some(Field) object
            test Some(Field) object
            toolchain Some(Field) object
            wrappers Some(Field) object"]],
    );
}

#[test]
fn applies_table_and_partial_header_completions() {
    check_applied(
        "<caret>",
        "package",
        expect![[r"
            [package]
            $0"]],
    );

    check_applied("[pac<caret>]", "package", expect!["[package]"]);

    // Completion must remain available while the closing bracket has not been typed yet.
    check_applied("[pac<caret>", "package", expect!["[package"]);
}

#[test]
fn completes_only_object_properties_in_nested_table_headers() {
    check_labels(
        "[test.<caret>]",
        expect![[r"
            coverage Some(Field) object
            fuzz Some(Field) object
            mutation Some(Field) object"]],
    );

    // Quoted dynamic keys may contain dots and must stay a single schema segment.
    check_labels(
        r#"[networks."dev.net".<caret>]"#,
        expect!["api Some(Field) object"],
    );

    // Array-of-table headers only offer array-valued properties.
    check_applied(
        "[[contracts.wallet.dep<caret>]]",
        "depends",
        expect!["[[contracts.wallet.depends]]"],
    );

    // Scalar arrays cannot be represented as an array of TOML tables.
    check_labels("[[test.<caret>]]", expect![""]);

    // Applying a nested header completion replaces only its final segment.
    check_applied("[test.cov<caret>]", "coverage", expect!["[test.coverage]"]);
}

#[test]
fn completes_only_missing_package_keys() {
    check_labels(
        r#"
            [package]
            name = "app"
            version = "1.0.0"
            <caret>
        "#,
        expect![[r"
            description Some(Field) Required, string
            license Some(Field) string
            repository Some(Field) string"]],
    );
}

#[test]
fn completes_dynamic_contract_and_network_objects() {
    // `contracts` uses additionalProperties, so the arbitrary contract name remains in the path.
    check_labels(
        r#"
            [contracts.wallet]
            src = "wallet.tolk"
            <caret>
        "#,
        expect![[r"
            depends Some(Field) array
            display-name Some(Field) string
            output Some(Field) string
            types Some(Field) string"]],
    );

    // A known property behind a dynamic network name is still completed from its schema.
    check_labels(
        r"
            [networks.devnet.api]
            <caret>
        ",
        expect![[r"
            v2 Some(Field) string
            v3 Some(Field) string"]],
    );

    // Existing keys are also filtered inside an inline table value.
    check_labels(
        r#"
            [networks.devnet]
            api = { v2 = "https://example.invalid", <caret> }
        "#,
        expect!["v3 Some(Field) string"],
    );
}

#[test]
fn completes_union_object_fields_in_inline_tables_and_table_arrays() {
    // The object branch of ContractDependency contributes its fields inside an inline table.
    check_labels(
        r#"
            [contracts.wallet]
            src = "wallet.tolk"
            depends = [{ name = "base", <caret> }]
        "#,
        expect![[r"
            function Some(Field) string
            kind Some(Field) string
            path Some(Field) string"]],
    );

    // The same union branch is reachable through TOML's array-of-tables representation.
    check_labels(
        r#"
            [contracts.wallet]
            src = "wallet.tolk"

            [[contracts.wallet.depends]]
            name = "base"
            <caret>
        "#,
        expect![[r"
            function Some(Field) string
            kind Some(Field) string
            path Some(Field) string"]],
    );
}

#[test]
fn keeps_table_array_elements_independent() {
    // A key used by one array element remains available in the next element.
    check_labels(
        r#"
            [contracts.wallet]
            src = "wallet.tolk"

            [[contracts.wallet.depends]]
            name = "base"

            [[contracts.wallet.depends]]
            <caret>
        "#,
        expect![[r"
            function Some(Field) string
            kind Some(Field) string
            name Some(Field) Required, string
            path Some(Field) string"]],
    );

    // Repeating an array-of-tables header must not be hidden by an earlier element.
    check_applied(
        r#"
            [contracts.wallet]
            src = "wallet.tolk"

            [[contracts.wallet.depends]]
            name = "base"

            [[contracts.wallet.dep<caret>]]
        "#,
        "depends",
        expect![[r#"
            [contracts.wallet]
            src = "wallet.tolk"

            [[contracts.wallet.depends]]
            name = "base"

            [[contracts.wallet.depends]]"#]],
    );
}

#[test]
fn inserts_schema_appropriate_value_snippets_for_properties() {
    // Boolean properties use a choice placeholder.
    check_applied(
        "
            [test]
            <caret>
        ",
        "debug",
        expect![[r"
            [test]
            debug = ${1|true,false|}"]],
    );

    // Numeric defaults are selected when the schema provides one.
    check_applied(
        "
            [test]
            <caret>
        ",
        "debug-port",
        expect![[r"
            [test]
            debug-port = ${1:12345}"]],
    );

    // Strings, arrays, nested objects, and floating-point values get distinct snippets.
    check_applied(
        "
            [test]
            <caret>
        ",
        "filter",
        expect![[r#"
            [test]
            filter = "$1""#]],
    );

    check_applied(
        "
            [test]
            <caret>
        ",
        "exclude",
        expect![[r"
            [test]
            exclude = [${1}]"]],
    );

    check_applied(
        "
            [test]
            <caret>
        ",
        "coverage",
        expect![[r"
            [test]
            coverage = { $1 }"]],
    );

    check_applied(
        "
            [test.coverage]
            <caret>
        ",
        "minimum-percent",
        expect![[r"
            [test.coverage]
            minimum-percent = ${1:0.0}"]],
    );
}

#[test]
fn replaces_boolean_and_string_enum_values() {
    check_applied(
        r"
            [test.coverage]
            enabled = f<caret>alse
        ",
        "true",
        expect![[r"
            [test.coverage]
            enabled = true"]],
    );

    check_applied(
        r#"
            [test]
            reporter = ["con<caret>sole"]
        "#,
        "\"teamcity\"",
        expect![[r#"
            [test]
            reporter = ["teamcity"]"#]],
    );
}

#[test]
fn completes_values_for_resolved_refs_unions_and_dynamic_properties() {
    // The legacy implementation fell back to root keys instead of following the enum reference.
    check_labels(
        "
            [lint]
            output-format = <caret>json
        ",
        expect![[r#"
            "plain" Some(EnumMember) Enum value
            "json" Some(EnumMember) Enum value
            "sarif" Some(EnumMember) Enum value
            "github" Some(EnumMember) Enum value
            "gitlab" Some(EnumMember) Enum value"#]],
    );

    // Quoted keys are decoded according to TOML, including basic-string escapes.
    check_labels(
        r#"
            [lint]
            "output\u002dformat" = "<caret>"
        "#,
        expect![[r#"
            "plain" Some(EnumMember) Enum value
            "json" Some(EnumMember) Enum value
            "sarif" Some(EnumMember) Enum value
            "github" Some(EnumMember) Enum value
            "gitlab" Some(EnumMember) Enum value"#]],
    );

    // A dynamic lint-rule key resolves through additionalProperties and an anyOf branch.
    check_labels(
        r#"
            [lint.rules]
            unused-imports = "<caret>"
        "#,
        expect![[r#"
            "allow" Some(EnumMember) Enum value
            "warn" Some(EnumMember) Enum value
            "deny" Some(EnumMember) Enum value"#]],
    );

    // Contract-specific overrides add one more dynamic object segment to the schema path.
    check_labels(
        r#"
            [lint.rules.shadowing]
            Wallet = "<caret>"
        "#,
        expect![[r#"
            "allow" Some(EnumMember) Enum value
            "warn" Some(EnumMember) Enum value
            "deny" Some(EnumMember) Enum value"#]],
    );

    // Array item completion follows the item schema, not the array schema itself.
    check_labels(
        r#"
            [test]
            reporter = ["console", "<caret>"]
        "#,
        expect![[r#"
            "console" Some(EnumMember) Enum value
            "teamcity" Some(EnumMember) Enum value
            "junit" Some(EnumMember) Enum value
            "dot" Some(EnumMember) Enum value"#]],
    );
}

#[test]
fn completes_values_before_the_parser_has_a_scalar_node() {
    // Editors request completion immediately after `=`, before a value node exists.
    check_labels(
        "
            [test]
            debug = <caret>
        ",
        expect![[r"
            true Some(Value) 
            false Some(Value) "]],
    );

    check_applied(
        "
            [test]
            debug = <caret>
        ",
        "true",
        expect![[r"
            [test]
            debug = true"]],
    );

    // An incomplete property name remains a key context even though `=` is already present.
    check_applied(
        "
            [test]
            deb<caret> = false
        ",
        "debug",
        expect![[r"
            [test]
            debug = false"]],
    );

    // An unfinished quoted value is replaced with a complete quoted enum literal.
    check_applied(
        r#"
            [lint]
            output-format = "git<caret>
        "#,
        "\"github\"",
        expect![[r#"
            [lint]
            output-format = "github""#]],
    );

    // A hash inside an unfinished basic string is content, not the start of a comment.
    check_applied(
        r#"
            [lint]
            output-format = "git#<caret>
        "#,
        "\"github\"",
        expect![[r#"
            [lint]
            output-format = "github""#]],
    );

    // Recovery replacement stops before an unquoted TOML comment.
    check_applied(
        "
            [test]
            debug = f<caret> # keep this comment
        ",
        "true",
        expect![[r"
            [test]
            debug = true # keep this comment"]],
    );
}

#[test]
fn ignores_comments_and_header_like_text_inside_multiline_strings() {
    // Schema keys must never be offered in a standalone TOML comment.
    check_labels(
        "
            [package]
            # Project <caret>metadata
        ",
        expect![""],
    );

    // A trailing comment stays outside the preceding property's value context.
    check_labels(
        "
            [test]
            debug = true # <caret>enabled in development
        ",
        expect![""],
    );

    // A line that starts with `[` inside a multiline string is not a table header.
    check_labels(
        r#"
            [package]
            description = """
            [<caret>not-a-table]
            """
        "#,
        expect![""],
    );
}

#[test]
fn applies_enum_values_without_replacing_toml_quotes() {
    check_applied(
        r#"
            [lint]
            output-format = "js<caret>on"
        "#,
        "\"json\"",
        expect![[r#"
            [lint]
            output-format = "json""#]],
    );

    check_applied(
        r#"
            [contracts.wallet]
            src = "wallet.tolk"
            depends = [{ name = "base", kind = "lib<caret>" }]
        "#,
        "\"library_ref\"",
        expect![[r#"
            [contracts.wallet]
            src = "wallet.tolk"
            depends = [{ name = "base", kind = "library_ref" }]"#]],
    );
}

#[test]
fn computes_utf16_replacement_ranges_for_non_ascii_lines() {
    check_applied(
        "
            # проект 🚀
            [test]
            debug = f<caret>alse
        ",
        "true",
        expect![[r"
            # проект 🚀
            [test]
            debug = true"]],
    );
}

#[test]
fn reparses_incrementally_after_manifest_changes() {
    let uri = DocumentUri::from("file:///workspace/Acton.toml");
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TomlLanguage::new());
    let initial = r#"[package]
name = "app"
"#;
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, initial)
        .expect("initial TOML document should open");

    let before = service
        .completion(
            &uri,
            Position::new(2, 0),
            ton_language_server_core::CompletionTrigger::invoked(),
        )
        .expect("initial completion should succeed");
    expect![[r"
        description Some(Field) Required, string
        license Some(Field) string
        repository Some(Field) string
        version Some(Field) Required, string"]]
    .assert_eq(&render_labels(&before));

    let changed = r#"[package]
name = "app"
version = "1.0.0"
"#;
    service
        .change_document(&uri, 2, changed)
        .expect("changed TOML document should reparse");

    let after = service
        .completion(
            &uri,
            Position::new(3, 0),
            ton_language_server_core::CompletionTrigger::invoked(),
        )
        .expect("completion after change should succeed");
    expect![[r"
        description Some(Field) Required, string
        license Some(Field) string
        repository Some(Field) string"]]
    .assert_eq(&render_labels(&after));
}

#[test]
fn ignores_non_acton_toml_files_and_unknown_tables() {
    let (_, completion) = complete_at_uri("file:///workspace/config.toml", "<caret>");
    expect!["0"].assert_eq(&completion.items.len().to_string());

    check_labels(
        r"
            [unknown]
            value = <caret>
        ",
        expect![""],
    );
}
