use crate::integration::check::run_simple_test;
use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

#[test]
#[named]
fn test_check_write_only_variable() {
    run_simple_test(
        "write_only_variable",
        r#"
            fun main() {
                var counter = 0;
                counter = 1;
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_write_only_variable_ignores_mutable_parameters() {
    let project = ProjectBuilder::new(&format!("check-{}", function_name!()))
        .contract(
            "main",
            r#"
            fun foo(mutate a: int) {
                a = 100;
            }
        "#,
        )
        .build();

    project.acton().init().run().success();
    let output = project.acton().check().run().success();
    assert!(
        output.get_normalized_stderr().is_empty(),
        "expected no diagnostics for mutable parameter write-only scenario, got:\n{}",
        output.get_normalized_stderr()
    );
}
