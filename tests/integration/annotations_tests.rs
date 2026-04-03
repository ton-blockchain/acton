use crate::support::TestOutputExt;
use crate::support::project::{ProjectBuilder, TestConfig};

const SIMPLE_CONTRACT: &str = r"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn test_skip_annotation_string_literal() {
    ProjectBuilder::new("skip-string")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test("skip")
            get fun `test-skipped-string`() {
                expect(1).toEqual(2); // This should not run
            }

            get fun `test-not-skipped`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_skipped(1)
        .assert_contains("skipped");
}

#[test]
fn test_skip_annotation_object_literal() {
    ProjectBuilder::new("skip-object")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ skip: true })
            get fun `test-skipped-object`() {
                expect(1).toEqual(2); // This should not run
            }

            get fun `test-not-skipped`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_skipped(1)
        .assert_contains("skipped");
}

#[test]
fn test_todo_annotation_string_literal() {
    ProjectBuilder::new("todo-string")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test("todo")
            get fun `test-todo-string`() {
                expect(1).toEqual(2); // This should not run
            }

            get fun `test-not-todo`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_todo(1)
        .assert_contains("TODO");
}

#[test]
fn test_todo_annotation_with_description() {
    ProjectBuilder::new("todo-description")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ todo: "Implement this later" })
            get fun `test-todo-described`() {
                expect(1).toEqual(2); // This should not run
            }

            get fun `test-not-todo`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_todo(1)
        .assert_contains("Implement this later");
}

#[test]
fn test_todo_annotation_boolean() {
    ProjectBuilder::new("todo-boolean")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ todo: true })
            get fun `test-todo-boolean`() {
                expect(1).toEqual(2); // This should not run
            }

            get fun `test-not-todo`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_todo(1)
        .assert_contains("TODO");
}

/// Test @test({ `gas_limit`: 100 }) annotation
#[test]
fn test_gas_limit_annotation() {
    ProjectBuilder::new("gas-limit")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ gas_limit: 100 })
            get fun `test-gas-limit-exceeded`() {
                // This loop should exceed the gas limit
                var i = 0;
                while (i < 1000) {
                    i = i + 1;
                }
                expect(1).toEqual(1); // Should not reach here
            }

            get fun `test-normal-gas`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_passed(1)
        .assert_failed(1)
        .assert_contains("Gas limit exceeded");
}

/// Test @test({ `fail_with`: 42 }) annotation
#[test]
fn test_fail_with_annotation() {
    ProjectBuilder::new("fail-with")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ fail_with: 42 })
            get fun `test-expected-failure`() {
                throw 42; // This is expected
            }

            get fun `test-normal-pass`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(2)
        .assert_not_contains("Expected exit_code");
}

/// Test @test({ `fail_with`: 42 }) annotation with wrong exit code
#[test]
fn test_fail_with_annotation_wrong_code() {
    ProjectBuilder::new("fail-with-wrong")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ fail_with: 42 })
            get fun `test-wrong-exit-code`() {
                throw 99; // Expected 42, got 99
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Expected exit_code=42, got=99");
}

/// Test multiple annotations combined
#[test]
fn test_multiple_annotations() {
    ProjectBuilder::new("multiple-annotations")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ skip: true, gas_limit: 1000, fail_with: 10 })
            get fun `test-multiple-annotations`() {
                // This should be skipped, so these annotations don't matter
                throw 10;
            }

            get fun `test-normal`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_skipped(1)
        .assert_not_contains("Gas limit exceeded")
        .assert_not_contains("Expected exit_code");
}

/// Test that annotations can be used with filters
#[test]
fn test_annotations_with_filter() {
    ProjectBuilder::new("annotations-filter")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ skip: true })
            get fun `test-skipped-1`() {
                expect(1).toEqual(2);
            }

            @test({ skip: true })
            get fun `test-skipped-2`() {
                expect(1).toEqual(2);
            }

            get fun `test-not-skipped`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .filter("test-not-skipped")
        .run()
        .success()
        .assert_passed(1);
}

#[test]
fn test_fuzz_annotation_runs_parameterized_test_multiple_times() {
    ProjectBuilder::new("fuzz-parameterized")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ fuzz: 4 })
            get fun `test-fuzz-int`(value: int) {
                expect(value).toEqual(value);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("(4 runs)");
}

#[test]
fn test_fuzz_annotation_reports_failing_input() {
    ProjectBuilder::new("fuzz-failure")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ fuzz: true })
            get fun `test-fuzz-bool`(flag: bool) {
                expect(flag).toBeFalse();
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Fuzz case 2/256")
        .assert_contains("Inputs: flag=true");
}

#[test]
fn test_fuzz_annotation_true_uses_acton_toml_defaults() {
    ProjectBuilder::new("fuzz-config-default-runs")
        .contract("simple", SIMPLE_CONTRACT)
        .with_test_config(TestConfig {
            fuzz_runs: Some(4),
            ..TestConfig::default()
        })
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ fuzz: true })
            get fun `test-fuzz-config-runs`(value: int) {
                expect(value).toEqual(value);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("(4 runs)");
}

#[test]
fn test_parameterized_test_requires_explicit_fuzz_annotation() {
    ProjectBuilder::new("fuzz-required")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            get fun `test-missing-fuzz`(value: int) {
                expect(value).toEqual(value);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("requires @test({ fuzz: true }) or @test({ fuzz: <runs> })");
}

#[test]
fn test_fuzz_annotation_requires_parameters() {
    ProjectBuilder::new("fuzz-no-params")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ fuzz: true })
            get fun `test-no-params`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("uses @test({ fuzz: ... }) but has no parameters");
}

#[test]
fn test_fuzz_assume_retries_rejected_inputs() {
    ProjectBuilder::new("fuzz-assume-retry")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/testing/fuzz"

            @test({ fuzz: 2 })
            get fun `test-fuzz-assume`(flag: bool) {
                fuzz.assume(flag);
                expect(flag).toBeTrue();
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("(2 runs)");
}

#[test]
fn test_fuzz_assume_budget_exhaustion_reports_clear_error() {
    ProjectBuilder::new("fuzz-assume-exhaustion")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/testing/fuzz"

            @test({ fuzz: 1 })
            get fun `test-fuzz-assume-exhaustion`(value: int) {
                fuzz.assume(false);
                expect(value).toEqual(value);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("assume(...) rejected 256 fuzz inputs before reaching 1 successful runs");
}

#[test]
fn test_fuzz_assume_budget_uses_acton_toml_max_test_rejects() {
    ProjectBuilder::new("fuzz-assume-config-exhaustion")
        .contract("simple", SIMPLE_CONTRACT)
        .with_test_config(TestConfig {
            fuzz_max_test_rejects: Some(3),
            ..TestConfig::default()
        })
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/testing/fuzz"

            @test({ fuzz: true })
            get fun `test-fuzz-assume-config-exhaustion`(value: int) {
                fuzz.assume(false);
                expect(value).toEqual(value);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("assume(...) rejected 3 fuzz inputs before reaching 256 successful runs");
}

#[test]
fn test_fuzz_bits_parameter_is_not_supported() {
    ProjectBuilder::new("fuzz-bits-unsupported")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"

            @test({ fuzz: true })
            get fun `test-fuzz-bits`(value: bits12) {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .failure()
        .assert_failed(1)
        .assert_contains("Fuzzing parameter 'value' of type 'bits12' is not supported yet");
}

#[test]
fn test_bound_helper_wraps_values_into_range() {
    ProjectBuilder::new("fuzz-bound-helper")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            import "../../lib/testing/fuzz"

            get fun `test-bound-helper`() {
                expect(fuzz.bound(2, 1, 3)).toEqual(2);
                expect(fuzz.bound(0, 1, 3)).toEqual(3);
                expect(fuzz.bound(4, 1, 3)).toEqual(1);
                expect(fuzz.bound(5, 1, 3)).toEqual(2);

                val boundedUint = fuzz.bound(0 as uint32, 1 as uint32, 3 as uint32);
                expect(boundedUint as int).toEqual(3);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1);
}
