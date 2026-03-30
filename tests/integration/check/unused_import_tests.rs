use crate::integration::check::{run_rule_check_test_with_files, run_rule_fix_test_with_files};
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

const RULE_CODE: &str = "E006";

fn run_check_test_with_files(group: &str, main_content: &str, files: &[(&str, &str)], name: &str) {
    run_rule_check_test_with_files(group, RULE_CODE, main_content, files, name);
}

fn run_fix_test_with_files(before: &str, after: &str, files: &[(&str, &str)], name: &str) {
    run_rule_fix_test_with_files(RULE_CODE, before, after, files, name);
}

const FUNCTIONS_FILE: &str = r"
    fun fromFunction(): int {
        return 1;
    }
";

const CONSTANTS_FILE: &str = r"
    const FROM_CONST = 10
";

const TYPES_FILE: &str = r"
    type FromAlias = int
";

const STRUCTS_FILE: &str = r"
    struct FromStruct {
        value: int,
    }
";

const METHODS_FILE: &str = r"
    fun int.bump(self): int {
        return self + 1;
    }
";

const UNUSED_FILE: &str = r"
    fun notUsed(): int {
        return 0;
    }
";

const UNUSED_FILE_2: &str = r"
    const NEVER_USED = 42
";

#[test]
#[named]
fn test_check_unused_import_with_all_symbol_usage_variants() {
    run_check_test_with_files(
        "unused_import",
        r#"
            import "./functions.tolk";
            import "./constants.tolk";
            import "./types.tolk";
            import "./structs.tolk";
            import "./methods.tolk";
            import "./unused.tolk";

            fun main() {
                fromFunction();
                FROM_CONST;

                val aliasValue: FromAlias = 1;
                aliasValue;

                val structValue = FromStruct { value: 1 };
                structValue;

                1.bump();
            }
        "#,
        &[
            ("contracts/functions", FUNCTIONS_FILE),
            ("contracts/constants", CONSTANTS_FILE),
            ("contracts/types", TYPES_FILE),
            ("contracts/structs", STRUCTS_FILE),
            ("contracts/methods", METHODS_FILE),
            ("contracts/unused", UNUSED_FILE),
        ],
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_unused_import_with_multiple_files() {
    run_fix_test_with_files(
        r#"
            import "./functions.tolk";
            import "./constants.tolk";
            import "./types.tolk";
            import "./structs.tolk";
            import "./methods.tolk";
            import "./unused.tolk";

            fun main() {
                fromFunction();
                FROM_CONST;

                val aliasValue: FromAlias = 1;
                aliasValue;

                val structValue = FromStruct { value: 1 };
                structValue;

                1.bump();
            }
        "#,
        r#"
            import "./functions.tolk";
            import "./constants.tolk";
            import "./types.tolk";
            import "./structs.tolk";
            import "./methods.tolk";

            fun main() {
                fromFunction();
                FROM_CONST;

                val aliasValue: FromAlias = 1;
                aliasValue;

                val structValue = FromStruct { value: 1 };
                structValue;

                1.bump();
            }
        "#,
        &[
            ("contracts/functions", FUNCTIONS_FILE),
            ("contracts/constants", CONSTANTS_FILE),
            ("contracts/types", TYPES_FILE),
            ("contracts/structs", STRUCTS_FILE),
            ("contracts/methods", METHODS_FILE),
            ("contracts/unused", UNUSED_FILE),
        ],
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_unused_import_without_indentation_and_with_trailing_spaces() {
    run_fix_test_with_files(
        "import \"./unused.tolk\";   \nimport \"./functions.tolk\";\n\nfun main() {\n    fromFunction();\n}\n",
        "import \"./functions.tolk\";\n\nfun main() {\n    fromFunction();\n}\n",
        &[
            ("contracts/functions", FUNCTIONS_FILE),
            ("contracts/unused", UNUSED_FILE),
        ],
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_unused_import_with_inline_comment() {
    run_fix_test_with_files(
        "import \"./unused.tolk\"; // keep this file import only if used\nimport \"./functions.tolk\";\n\nfun main() {\n    fromFunction();\n}\n",
        "import \"./functions.tolk\";\n\nfun main() {\n    fromFunction();\n}\n",
        &[
            ("contracts/functions", FUNCTIONS_FILE),
            ("contracts/unused", UNUSED_FILE),
        ],
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_unused_import_with_several_unused_imports() {
    run_fix_test_with_files(
        r#"
            import "./unused.tolk";
            import "./functions.tolk";
            import "./unused2.tolk";

            fun main() {
                fromFunction();
            }
        "#,
        r#"
            import "./functions.tolk";

            fun main() {
                fromFunction();
            }
        "#,
        &[
            ("contracts/functions", FUNCTIONS_FILE),
            ("contracts/unused", UNUSED_FILE),
            ("contracts/unused2", UNUSED_FILE_2),
        ],
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_unused_import_two_imports_on_one_line() {
    run_check_test_with_files(
        "unused_import",
        r#"
            import "./unused.tolk"; import "./functions.tolk";

            fun main() {
                fromFunction();
            }
        "#,
        &[
            ("contracts/functions", FUNCTIONS_FILE),
            ("contracts/unused", UNUSED_FILE),
        ],
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_unused_import_two_imports_on_one_line_does_not_change_file() {
    run_fix_test_with_files(
        r#"
            import "./unused.tolk"; import "./functions.tolk";

            fun main() {
                fromFunction();
            }
        "#,
        r#"
            import "./unused.tolk"; import "./functions.tolk";

            fun main() {
                fromFunction();
            }
        "#,
        &[
            ("contracts/functions", FUNCTIONS_FILE),
            ("contracts/unused", UNUSED_FILE),
        ],
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_unused_import_with_mappings() {
    let project = ProjectBuilder::new("check-unused-import-with-mappings")
        .mapping("libs", "./libs")
        .file("libs/functions", FUNCTIONS_FILE)
        .file("libs/unused", UNUSED_FILE)
        .contract(
            "main",
            r#"
            import "@libs/functions";
            import "@libs/unused";

            fun main() {
                fromFunction();
            }
        "#,
        )
        .build();

    project.acton().init().run().success();
    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg(RULE_CODE)
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/unused_import/{}.txt",
            function_name!()
        ));
}

#[test]
fn test_fix_unused_import_with_mappings() {
    let project = ProjectBuilder::new("check-fix-unused-import-with-mappings")
        .mapping("libs", "./libs")
        .file("libs/functions", FUNCTIONS_FILE)
        .file("libs/unused", UNUSED_FILE)
        .contract(
            "main",
            r#"
            import "@libs/functions";
            import "@libs/unused";

            fun main() {
                fromFunction();
            }
        "#,
        )
        .build();

    project.acton().init().run().success();
    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg(RULE_CODE)
        .arg("--fix")
        .run()
        .success();

    let main_file = project.path().join("contracts/main.tolk");
    let actual = std::fs::read_to_string(&main_file)
        .unwrap_or_else(|e| panic!("failed to read fixed file '{}': {}", main_file.display(), e));

    assert_eq!(
        actual.trim(),
        r#"
            import "@libs/functions";

            fun main() {
                fromFunction();
            }
        "#
        .trim()
    );
}

#[test]
#[named]
fn test_fix_unused_import_in_shared_file_used_by_multiple_targets() {
    let project = ProjectBuilder::new("check-fix-unused-import-shared-multi-target")
        .with_lint_level("unused-import", "warn")
        .contract(
            "main_a",
            r#"import "./shared.tolk";

fun main() {
    sharedHelper();
}
"#,
        )
        .contract(
            "main_b",
            r#"import "./shared.tolk";

fun main() {
    sharedHelper();
}
"#,
        )
        .file(
            "contracts/shared",
            r#"import "./unused.tolk";
import "./functions.tolk";

fun sharedHelper(): int {
    return fromFunction();
}
"#,
        )
        .file("contracts/functions", FUNCTIONS_FILE)
        .file("contracts/unused", UNUSED_FILE)
        .build();

    project.acton().init().run().success();
    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg(RULE_CODE)
        .arg("--fix")
        .run()
        .success()
        .assert_file_snapshot_matches(
            "contracts/shared.tolk",
            &format!(
                "integration/snapshots/check/unused_import/{}.shared.tolk.txt",
                function_name!()
            ),
        );
}
