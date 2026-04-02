use crate::support::TestOutputExt;
use crate::support::project::{Project, ProjectBuilder};

const MUTATION_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {
    assert (in.valueCoins > 0) throw 5;
}

fun onBouncedMessage(_: InMessageBounced) {}

get fun addOne(x: int): int {
    return x + 1;
}
";

const PASSING_TEST: &str = r#"
import "../../lib/testing/expect"

get fun `test-always-pass`() {
    expect(1).toEqual(1);
}
"#;

const DEPENDENT_MUTATION_CONTRACT: &str = r#"
import "../gen/dependency_code.tolk"

fun onInternalMessage(in: InMessage) {
    assert (in.valueCoins > 0) throw 5;
    val code = dependencyCompiledCode();
}

fun onBouncedMessage(_: InMessageBounced) {}
"#;

const BROKEN_DEPENDENCY_MUTATION_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {
    THIS IS A SYNTAX ERROR
}

fun onBouncedMessage(_: InMessageBounced) {}
";

const COMPILE_ERROR_MUTATION_CONTRACT: &str = r"
get fun mustFail(): int {
    throw 5;
}

get fun addOne(x: int): int {
    return x + 1;
}

fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const NO_MUTATION_POINTS_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

fn mutation_project(name: &str) -> Project {
    ProjectBuilder::new(name)
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .build()
}

#[test]
fn mutate_requires_mutate_contract() {
    mutation_project("j-mutate-requires-contract")
        .acton()
        .test()
        .arg("--mutate")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_requires_mutate_contract.stderr.txt",
        );
}

#[test]
fn mutate_fails_for_unknown_contract() {
    mutation_project("j-mutate-unknown-contract")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("missing")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_fails_for_unknown_contract.stderr.txt",
        );
}

#[test]
fn mutate_reports_summary() {
    mutation_project("j-mutate-summary")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_reports_summary.stdout.txt",
        );
}

#[test]
fn mutate_disable_rule_filters_mutants() {
    mutation_project("j-mutate-disable-rule")
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .arg("--disable-rule")
        .arg("remove_assert")
        .arg("--disable-rule")
        .arg("flip_plus")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_disable_rule_filters_mutants.stdout.txt",
        );
}

#[test]
fn mutate_uses_disable_rules_from_config() {
    ProjectBuilder::new("j-mutate-config-disable-rules")
        .without_acton_toml()
        .contract("simple", MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .raw_file(
            "Acton.toml",
            r#"[package]
name = "j-mutate-config-disable-rules"
description = "A test project"
version = "0.1.0"

[contracts.simple]
name = "simple"
src = "contracts/simple.tolk"

[test.mutation]
disable-rules = ["remove_assert", "flip_plus", "flip_gt_ge"]
"#,
        )
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("simple")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_uses_disable_rules_from_config.stdout.txt",
        );
}

#[test]
fn mutate_contract_with_dependencies() {
    ProjectBuilder::new("j-mutate-contract-with-dependencies")
        .contract("dependency", MUTATION_CONTRACT)
        .contract_with_deps("main", DEPENDENT_MUTATION_CONTRACT, vec!["dependency"])
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_contract_with_dependencies.stdout.txt",
        );
}

#[test]
fn mutate_contract_with_library_ref_dependency() {
    ProjectBuilder::new("j-mutate-contract-with-library-ref-dependency")
        .contract("dependency", MUTATION_CONTRACT)
        .contract_with_detailed_deps(
            "main",
            DEPENDENT_MUTATION_CONTRACT,
            vec![("dependency", Some("library_ref"), None, None)],
        )
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_contract_with_library_ref_dependency.stdout.txt",
        );
}

#[test]
fn mutate_contract_with_dependencies_and_clear_cache() {
    ProjectBuilder::new("j-mutate-contract-with-dependencies-clear-cache")
        .contract("dependency", MUTATION_CONTRACT)
        .contract_with_deps("main", DEPENDENT_MUTATION_CONTRACT, vec!["dependency"])
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .clear_cache()
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_contract_with_dependencies_and_clear_cache.stdout.txt",
        );
}

#[test]
fn mutate_reports_dependency_build_failure() {
    ProjectBuilder::new("j-mutate-dependency-build-failure")
        .contract("dependency", BROKEN_DEPENDENCY_MUTATION_CONTRACT)
        .contract_with_deps("main", DEPENDENT_MUTATION_CONTRACT, vec!["dependency"])
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .failure()
        .assert_stderr_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_reports_dependency_build_failure.stderr.txt",
        );
}

#[test]
fn mutate_compile_errors_are_excluded_from_score() {
    ProjectBuilder::new("j-mutate-compile-errors-excluded-from-score")
        .contract("main", COMPILE_ERROR_MUTATION_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_compile_errors_are_excluded_from_score.stdout.txt",
        );
}

#[test]
fn mutate_reports_no_mutation_points() {
    ProjectBuilder::new("j-mutate-no-mutation-points")
        .contract("main", NO_MUTATION_POINTS_CONTRACT)
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .success()
        .assert_snapshot_matches(
            "integration/snapshots/test-runner/test_runner_mutate/mutate_reports_no_mutation_points.stdout.txt",
        );
}

#[test]
#[ignore = "benchmark scenario for local perf tracking"]
fn mutate_benchmark_large_mutant_set() {
    use std::fmt::Write as _;
    use std::time::Instant;

    let mut asserts = String::new();
    for _ in 0..120 {
        writeln!(&mut asserts, "    assert (in.valueCoins > 0) throw 5;")
            .expect("write benchmark contract");
    }

    let contract = format!(
        r"
fun onInternalMessage(in: InMessage) {{
{asserts}
}}

fun onBouncedMessage(_: InMessageBounced) {{}}
"
    );

    let start = Instant::now();
    let output = ProjectBuilder::new("j-mutate-benchmark-large-mutant-set")
        .contract("main", &contract)
        .test_file("mutation", PASSING_TEST)
        .build()
        .acton()
        .test()
        .arg("--mutate")
        .arg("--mutate-contract")
        .arg("main")
        .run()
        .success();
    let elapsed = start.elapsed();

    let stdout = output.get_normalized_stdout();
    let mutant_count = stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("Mutants:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .expect("mutation output must contain mutant count");

    assert!(
        mutant_count >= 100,
        "expected at least 100 mutants in benchmark scenario, got {mutant_count}\n{stdout}"
    );

    if let Some(max_ms) = std::env::var("MUTATION_BENCH_MAX_MS")
        .ok()
        .and_then(|value| value.parse::<u128>().ok())
    {
        assert!(
            elapsed.as_millis() <= max_ms,
            "mutation benchmark regression: elapsed={}ms exceeds MUTATION_BENCH_MAX_MS={}ms",
            elapsed.as_millis(),
            max_ms
        );
    }

    eprintln!("mutation benchmark: {mutant_count} mutants processed in {elapsed:?}");
}
