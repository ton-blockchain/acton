#![allow(clippy::needless_raw_string_hashes)]

#[path = "../../support/mod.rs"]
mod support;

use expect_test::{Expect, expect};
use support::MarkedSource;
use ton_language_server_core::languages::toml::{LANGUAGE_ID, TomlLanguage};
use ton_language_server_core::{DocumentUri, LanguageService, LanguageServiceConfig};

fn check_hover(uri: &str, source: &str, expected: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from(uri);
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TomlLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source())
        .expect("TOML document should open");
    let result = service
        .hover(&uri, marked.marker("caret").position)
        .expect("hover request should succeed")
        .map_or_else(|| "<none>".to_owned(), |hover| hover.contents);
    expected.assert_eq(&result);
}

fn check_hovers(uri: &str, source: &str, expected: Expect) {
    let marked = MarkedSource::parse(source);
    let uri = DocumentUri::from(uri);
    let mut service = LanguageService::new(LanguageServiceConfig::default());
    service.register_language(TomlLanguage::new());
    service
        .open_document(uri.clone(), LANGUAGE_ID, 1, marked.source())
        .expect("TOML document should open");

    let result = marked
        .markers()
        .iter()
        .filter(|marker| marker.name == "caret")
        .map(|marker| {
            service
                .hover(&uri, marker.position)
                .expect("hover request should succeed")
                .map_or_else(|| "<none>".to_owned(), |hover| hover.contents)
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");
    expected.assert_eq(&result);
}

#[test]
fn documents_nested_array_values() {
    check_hover(
        "file:///workspace/Acton.toml",
        r#"
            [fmt]
            ignore = ["<caret>build/**"]
        "#,
        expect![[r#"
            ```toml
            fmt.ignore[0]
            ```

            - Type: `string`"#]],
    );
}

#[test]
fn documents_tables_keys_defaults_and_enums() {
    check_hover(
        "file:///workspace/Acton.toml",
        r#"
            [<caret>package]
            name = "app"
        "#,
        expect![[r#"
            ```toml
            package
            ```

            Package metadata for the Acton project

            - Type: `object`"#]],
    );

    check_hover(
        "file:///workspace/Acton.toml",
        r#"
            [test]
            <caret>debug-port = 12345
        "#,
        expect![[r#"
            ```toml
            test.debug-port
            ```

            Port for the debug server

            - Type: `integer`
            - Default: `12345`"#]],
    );

    check_hover(
        "file:///workspace/Acton.toml",
        r#"
            [test]
            reporter = ["<caret>console"]
        "#,
        expect![[r#"
            ```toml
            test.reporter[0]
            ```

            Human-readable console output

            `TeamCity` service messages

            `JUnit` XML report

            Compact dot-progress output

            Output formats supported by `acton test`

            - Type: `string`
            - Enum: `"console" | "teamcity" | "junit" | "dot"`"#]],
    );
}

#[test]
fn resolves_dynamic_network_and_contract_paths() {
    check_hover(
        "file:///workspace/Acton.toml",
        r#"
            [networks.mainnet]
            api = { <caret>v2 = "https://example.invalid/v2" }
        "#,
        expect![[r#"
            ```toml
            networks.mainnet.api.v2
            ```

            The URL for the `TonCenter` API v2. For localnet this defaults to `http://127.0.0.1:<localnet.port>/api/v2` with `5411` as the fallback port

            - Type: `string`"#]],
    );
}

#[test]
fn documents_contract_dependency_union_branches() {
    check_hovers(
        "file:///workspace/Acton.toml",
        r#"
            [contracts.wallet]
            src = "wallet.tolk"
            depends = [
                {
                    name = "<caret>jetton",
                    kind = "<caret>library_ref",
                    function = "<caret>gen_wallet_dep",
                    path = "<caret>deps/wallet.tolk"
                },
                "<caret>common_dep"
            ]
        "#,
        expect![[r#"
            ```toml
            contracts.wallet.depends[0].name
            ```

            Name of the contract to depend on

            - Type: `string`

            ---

            ```toml
            contracts.wallet.depends[0].kind
            ```

            Embed dependency code directly into the output

            Reference the dependency as an on-chain library

            How a compiled dependency is linked into a contract

            Dependency type

            - Type: `string`
            - Default: `"embed_code"`
            - Enum: `"embed_code" | "library_ref"`

            ---

            ```toml
            contracts.wallet.depends[0].function
            ```

            Custom name for the generated code function

            - Type: `string`

            ---

            ```toml
            contracts.wallet.depends[0].path
            ```

            Custom output path for the generated code file

            - Type: `string`

            ---

            ```toml
            contracts.wallet.depends[1]
            ```

            Name of the contract to depend on in the simple form

            Detailed dependency configuration

            Dependency declaration for a contract

            - Type: `string`"#]],
    );
}

#[test]
fn documents_dynamic_lint_rule_values() {
    check_hover(
        "file:///workspace/Acton.toml",
        r#"
            [lint.rules]
            unused-imports = "<caret>warn"
        "#,
        expect![[r#"
            ```toml
            lint.rules.unused-imports
            ```

            Disable the rule

            Emit warnings for the rule

            Treat the rule as an error

            Lint severity level

            Global lint level for a rule

            Contract-specific lint overrides

            Lint rule configuration, either a global level or contract-specific overrides

            - Type: `string`
            - Enum: `"allow" | "warn" | "deny"`"#]],
    );

    check_hover(
        "file:///workspace/Acton.toml",
        r#"
            [lint.rules.shadowing]
            Wallet = "<caret>deny"
        "#,
        expect![[r#"
            ```toml
            lint.rules.shadowing.Wallet
            ```

            Disable the rule

            Emit warnings for the rule

            Treat the rule as an error

            Lint severity level

            - Type: `string`
            - Enum: `"allow" | "warn" | "deny"`"#]],
    );
}

#[test]
fn documents_table_array_items() {
    check_hover(
        "file:///workspace/Acton.toml",
        r#"
            [contracts.wallet]
            src = "wallet.tolk"

            [[contracts.wallet.depends]]
            name = "base"
            kind = "<caret>library_ref"
        "#,
        expect![[r#"
            ```toml
            contracts.wallet.depends[0].kind
            ```

            Embed dependency code directly into the output

            Reference the dependency as an on-chain library

            How a compiled dependency is linked into a contract

            Dependency type

            - Type: `string`
            - Default: `"embed_code"`
            - Enum: `"embed_code" | "library_ref"`"#]],
    );

    // Hover paths preserve the concrete index instead of normalizing every element to zero.
    check_hover(
        "file:///workspace/Acton.toml",
        r#"
            [contracts.wallet]
            src = "wallet.tolk"

            [[contracts.wallet.depends]]
            name = "base"

            [[contracts.wallet.depends]]
            name = "wallet"
            kind = "<caret>embed_code"
        "#,
        expect![[r#"
            ```toml
            contracts.wallet.depends[1].kind
            ```

            Embed dependency code directly into the output

            Reference the dependency as an on-chain library

            How a compiled dependency is linked into a contract

            Dependency type

            - Type: `string`
            - Default: `"embed_code"`
            - Enum: `"embed_code" | "library_ref"`"#]],
    );
}

#[test]
fn documents_quoted_and_dotted_keys() {
    check_hover(
        "file:///workspace/Acton.toml",
        r#"
            ["networks"."dev.net"]
            "api"."v3" = "<caret>https://example.invalid"
        "#,
        expect![[r#"
            ```toml
            networks."dev.net".api.v3
            ```

            The URL for the `TonCenter` API v3. For localnet this defaults to `http://127.0.0.1:<localnet.port>/api/v3` with `5411` as the fallback port

            - Type: `string`"#]],
    );
}

#[test]
fn ignores_unknown_keys_and_non_acton_manifests() {
    check_hover(
        "file:///workspace/Acton.toml",
        "unknown = <caret>1",
        expect!["<none>"],
    );

    check_hover(
        "file:///workspace/config.toml",
        "[<caret>package]",
        expect!["<none>"],
    );

    check_hover(
        "file:///workspace/ACTON.TOML",
        "[<caret>package]",
        expect![[r#"
            ```toml
            package
            ```

            Package metadata for the Acton project

            - Type: `object`"#]],
    );
}

#[test]
fn ignores_comments_and_whitespace() {
    check_hover(
        "file:///workspace/Acton.toml",
        "
            [package]
            # Project <caret>metadata
        ",
        expect!["<none>"],
    );

    check_hover(
        "file:///workspace/Acton.toml",
        r#"
            [package]
            name<caret> = "app"
        "#,
        expect!["<none>"],
    );
}
