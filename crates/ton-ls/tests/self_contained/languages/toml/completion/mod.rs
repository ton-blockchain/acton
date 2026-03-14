use expect_test::expect;
use function_name::named;

use crate::self_contained::languages::toml::helpers::{
    case_toml_completion, case_toml_completion_apply,
};

#[named]
#[test]
fn test_completion_root_keys() {
    case_toml_completion(
        function_name!(),
        r#"
            <caret>
        "#,
        expect![[r#"
            0: label=build kind=Field detail=object format=snippet
            1: label=contracts kind=Field detail=object format=snippet
            2: label=fmt kind=Field detail=object format=snippet
            3: label=lint kind=Field detail=object format=snippet
            4: label=litenode kind=Field detail=object format=snippet
            5: label=mappings kind=Field detail=object format=snippet
            6: label=networks kind=Field detail=object format=snippet
            7: label=package kind=Field detail=Required, object format=snippet
            8: label=scripts kind=Field detail=object format=snippet
            9: label=test kind=Field detail=object format=snippet"#]],
    );
}

#[named]
#[test]
fn test_completion_table_header_keys() {
    case_toml_completion(
        function_name!(),
        r#"
            [<caret>]
        "#,
        expect![[r#"
            0: label=build kind=Field detail=object format=plain
            1: label=contracts kind=Field detail=object format=plain
            2: label=fmt kind=Field detail=object format=plain
            3: label=lint kind=Field detail=object format=plain
            4: label=litenode kind=Field detail=object format=plain
            5: label=mappings kind=Field detail=object format=plain
            6: label=networks kind=Field detail=object format=plain
            7: label=package kind=Field detail=Required, object format=plain
            8: label=scripts kind=Field detail=object format=plain
            9: label=test kind=Field detail=object format=plain"#]],
    );
}

#[named]
#[test]
fn test_completion_test_boolean_values() {
    case_toml_completion(
        function_name!(),
        r#"
            [test]
            coverage = <caret>false
        "#,
        expect![[r#"
            0: label=true kind=Value detail= format=plain
            1: label=false kind=Value detail= format=plain"#]],
    );
}

#[named]
#[test]
fn test_completion_test_reporter_values() {
    case_toml_completion(
        function_name!(),
        r#"
            [test]
            reporter = ["<caret>console"]
        "#,
        expect![[r#"
            0: label="console" kind=EnumMember detail=Enum value format=plain
            1: label="teamcity" kind=EnumMember detail=Enum value format=plain
            2: label="junit" kind=EnumMember detail=Enum value format=plain
            3: label="dot" kind=EnumMember detail=Enum value format=plain"#]],
    );
}

#[named]
#[test]
fn test_completion_filters_existing_root_keys() {
    case_toml_completion(
        function_name!(),
        r#"
            name = "my-app"
            <caret>
        "#,
        expect![[r#"
            0: label=build kind=Field detail=object format=snippet
            1: label=contracts kind=Field detail=object format=snippet
            2: label=fmt kind=Field detail=object format=snippet
            3: label=lint kind=Field detail=object format=snippet
            4: label=litenode kind=Field detail=object format=snippet
            5: label=mappings kind=Field detail=object format=snippet
            6: label=networks kind=Field detail=object format=snippet
            7: label=package kind=Field detail=Required, object format=snippet
            8: label=scripts kind=Field detail=object format=snippet
            9: label=test kind=Field detail=object format=snippet"#]],
    );
}

#[named]
#[test]
fn test_completion_unknown_table_has_none() {
    case_toml_completion(
        function_name!(),
        r#"
            [unknown]
            value = <caret>
        "#,
        expect!["<none>"],
    );
}

#[named]
#[test]
fn test_completion_package_keys_filter_existing() {
    case_toml_completion(
        function_name!(),
        r#"
            [package]
            name = "my-app"
            version = "0.1.0"
            <caret>
        "#,
        expect![[r#"
            0: label=description kind=Field detail=Required, string format=snippet
            1: label=license kind=Field detail=string format=snippet
            2: label=repository kind=Field detail=string format=snippet"#]],
    );
}

#[named]
#[test]
fn test_completion_lint_output_format_enum_values() {
    case_toml_completion(
        function_name!(),
        r#"
            [lint]
            output-format = <caret>json
        "#,
        expect![[r#"
            0: label=build kind=Field detail=object format=snippet
            1: label=contracts kind=Field detail=object format=snippet
            2: label=fmt kind=Field detail=object format=snippet
            3: label=lint kind=Field detail=object format=snippet
            4: label=litenode kind=Field detail=object format=snippet
            5: label=mappings kind=Field detail=object format=snippet
            6: label=networks kind=Field detail=object format=snippet
            7: label=package kind=Field detail=Required, object format=snippet
            8: label=scripts kind=Field detail=object format=snippet
            9: label=test kind=Field detail=object format=snippet"#]],
    );
}

#[named]
#[test]
fn test_completion_test_coverage_format_default_value_in_string() {
    case_toml_completion(
        function_name!(),
        r#"
            [test]
            coverage-format = "<caret>foo"
        "#,
        expect![[r#"0: label="lcov" kind=Value detail=Default value format=plain"#]],
    );
}

#[named]
#[test]
fn test_apply_completion_root_string_key() {
    case_toml_completion_apply(
        function_name!(),
        r#"
            <caret>
        "#,
        &[
            "build",
            "contracts",
            "fmt",
            "lint",
            "litenode",
            "mappings",
            "networks",
            "package",
            "scripts",
            "test",
        ],
        0,
        r#"
            [build]
            <caret>
        "#,
    );
}

#[named]
#[test]
fn test_apply_completion_root_object_table() {
    case_toml_completion_apply(
        function_name!(),
        r#"
            <caret>
        "#,
        &[
            "build",
            "contracts",
            "fmt",
            "lint",
            "litenode",
            "mappings",
            "networks",
            "package",
            "scripts",
            "test",
        ],
        7,
        r#"
            [package]
            <caret>
        "#,
    );
}

#[named]
#[test]
fn test_apply_completion_table_header_name_only() {
    case_toml_completion_apply(
        function_name!(),
        r#"
            [<caret>]
        "#,
        &[
            "build",
            "contracts",
            "fmt",
            "lint",
            "litenode",
            "mappings",
            "networks",
            "package",
            "scripts",
            "test",
        ],
        0,
        r#"
            [build<caret>]
        "#,
    );
}

#[named]
#[test]
fn test_apply_completion_table_header_partial_replace() {
    case_toml_completion_apply(
        function_name!(),
        r#"
            [pac<caret>]
        "#,
        &[
            "build",
            "contracts",
            "fmt",
            "lint",
            "litenode",
            "mappings",
            "networks",
            "package",
            "scripts",
            "test",
        ],
        7,
        r#"
            [package<caret>]
        "#,
    );
}

#[named]
#[test]
fn test_apply_completion_value_in_string_literal() {
    case_toml_completion_apply(
        function_name!(),
        r#"
            [test]
            coverage = <caret>false
        "#,
        &["true", "false"],
        0,
        r#"
            [test]
            coverage = true<caret>
        "#,
    );
}

#[named]
#[test]
fn test_apply_completion_boolean_partial_replace() {
    case_toml_completion_apply(
        function_name!(),
        r#"
            [test]
            coverage = f<caret>alse
        "#,
        &["true", "false"],
        0,
        r#"
            [test]
            coverage = true<caret>
        "#,
    );
}

#[named]
#[test]
fn test_apply_completion_enum_value_in_string_literal() {
    case_toml_completion_apply(
        function_name!(),
        r#"
            [test]
            reporter = ["<caret>console"]
        "#,
        &["\"console\"", "\"teamcity\"", "\"junit\"", "\"dot\""],
        1,
        r#"
            [test]
            reporter = ["teamcity<caret>"]
        "#,
    );
}

#[named]
#[test]
fn test_apply_completion_enum_value_partial_replace() {
    case_toml_completion_apply(
        function_name!(),
        r#"
            [test]
            reporter = ["te<caret>am"]
        "#,
        &["\"console\"", "\"teamcity\"", "\"junit\"", "\"dot\""],
        1,
        r#"
            [test]
            reporter = ["teamcity<caret>"]
        "#,
    );
}

#[named]
#[test]
fn test_apply_completion_default_string_value_in_string_literal() {
    case_toml_completion_apply(
        function_name!(),
        r#"
            [test]
            coverage-format = "<caret>foo"
        "#,
        &["\"lcov\""],
        0,
        r#"
            [test]
            coverage-format = "lcov<caret>"
        "#,
    );
}
