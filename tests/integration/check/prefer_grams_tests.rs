use crate::integration::check::run_rule_test;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

const RULE_CODE: &str = "S009";
const GROUP: &str = "prefer_grams";

#[test]
#[named]
fn test_check_prefer_grams_reports_stdlib_references() {
    run_rule_test(
        GROUP,
        RULE_CODE,
        r#"
            const amount = ton("0.1");
            struct Payment {
                value: coins = ton("0.2")
            }
            fun main(): coins {
                return ton("0.3") + grams("0.4");
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_fix_prefer_grams_preserves_arguments_and_comments() {
    let project = ProjectBuilder::new(function_name!())
        .contract(
            "main",
            r#"const amount = ton /* keep this comment */ ("0.100");
fun main(): coins {
    // ton("0.2") in a comment stays unchanged
    val text = "ton(0.3)";
    return `ton`("0.4") + amount;
}
"#,
        )
        .build();
    project.acton().init().run().success();

    for _ in 0..2 {
        project
            .acton()
            .check()
            .arg("--enable-only")
            .arg(RULE_CODE)
            .arg("--fix")
            .run()
            .success()
            .assert_file_snapshot_matches(
                "contracts/main.tolk",
                "integration/snapshots/check/prefer_grams/fixed.tolk",
            );
    }
}

#[test]
#[named]
fn test_check_prefer_grams_ignores_user_methods() {
    run_rule_test(
        GROUP,
        RULE_CODE,
        r#"
            struct Wallet {}
            fun Wallet.ton(self, value: string): coins { return 2; }
            fun main(): coins {
                return Wallet {}.ton("0.2");
            }
        "#,
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_prefer_grams_reports_shadowed_grams_without_fixes() {
    let project = ProjectBuilder::new(function_name!())
        .contract(
            "main",
            r#"fun parameter(grams: int): coins { return ton("0.1"); }
fun main(): coins {
    val ton = fun(value: string): coins { return 1; };
    val grams = 2;
    return ton("0.2");
}
fun captured(): coins {
    val grams = 3;
    val callback = fun(): coins { return ton("0.3"); };
    return callback();
}
fun caught(): coins {
    try { throw 1; }
    catch (grams) { return ton("0.4"); }
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
        .success()
        .assert_stderr_snapshot_matches("integration/snapshots/check/prefer_grams/shadowed.txt")
        .assert_file_snapshot_matches(
            "contracts/main.tolk",
            "integration/snapshots/check/prefer_grams/shadowed.tolk",
        );
}

#[test]
#[named]
fn test_check_prefer_grams_respects_local_scope_boundaries() {
    let project = ProjectBuilder::new(function_name!())
        .contract(
            "main",
            r#"fun other(grams: int) {}
fun main(): coins {
    val before = ton("0.1");
    if (true) {
        val grams = 1;
        val shadowed = ton("0.2");
    }
    val after = ton("0.3");
    val grams = 2;
    return ton("0.4");
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
        .assert_stderr_snapshot_matches(
            "integration/snapshots/check/prefer_grams/test_check_prefer_grams_respects_local_scope_boundaries.txt",
        );

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg(RULE_CODE)
        .arg("--fix")
        .run()
        .success()
        .assert_file_snapshot_matches(
            "contracts/main.tolk",
            "integration/snapshots/check/prefer_grams/scope_boundaries.tolk",
        );
}

#[test]
#[named]
fn test_check_prefer_grams_ignores_old_stdlib() {
    let project = ProjectBuilder::new(function_name!())
        .contract("main", r#"fun main(): coins { return ton("0.1"); }"#)
        .build();
    project.acton().init().run().success();

    let common = project.path().join(".acton/tolk-stdlib/common.tolk");
    let source = std::fs::read_to_string(&common).unwrap();
    let source = source.replace(
        "@pure\nfun grams(floatString: string): coins\n    builtin",
        "",
    );
    std::fs::write(common, source).unwrap();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg(RULE_CODE)
        .run()
        .success()
        .assert_stderr_snapshot_matches("integration/snapshots/check/prefer_grams/old_stdlib.txt");
}

#[test]
#[named]
fn test_check_prefer_grams_does_not_duplicate_deprecation() {
    let project = ProjectBuilder::new(function_name!())
        .contract(
            "main",
            r#"
                fun main(): coins { return ton("0.1"); }
                fun shadowed(grams: int): coins { return ton("0.2"); }
            "#,
        )
        .build();
    project.acton().init().run().success();

    let common = project.path().join(".acton/tolk-stdlib/common.tolk");
    let source = std::fs::read_to_string(&common).unwrap();
    let source = source.replace(
        "@pure\nfun ton(",
        "@deprecated(\"use grams instead\")\n@pure\nfun ton(",
    );
    std::fs::write(common, source).unwrap();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("S009,E003")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/check/prefer_grams/deprecated_with_prefer_grams.txt",
        );

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E003")
        .run()
        .success()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/check/prefer_grams/deprecated_without_prefer_grams.txt",
        );
}

#[test]
#[named]
fn test_check_prefer_grams_supports_inline_suppression() {
    run_rule_test(
        GROUP,
        RULE_CODE,
        r#"
            fun main(): coins {
                // check-disable-next-line prefer-grams
                return ton("0.1");
            }
        "#,
        function_name!(),
    );
}
