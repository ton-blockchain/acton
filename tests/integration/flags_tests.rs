use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const SIMPLE_CONTRACT: &str = r#"
fun onInternalMessage(in: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
"#;

#[test]
fn test_run_specific_test_file() {
    let project = ProjectBuilder::new("multi-file")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test1",
            r#"
            import "../../lib/testing/expect"
            
            get fun `test-in-file-1`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .test_file(
            "test2",
            r#"
            import "../../lib/testing/expect"
            
            get fun `test-in-file-2`() {
                expect(2).toEqual(2);
            }
        "#,
        )
        .build();

    // Run only test1.tolk
    project
        .acton()
        .test()
        .path("tests/test1_test.tolk")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("in file 1")
        .assert_not_contains("in file 2");
}

#[test]
fn test_filter_by_name() {
    ProjectBuilder::new("filtered")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            
            get fun `test-unit-1`() {
                expect(1).toEqual(1);
            }
            
            get fun `test-unit-2`() {
                expect(2).toEqual(2);
            }
            
            get fun `test-other`() {
                expect(3).toEqual(3);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .filter("test-unit-.*")
        .run()
        .success()
        .assert_passed(2)
        .assert_contains("unit 1")
        .assert_contains("unit 2")
        .assert_not_contains("other");
}

#[test]
fn test_filter_single_test() {
    ProjectBuilder::new("single-filter")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            
            get fun `test-alpha`() {
                expect(1).toEqual(1);
            }
            
            get fun `test-beta`() {
                expect(2).toEqual(2);
            }
            
            get fun `test-gamma`() {
                expect(3).toEqual(3);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .filter("test-beta")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("beta")
        .assert_not_contains("alpha")
        .assert_not_contains("gamma");
}

#[test]
fn test_combined_path_and_filter() {
    let project = ProjectBuilder::new("path-and-filter")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "unit_tests",
            r#"
            import "../../lib/testing/expect"
            
            get fun `test-unit-counter-test`() {
                expect(1).toEqual(1);
            }
            
            get fun `test-unit-wallet-test`() {
                expect(2).toEqual(2);
            }
        "#,
        )
        .test_file(
            "integration_tests",
            r#"
            import "../../lib/testing/expect"
            
            get fun `test-integration-counter-test`() {
                expect(3).toEqual(3);
            }
        "#,
        )
        .build();

    // Run only unit_tests.tolk with counter filter
    project
        .acton()
        .test()
        .path("tests/unit_tests_test.tolk")
        .filter(".*counter.*")
        .run()
        .success()
        .assert_passed(1)
        .assert_contains("unit counter test")
        .assert_not_contains("unit wallet test")
        .assert_not_contains("integration counter test");
}

#[test]
fn test_filter_with_no_matches() {
    ProjectBuilder::new("no-match")
        .contract("simple", SIMPLE_CONTRACT)
        .test_file(
            "test",
            r#"
            import "../../lib/testing/expect"
            
            get fun `test-alpha`() {
                expect(1).toEqual(1);
            }
        "#,
        )
        .build()
        .acton()
        .test()
        .filter("non-existent-test")
        .run()
        .success()
        .assert_passed(0);
}
