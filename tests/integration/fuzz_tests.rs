use crate::common::strip_ansi;
use crate::support::TestOutputExt;
use crate::support::project::{ProjectBuilder, TestConfig};

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

const EXPECT_IMPORT: &str = r#"
import "../../lib/testing/expect"
"#;

const FUZZ_IMPORTS: &str = r#"
import "../../lib/testing/expect"
import "../../lib/testing/fuzz"
"#;

fn with_imports(imports: &str, test_body: &str) -> String {
    format!("{imports}\n{test_body}\n")
}

fn fuzz_project(project_name: &str, source: &str) -> ProjectBuilder {
    ProjectBuilder::new(project_name)
        .contract("simple", SIMPLE_CONTRACT)
        .test_file("test", source)
}

fn snapshot_path(name: &str) -> String {
    format!("integration/snapshots/fuzz/{name}.stdout.txt")
}

fn run_seeded_success_snapshot(project_name: &str, source: &str, snapshot_name: &str) {
    let snapshot_path = snapshot_path(snapshot_name);
    fuzz_project(project_name, source)
        .build()
        .acton()
        .test()
        .arg("--fuzz-seed")
        .arg("42")
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(&snapshot_path);
}

fn run_seeded_failure_snapshot(project_name: &str, source: &str, snapshot_name: &str) {
    let snapshot_path = snapshot_path(snapshot_name);
    fuzz_project(project_name, source)
        .build()
        .acton()
        .test()
        .arg("--fuzz-seed")
        .arg("42")
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(&snapshot_path);
}

fn run_failure_snapshot(project_name: &str, source: &str, snapshot_name: &str) {
    let snapshot_path = snapshot_path(snapshot_name);
    fuzz_project(project_name, source)
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_snapshot_matches(&snapshot_path);
}

fn extract_seed(stdout: &str) -> u64 {
    strip_ansi(stdout)
        .lines()
        .find_map(|line| {
            let (_, tail) = line.rsplit_once("seed ")?;
            let digits = tail
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>();
            digits.parse::<u64>().ok()
        })
        .unwrap_or_else(|| panic!("fuzz seed not found in output:\n{stdout}"))
}

#[test]
fn legacy_fuzz_annotation_is_ignored() {
    run_failure_snapshot(
        "legacy-fuzz-annotation",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test({ fuzz: 1 })
            get fun `test legacy fuzz`(value: int) {
                expect(value).toEqual(value);
            }
        ",
        ),
        "legacy_annotation_is_ignored",
    );
}

#[test]
fn fuzz_runs_parameterized_test_multiple_times() {
    run_seeded_success_snapshot(
        "fuzz-parameterized",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz(4)
            get fun `test fuzz int`(value: int) {
                expect(value).toEqual(value);
            }
        ",
        ),
        "runs_parameterized_test_multiple_times",
    );
}

#[test]
fn fuzz_supports_int1_without_out_of_range_seed() {
    run_seeded_success_snapshot(
        "fuzz-int1",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz(4)
            get fun `test fuzz int1`(value: int1) {
                val raw = value as int;
                expect(raw == 0 || raw == -1).toBeTrue();
            }
        ",
        ),
        "supports_int1_without_out_of_range_seed",
    );
}

#[test]
fn fuzz_reports_failing_input() {
    run_seeded_failure_snapshot(
        "fuzz-failure",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz
            get fun `test fuzz bool`(flag: bool) {
                expect(flag).toBeFalse();
            }
        ",
        ),
        "reports_failing_input",
    );
}

#[test]
fn fuzz_true_uses_acton_toml_defaults() {
    let snapshot_path = snapshot_path("true_uses_acton_toml_defaults");
    fuzz_project(
        "fuzz-config-default-runs",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz
            get fun `test fuzz config runs`(value: int) {
                expect(value).toEqual(value);
            }
        ",
        ),
    )
    .with_test_config(TestConfig {
        fuzz_runs: Some(4),
        fuzz_seed: Some(17),
        ..TestConfig::default()
    })
    .build()
    .acton()
    .test()
    .run()
    .success()
    .assert_passed(1)
    .assert_snapshot_matches(&snapshot_path);
}

#[test]
fn fuzz_object_runs_path_uses_runs_override() {
    run_seeded_success_snapshot(
        "fuzz-object-runs",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz({ runs: 3 })
            get fun `test fuzz object runs`(value: int) {
                expect(value).toEqual(value);
            }
        ",
        ),
        "object_runs_path_uses_runs_override",
    );
}

#[test]
fn parameterized_test_requires_explicit_fuzz_annotation() {
    run_failure_snapshot(
        "fuzz-required",
        &with_imports(
            EXPECT_IMPORT,
            r"
            get fun `test missing fuzz`(value: int) {
                expect(value).toEqual(value);
            }
        ",
        ),
        "parameterized_test_requires_explicit_fuzz_annotation",
    );
}

#[test]
fn fuzz_false_does_not_enable_fuzzing() {
    run_failure_snapshot(
        "fuzz-false-does-not-enable",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz(false)
            get fun `test fuzz false does not enable`(value: int) {
                expect(value).toEqual(value);
            }
        ",
        ),
        "false_does_not_enable_fuzzing",
    );
}

#[test]
fn fuzz_annotation_requires_parameters() {
    run_failure_snapshot(
        "fuzz-no-params",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz
            get fun `test no params`() {
                expect(1).toEqual(1);
            }
        ",
        ),
        "annotation_requires_parameters",
    );
}

#[test]
fn fuzz_assume_retries_rejected_inputs() {
    run_seeded_success_snapshot(
        "fuzz-assume-retry",
        &with_imports(
            FUZZ_IMPORTS,
            r"
            @test.fuzz(2)
            get fun `test fuzz assume`(flag: bool) {
                fuzz.assume(flag);
                expect(flag).toBeTrue();
            }
        ",
        ),
        "assume_retries_rejected_inputs",
    );
}

#[test]
fn fuzz_assume_budget_exhaustion_reports_error() {
    run_seeded_failure_snapshot(
        "fuzz-assume-exhaustion",
        &with_imports(
            FUZZ_IMPORTS,
            r"
            @test.fuzz(1)
            get fun `test fuzz assume exhaustion`(value: int) {
                fuzz.assume(false);
                expect(value).toEqual(value);
            }
        ",
        ),
        "assume_budget_exhaustion_reports_error",
    );
}

#[test]
fn fuzz_assume_budget_uses_acton_toml_max_test_rejects() {
    let snapshot_path = snapshot_path("assume_budget_uses_acton_toml_max_test_rejects");
    fuzz_project(
        "fuzz-assume-config-exhaustion",
        &with_imports(
            FUZZ_IMPORTS,
            r"
            @test.fuzz
            get fun `test fuzz assume config exhaustion`(value: int) {
                fuzz.assume(false);
                expect(value).toEqual(value);
            }
        ",
        ),
    )
    .with_test_config(TestConfig {
        fuzz_max_test_rejects: Some(3),
        fuzz_seed: Some(17),
        ..TestConfig::default()
    })
    .build()
    .acton()
    .test()
    .run()
    .failure()
    .assert_failed(1)
    .assert_snapshot_matches(&snapshot_path);
}

#[test]
fn fuzz_max_test_rejects_without_runs_uses_config_runs() {
    let snapshot_path = snapshot_path("max_test_rejects_without_runs_uses_config_runs");
    fuzz_project(
        "fuzz-max-test-rejects-without-runs",
        &with_imports(
            FUZZ_IMPORTS,
            r"
            @test.fuzz({ max_test_rejects: 3 })
            get fun `test fuzz max test rejects without runs`(value: int) {
                fuzz.assume(false);
                expect(value).toEqual(value);
            }
        ",
        ),
    )
    .with_test_config(TestConfig {
        fuzz_runs: Some(5),
        fuzz_seed: Some(17),
        ..TestConfig::default()
    })
    .build()
    .acton()
    .test()
    .run()
    .failure()
    .assert_failed(1)
    .assert_snapshot_matches(&snapshot_path);
}

#[test]
fn fuzz_assume_budget_can_be_overridden_per_test() {
    let snapshot_path = snapshot_path("assume_budget_can_be_overridden_per_test");
    fuzz_project(
        "fuzz-assume-annotation-exhaustion",
        &with_imports(
            FUZZ_IMPORTS,
            r"
            @test.fuzz({ runs: 2, max_test_rejects: 3 })
            get fun `test fuzz assume annotation exhaustion`(value: int) {
                fuzz.assume(false);
                expect(value).toEqual(value);
            }
        ",
        ),
    )
    .with_test_config(TestConfig {
        fuzz_runs: Some(128),
        fuzz_max_test_rejects: Some(99),
        fuzz_seed: Some(17),
        ..TestConfig::default()
    })
    .build()
    .acton()
    .test()
    .run()
    .failure()
    .assert_failed(1)
    .assert_snapshot_matches(&snapshot_path);
}

#[test]
fn fuzz_same_seed_produces_same_values() {
    let snapshot_path = snapshot_path("same_seed_produces_same_values");
    let output = fuzz_project(
        "fuzz-same-seed",
        &with_imports(
            FUZZ_IMPORTS,
            r"
            @test.fuzz({ runs: 1, max_test_rejects: 32 })
            get fun `test fuzz seed a`(value: int8) {
                fuzz.assume(value != 0);
                fuzz.assume(value != 1);
                fuzz.assume(value != -1);
                fuzz.assume(value != 127);
                fuzz.assume(value != -128);
                expect(false).toBeTrue();
            }

            @test.fuzz({ runs: 1, max_test_rejects: 32 })
            get fun `test fuzz seed b`(value: int8) {
                fuzz.assume(value != 0);
                fuzz.assume(value != 1);
                fuzz.assume(value != -1);
                fuzz.assume(value != 127);
                fuzz.assume(value != -128);
                expect(false).toBeTrue();
            }
        ",
        ),
    )
    .build()
    .acton()
    .test()
    .arg("--fuzz-seed")
    .arg("777")
    .run()
    .failure();

    output
        .assert_failed(2)
        .assert_snapshot_matches(&snapshot_path);

    let stdout = strip_ansi(&output.get_stdout());
    let inputs = stdout
        .lines()
        .filter(|line| line.contains("Inputs: value="))
        .map(|line| line.trim().to_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        inputs.len(),
        2,
        "expected two fuzz input lines, got:\n{stdout}"
    );
    assert_eq!(inputs[0], inputs[1], "same seed should produce same values");
}

#[test]
fn fuzz_bits_parameter_is_not_supported() {
    run_failure_snapshot(
        "fuzz-bits-unsupported",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz
            get fun `test fuzz bits`(value: bits12) {
                expect(1).toEqual(1);
            }
        ",
        ),
        "bits_parameter_is_not_supported",
    );
}

#[test]
fn fuzz_bound_helper_wraps_values_into_range() {
    let snapshot_path = snapshot_path("bound_helper_wraps_values_into_range");
    fuzz_project(
        "fuzz-bound-helper",
        &with_imports(
            FUZZ_IMPORTS,
            r"
            get fun `test bound helper`() {
                expect(fuzz.bound(2, 1, 3)).toEqual(2);
                expect(fuzz.bound(0, 1, 3)).toEqual(3);
                expect(fuzz.bound(4, 1, 3)).toEqual(1);
                expect(fuzz.bound(5, 1, 3)).toEqual(2);

                val boundedUint = fuzz.bound(0 as uint32, 1 as uint32, 3 as uint32);
                expect(boundedUint as int).toEqual(3);
            }
        ",
        ),
    )
    .build()
    .acton()
    .test()
    .run()
    .success()
    .assert_passed(1)
    .assert_snapshot_matches(&snapshot_path);
}

#[test]
fn fuzz_supported_scalar_types_report_inputs() {
    run_seeded_failure_snapshot(
        "fuzz-supported-scalars",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz(1)
            get fun `test fuzz supported scalars`(amount: coins, count: uint32, label: string, flag: bool) {
                expect(false).toBeTrue();
            }
        ",
        ),
        "supported_scalar_types_report_inputs",
    );
}

#[test]
fn fuzz_supported_address_and_nullable_types_report_inputs() {
    run_seeded_failure_snapshot(
        "fuzz-supported-addresses-nullables",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz(1)
            get fun `test fuzz supported addresses nullables`(
                maybeText: string?,
                owner: address,
                target: any_address,
                maybeCount: int?,
                maybeOwner: address?
            ) {
                expect(false).toBeTrue();
            }
        ",
        ),
        "supported_address_and_nullable_types_report_inputs",
    );
}

#[test]
fn fuzz_address_parameters_are_passed_as_slices() {
    run_seeded_success_snapshot(
        "fuzz-address-slices",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz(1)
            get fun `test fuzz address slices`(owner: address, target: any_address) {
                expect(owner as any_address).toBeInternalAddress();
                expect(target).toBeInternalAddress();
            }
        ",
        ),
        "address_parameters_are_passed_as_slices",
    );
}

#[test]
fn fuzz_cli_seed_overrides_acton_toml_seed() {
    let snapshot_path = snapshot_path("cli_seed_overrides_acton_toml_seed");
    fuzz_project(
        "fuzz-cli-seed-overrides-config",
        &with_imports(
            FUZZ_IMPORTS,
            r"
            @test.fuzz({ runs: 1, max_test_rejects: 32 })
            get fun `test fuzz cli seed overrides config`(value: int8) {
                fuzz.assume(value != 0);
                fuzz.assume(value != 1);
                fuzz.assume(value != -1);
                fuzz.assume(value != 127);
                fuzz.assume(value != -128);
                expect(false).toBeTrue();
            }
        ",
        ),
    )
    .with_test_config(TestConfig {
        fuzz_seed: Some(11),
        ..TestConfig::default()
    })
    .build()
    .acton()
    .test()
    .arg("--fuzz-seed")
    .arg("42")
    .run()
    .failure()
    .assert_failed(1)
    .assert_snapshot_matches(&snapshot_path);
}

#[test]
fn fuzz_annotation_seed_overrides_cli_seed() {
    let snapshot_path = snapshot_path("annotation_seed_overrides_cli_seed");
    fuzz_project(
        "fuzz-annotation-seed-overrides-cli",
        &with_imports(
            FUZZ_IMPORTS,
            r"
            @test.fuzz({ runs: 1, max_test_rejects: 32, seed: 9 })
            get fun `test fuzz annotation seed overrides cli`(value: int8) {
                fuzz.assume(value != 0);
                fuzz.assume(value != 1);
                fuzz.assume(value != -1);
                fuzz.assume(value != 127);
                fuzz.assume(value != -128);
                expect(false).toBeTrue();
            }
        ",
        ),
    )
    .build()
    .acton()
    .test()
    .arg("--fuzz-seed")
    .arg("42")
    .run()
    .failure()
    .assert_failed(1)
    .assert_snapshot_matches(&snapshot_path);
}

#[test]
fn fuzz_annotation_seed_without_runs_uses_config_runs() {
    let snapshot_path = snapshot_path("annotation_seed_without_runs_uses_config_runs");
    fuzz_project(
        "fuzz-annotation-seed-with-config-runs",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz({ seed: 99 })
            get fun `test fuzz annotation seed with config runs`(value: int) {
                expect(value).toEqual(value);
            }
        ",
        ),
    )
    .with_test_config(TestConfig {
        fuzz_runs: Some(3),
        ..TestConfig::default()
    })
    .build()
    .acton()
    .test()
    .run()
    .success()
    .assert_passed(1)
    .assert_snapshot_matches(&snapshot_path);
}

#[test]
fn fuzz_zero_runs_in_annotation_is_rejected() {
    run_failure_snapshot(
        "fuzz-zero-runs-annotation",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz(0)
            get fun `test fuzz zero runs annotation`(value: int) {
                expect(value).toEqual(value);
            }
        ",
        ),
        "zero_runs_in_annotation_is_rejected",
    );
}

#[test]
fn fuzz_zero_max_test_rejects_in_config_is_rejected() {
    let snapshot_path = snapshot_path("zero_max_test_rejects_in_config_is_rejected");
    fuzz_project(
        "fuzz-zero-max-test-rejects-config",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz
            get fun `test fuzz zero max test rejects config`(value: int) {
                expect(value).toEqual(value);
            }
        ",
        ),
    )
    .with_test_config(TestConfig {
        fuzz_max_test_rejects: Some(0),
        ..TestConfig::default()
    })
    .build()
    .acton()
    .test()
    .run()
    .failure()
    .assert_failed(1)
    .assert_snapshot_matches(&snapshot_path);
}

#[test]
fn fuzz_run_seed_changes_between_runs_when_unset() {
    let project = fuzz_project(
        "fuzz-random-run-seed",
        &with_imports(
            EXPECT_IMPORT,
            r"
            @test.fuzz(1)
            get fun `test fuzz random run seed`(value: int) {
                expect(value).toEqual(value);
            }
        ",
        ),
    )
    .build();

    let first = project.acton().test().run().success();
    let second = project.acton().test().run().success();

    first.assert_passed(1).assert_contains("seed ");
    second.assert_passed(1).assert_contains("seed ");

    let first_seed = extract_seed(&first.get_stdout());
    let second_seed = extract_seed(&second.get_stdout());

    assert_ne!(
        first_seed, second_seed,
        "expected different run-level fuzz seeds when no override is set"
    );
}
