use crate::integration::check::run_fix_test_with_mappings;
use crate::integration::check::run_simple_test_with_mappings;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

const LIB_MATH: &str = r#"
    fun plusOne(value: int): int {
        return value + 1;
    }
"#;

#[test]
#[named]
fn test_check_import_path_can_use_mappings_for_relative_import() {
    run_simple_test_with_mappings(
        "import_path_can_use_mappings",
        r#"
            import "../libs/math.tolk";

            fun main() {
                plusOne(1);
            }
        "#,
        &[("libs/math", LIB_MATH)],
        &[("libs", "./libs")],
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_import_path_can_use_mappings_for_relative_import_without_dots() {
    run_simple_test_with_mappings(
        "import_path_can_use_mappings",
        r#"
            import "libs/math.tolk";

            fun main() {
                plusOne(1);
            }
        "#,
        &[("contracts/libs/math", LIB_MATH)],
        &[("libs", "./libs")],
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_import_path_can_use_mappings_prefers_more_specific_mapping() {
    run_simple_test_with_mappings(
        "import_path_can_use_mappings",
        r#"
            import "../libs/utils/math.tolk";

            fun main() {
                plusTwo(1);
            }
        "#,
        &[(
            "libs/utils/math",
            r#"
                fun plusTwo(value: int): int {
                    return value + 2;
                }
            "#,
        )],
        &[("libs", "./libs"), ("utils", "./libs/utils")],
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_import_path_can_use_mappings_for_relative_import() {
    run_fix_test_with_mappings(
        r#"
            import "../libs/math.tolk";

            fun main() {
                plusOne(1);
            }
        "#,
        r#"
            import "@libs/math";

            fun main() {
                plusOne(1);
            }
        "#,
        &[("libs/math", LIB_MATH)],
        &[("libs", "./libs")],
        function_name!(),
    );
}

#[test]
fn test_check_import_path_can_use_mappings_skips_already_mapped_import() {
    let project = ProjectBuilder::new("check-import-path-can-use-mappings-already-mapped")
        .mapping("libs", "./libs")
        .file("libs/math", LIB_MATH)
        .contract(
            "main",
            r#"
            import "@libs/math";

            fun main() {
                plusOne(1);
            }
        "#,
        )
        .build();

    project.acton().init().run().success();

    let output = project.acton().check().run().success();
    assert!(
        !output.get_normalized_stderr().contains("E018"),
        "E018 should not be emitted for imports that already use mappings:\n{}",
        output.get_normalized_stderr()
    );
}
