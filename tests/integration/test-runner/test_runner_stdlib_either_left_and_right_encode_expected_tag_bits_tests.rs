use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const EITHER_TEST_IMPORTS: &str = r#"
import "../../lib/tlb/either"
import "../../lib/testing/expect"
"#;

fn run_either_case(project_name: &str, test_body: &str, snapshot_path: &str) {
    let source = format!(
        r#"
            {}

            {}
        "#,
        EITHER_TEST_IMPORTS, test_body
    );

    ProjectBuilder::new(project_name)
        .test_file("either_behavior", &source)
        .build()
        .acton()
        .test()
        .run()
        .success()
        .assert_passed(1)
        .assert_snapshot_matches(snapshot_path);
}

#[test]
fn either_left_and_right_encode_expected_tag_bits() {
    run_either_case(
        "ab-stdlib-either-tags",
        r#"
        get fun `test-ab-stdlib-either-tags`() {
            val left = Either<uint32, uint32>.left(17);
            val right = Either<uint32, uint32>.right(99);

            var leftSlice = left.toCell().beginParse();
            expect(leftSlice.loadUint(1)).toEqual(0);
            expect(leftSlice.loadUint(32)).toEqual(17);

            var rightSlice = right.toCell().beginParse();
            expect(rightSlice.loadUint(1)).toEqual(1);
            expect(rightSlice.loadUint(32)).toEqual(99);
        }
        "#,
        "integration/snapshots/test-runner/test_runner_stdlib_either_left_and_right_encode_expected_tag_bits_tests/either_left_and_right_encode_expected_tag_bits.stdout.txt",
    );
}

#[test]
fn either_match_routes_left_and_right_variants() {
    run_either_case(
        "ab-stdlib-either-match",
        r#"
        fun branchScore(choice: Either<uint32, uint32>): int {
            return match (choice) {
                EitherLeft => 1000 + choice.value,
                EitherRight => 2000 + choice.value,
            };
        }

        get fun `test-ab-stdlib-either-match-routes-variants`() {
            val left = Either<uint32, uint32>.left(11);
            val right = Either<uint32, uint32>.right(11);

            expect(branchScore(left)).toEqual(1011);
            expect(branchScore(right)).toEqual(2011);
        }
        "#,
        "integration/snapshots/test-runner/test_runner_stdlib_either_left_and_right_encode_expected_tag_bits_tests/either_match_routes_left_and_right_variants.stdout.txt",
    );
}
