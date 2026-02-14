use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;
use function_name::named;

pub(crate) fn run_simple_test(group: &str, content: &str, name: &str) {
    let project = ProjectBuilder::new(&format!("check-{}", name))
        .contract("main", content)
        .build();

    project.acton().init().run().success();

    project
        .acton()
        .check()
        .run()
        .success()
        .assert_stderr_snapshot_matches(&format!("integration/snapshots/check/{group}/{name}.txt"));
}

#[test]
#[named]
fn test_check_deprecated_function_use() {
    run_simple_test(
        "deprecated",
        r#"
            @deprecated
            fun foo() {}

            fun main() {
                foo();
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_deprecated_function_use_with_message() {
    run_simple_test(
        "deprecated",
        r#"
            @deprecated("use bar instead")
            fun foo() {}

            fun main() {
                foo();
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_deprecated_struct_use() {
    run_simple_test(
        "deprecated",
        r#"
            @deprecated("use Bar instead")
            struct Foo {}

            fun main(_a: Foo) {
                Foo {}
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_deprecated_static_method_use() {
    run_simple_test(
        "deprecated",
        r#"
            struct Foo {}

            @deprecated("use Foo.baz instead")
            fun Foo.bar() {}

            fun main() {
                Foo.bar();
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_deprecated_instance_method_use() {
    run_simple_test(
        "deprecated",
        r#"
            struct Foo {}

            @deprecated("use Foo.baz instead")
            fun Foo.bar(self) {}

            fun main() {
                Foo{}.bar();
            }
        "#,
        function_name!(),
    )
}

#[test]
#[named]
fn test_check_deprecated_global_var_use() {
    run_simple_test(
        "deprecated",
        r#"
            @deprecated
            global foo: int

            fun main() {
                foo;
            }
        "#,
        function_name!(),
    )
}
