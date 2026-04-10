use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

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
            get fun `test skipped string`() {
                expect(1).toEqual(2); // This should not run
            }

            get fun `test not skipped`() {
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
            get fun `test skipped object`() {
                expect(1).toEqual(2); // This should not run
            }

            get fun `test not skipped`() {
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
            get fun `test todo string`() {
                expect(1).toEqual(2); // This should not run
            }

            get fun `test not todo`() {
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
            get fun `test todo described`() {
                expect(1).toEqual(2); // This should not run
            }

            get fun `test not todo`() {
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
            get fun `test todo boolean`() {
                expect(1).toEqual(2); // This should not run
            }

            get fun `test not todo`() {
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
            get fun `test gas limit exceeded`() {
                // This loop should exceed the gas limit
                var i = 0;
                while (i < 1000) {
                    i = i + 1;
                }
                expect(1).toEqual(1); // Should not reach here
            }

            get fun `test normal gas`() {
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
            get fun `test expected failure`() {
                throw 42; // This is expected
            }

            get fun `test normal pass`() {
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
            get fun `test wrong exit code`() {
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
            get fun `test multiple annotations`() {
                // This should be skipped, so these annotations don't matter
                throw 10;
            }

            get fun `test normal`() {
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
            get fun `test skipped 1`() {
                expect(1).toEqual(2);
            }

            @test({ skip: true })
            get fun `test skipped 2`() {
                expect(1).toEqual(2);
            }

            get fun `test not skipped`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .filter("test not skipped")
        .run()
        .success()
        .assert_passed(1);
}
