use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::toml::helpers::case_toml_hover;

#[named]
#[test]
fn test_hover_root_keys_and_table() {
    case_toml_hover(
        function_name!(),
        r#"
            <caret>name = "my-app"
            <caret>version = "0.1.0"

            [<caret>package]
            <caret>type = "contract"
        "#,
        expect![[r#"
            <none>

            ---

            <none>

            ---

            ```toml
            package
            ```

            Package metadata for the Acton project

            - Type: `object`

            ---

            <none>"#]],
    );
}

#[named]
#[test]
fn test_hover_array_item_path() {
    case_toml_hover(
        function_name!(),
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

#[named]
#[test]
fn test_hover_unknown_key_returns_none() {
    case_toml_hover(
        function_name!(),
        r#"
            unknown = <caret>1
        "#,
        expect!["<none>"],
    );
}

#[named]
#[test]
fn test_hover_networks_dynamic_tables() {
    case_toml_hover(
        function_name!(),
        r#"
            [networks.<caret>mainnet]
            api = { <caret>v2 = "https://toncenter.com/api/v2", <caret>v3 = "https://toncenter.com/api/v3" }
        "#,
        expect![[r#"
            ```toml
            networks.mainnet
            ```

            - Type: `object`

            ---

            ```toml
            networks.mainnet.api.v2
            ```

            The URL for the TonCenter API v2

            - Type: `string`

            ---

            ```toml
            networks.mainnet.api.v3
            ```

            The URL for the TonCenter API v3

            - Type: `string`"#]],
    );
}

#[named]
#[test]
fn test_hover_contracts_depends_nested_objects() {
    case_toml_hover(
        function_name!(),
        r#"
            [contracts.wallet]
            name = "Wallet"
            src = "wallet.tolk"
            depends = [
              { name = "<caret>jetton", kind = "<caret>library_ref", function = "<caret>gen_wallet_dep", path = "<caret>deps/wallet.tolk" },
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

            Name of the contract to depend on (simple format)

            Detailed dependency configuration

            - Type: `string`"#]],
    );
}

#[named]
#[test]
fn test_hover_lint_rules_additional_properties() {
    case_toml_hover(
        function_name!(),
        r#"
            [lint.rules]
            unused-imports = "<caret>warn"

            [lint.rules.shadowing]
            Wallet = "<caret>deny"
        "#,
        expect![[r#"
            ```toml
            lint.rules.unused-imports
            ```

            Global lint level for a rule

            Contract-specific lint overrides

            - Type: `string`
            - Enum: `"allow" | "warn" | "deny"`

            ---

            ```toml
            lint.rules.shadowing.Wallet
            ```

            - Type: `string`
            - Enum: `"allow" | "warn" | "deny"`"#]],
    );
}

#[named]
#[test]
fn test_hover_defaults_and_enums() {
    case_toml_hover(
        function_name!(),
        r#"
            [test]
            <caret>debug-port = 12345
            <caret>coverage-format = "<caret>lcov"
            reporter = ["<caret>console"]
        "#,
        expect![[r#"
            ```toml
            test.debug-port
            ```

            Port for the debug server

            - Type: `integer`
            - Default: `12345`

            ---

            ```toml
            test.coverage-format
            ```

            Format for coverage reports (e.g., 'lcov')

            - Type: `string`
            - Default: `"lcov"`

            ---

            ```toml
            test.coverage-format
            ```

            Format for coverage reports (e.g., 'lcov')

            - Type: `string`
            - Default: `"lcov"`

            ---

            ```toml
            test.reporter[0]
            ```

            - Type: `string`
            - Enum: `"console" | "teamcity" | "junit" | "dot"`"#]],
    );
}
