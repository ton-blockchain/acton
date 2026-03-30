use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

fn run_random_requires_initialization_test(content: &str, name: &str) {
    let project = ProjectBuilder::new(&format!("check-{name}"))
        .contract("main", content)
        .with_lint_level("random-requires-initialization", "warn")
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E024")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/random_requires_initialization/{name}.txt"
        ));
}

fn run_random_requires_initialization_test_with_files(
    main_content: &str,
    files: &[(&str, &str)],
    name: &str,
) {
    let mut builder = ProjectBuilder::new(&format!("check-{name}"))
        .contract("main", main_content)
        .with_lint_level("random-requires-initialization", "warn");

    for (path, content) in files {
        builder = builder.file(path, content);
    }

    let project = builder.build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .arg("--enable-only")
        .arg("E024")
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!(
            "integration/snapshots/check/random_requires_initialization/{name}.txt"
        ));
}

#[test]
#[named]
fn test_check_random_requires_initialization_reports_uint256_without_initialize() {
    run_random_requires_initialization_test(
        r"
            fun main() {
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_reports_range_without_initialize() {
    run_random_requires_initialization_test(
        r"
            fun main() {
                val _value = random.range(10);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_allows_after_initialize() {
    run_random_requires_initialization_test(
        r"
            fun main() {
                random.initialize();
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_allows_after_initialize_by() {
    run_random_requires_initialization_test(
        r"
            fun main() {
                random.initializeBy(1);
                val _value = random.range(100);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_reports_if_only_one_branch_initializes() {
    run_random_requires_initialization_test(
        r"
            fun main(cond: bool) {
                if (cond) {
                    random.initializeBy(7);
                }
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_allows_if_all_branches_initialize() {
    run_random_requires_initialization_test(
        r"
            fun main(cond: bool) {
                if (cond) {
                    random.initialize();
                } else {
                    random.initializeBy(2);
                }
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_allows_when_helper_guarantees_initialize() {
    run_random_requires_initialization_test(
        r"
            fun initRnd() {
                random.initializeBy(77);
            }

            fun main() {
                initRnd();
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_reports_when_helper_initializes_conditionally() {
    run_random_requires_initialization_test(
        r"
            fun initRnd(cond: bool) {
                if (cond) {
                    random.initializeBy(77);
                }
            }

            fun main(cond: bool) {
                initRnd(cond);
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_allows_through_nested_helper_chain() {
    run_random_requires_initialization_test(
        r"
            fun initRndInner() {
                random.initializeBy(123);
            }

            fun initRnd() {
                initRndInner();
            }

            fun main() {
                initRnd();
                val _value = random.range(1000);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_reports_when_initialize_happens_after_sink() {
    run_random_requires_initialization_test(
        r"
            fun main() {
                val _value = random.uint256();
                random.initializeBy(1);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_allows_when_non_initialized_path_returns_before_sink()
{
    run_random_requires_initialization_test(
        r"
            fun main(cond: bool) {
                if (cond) {
                    return;
                }
                random.initialize();
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_reports_when_helper_has_early_return_without_init() {
    run_random_requires_initialization_test(
        r"
            fun initRnd(cond: bool) {
                if (cond) {
                    return;
                }
                random.initializeBy(1);
            }

            fun main(cond: bool) {
                initRnd(cond);
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_allows_when_helper_initializes_on_all_branches() {
    run_random_requires_initialization_test(
        r"
            fun initRnd(cond: bool) {
                if (cond) {
                    random.initialize();
                    return;
                }
                random.initializeBy(1);
            }

            fun main(cond: bool) {
                initRnd(cond);
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_reports_when_recursive_initializer_not_guaranteed() {
    run_random_requires_initialization_test(
        r"
            fun initRnd(depth: int): void {
                if (depth == 0) {
                    return;
                }
                if (depth == 1) {
                    random.initialize();
                    return;
                }
                initRnd(depth - 1);
            }

            fun main(depth: int) {
                initRnd(depth);
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_allows_when_second_helper_guarantees_init() {
    run_random_requires_initialization_test(
        r"
            fun initMaybe(cond: bool) {
                if (cond) {
                    random.initialize();
                }
            }

            fun initSure() {
                random.initializeBy(3);
            }

            fun main(cond: bool) {
                initMaybe(cond);
                initSure();
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_reports_when_init_and_sink_control_flow_split() {
    run_random_requires_initialization_test(
        r"
            fun main(a: bool, b: bool) {
                if (a) {
                    random.initializeBy(1);
                }
                if (b) {
                    return;
                }
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_allows_cross_file_helper_with_guaranteed_init() {
    run_random_requires_initialization_test_with_files(
        r#"
            import "./helpers/init.tolk";

            fun main() {
                initRnd();
                val _value = random.uint256();
            }
        "#,
        &[(
            "contracts/helpers/init",
            r"
                fun initRnd() {
                    random.initializeBy(9);
                }
            ",
        )],
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_reports_cross_file_helper_with_conditional_init() {
    run_random_requires_initialization_test_with_files(
        r#"
            import "./helpers/init.tolk";

            fun main(cond: bool) {
                initRnd(cond);
                val _value = random.uint256();
            }
        "#,
        &[(
            "contracts/helpers/init",
            r"
                fun initRnd(cond: bool) {
                    if (cond) {
                        random.initializeBy(9);
                    }
                }
            ",
        )],
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_reports_for_each_sink_without_init() {
    run_random_requires_initialization_test(
        r"
            fun main(cond: bool) {
                if (cond) {
                    random.initialize();
                }
                val _first = random.uint256();
                val _second = random.range(100);
            }
        ",
        function_name!(),
    );
}

#[test]
#[named]
fn test_check_random_requires_initialization_allows_when_non_initialized_path_throws_before_sink() {
    run_random_requires_initialization_test(
        r"
            fun main(cond: bool) {
                if (cond) {
                    throw 100;
                }
                random.initializeBy(1);
                val _value = random.uint256();
            }
        ",
        function_name!(),
    );
}
