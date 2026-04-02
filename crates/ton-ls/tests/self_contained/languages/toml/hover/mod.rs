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

            Custom network configuration

            - Type: `object`

            ---

            ```toml
            networks.mainnet.api.v2
            ```

            The URL for the TonCenter API v2. For localnet this defaults to `http://localhost:<litenode.port>/api/v2` with `5411` as the fallback port

            - Type: `string`

            ---

            ```toml
            networks.mainnet.api.v3
            ```

            The URL for the TonCenter API v3. For localnet this defaults to `http://localhost:<litenode.port>/api/v3` with `5411` as the fallback port

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

            Disable the rule

            Emit warnings for the rule

            Treat the rule as an error

            Lint severity level

            Global lint level for a rule

            Contract-specific lint overrides

            Lint rule configuration, either a global level or contract-specific overrides

            - Type: `string`
            - Enum: `"allow" | "warn" | "deny"`

            ---

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

#[named]
#[test]
fn test_hover_defaults_and_enums() {
    case_toml_hover(
        function_name!(),
        r#"
            [test]
            <caret>debug-port = 12345
            reporter = ["<caret>console"]

            [test.coverage]
            <caret>format = "<caret>lcov"
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
            test.reporter[0]
            ```

            Human-readable console output

            TeamCity service messages

            JUnit XML report

            Compact dot-progress output

            Output formats supported by `acton test`

            - Type: `string`
            - Enum: `"console" | "teamcity" | "junit" | "dot"`

            ---

            ```toml
            test.coverage.format
            ```

            LCOV coverage report

            Plain-text coverage summary

            Coverage output formats supported by `acton test`

            Format for coverage reports

            - Type: `string`
            - Default: `"lcov"`
            - Enum: `"lcov" | "text"`

            ---

            ```toml
            test.coverage.format
            ```

            LCOV coverage report

            Plain-text coverage summary

            Coverage output formats supported by `acton test`

            Format for coverage reports

            - Type: `string`
            - Default: `"lcov"`
            - Enum: `"lcov" | "text"`"#]],
    );
}
