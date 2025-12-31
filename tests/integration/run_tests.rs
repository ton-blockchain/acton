use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

#[test]
#[cfg_attr(not(unix), ignore)]
fn test_run_simple_script() {
    let project = ProjectBuilder::new("run-simple")
        .script_config("hello", "echo Hello, world!")
        .build();

    project
        .acton()
        .run_script_cmd("hello")
        .run()
        .code(0)
        .assert_contains("Hello, world!");
}

#[test]
#[cfg_attr(not(unix), ignore)]
fn test_run_script_with_args() {
    let project = ProjectBuilder::new("run-args")
        .script_config("greet", "echo Hello,")
        .build();

    project
        .acton()
        .run_script_cmd("greet")
        .arg("world!")
        .run()
        .code(0)
        .assert_contains("Hello, world!");
}

#[test]
#[cfg_attr(not(unix), ignore)]
fn test_run_unknown_script() {
    let project = ProjectBuilder::new("run-unknown")
        .script_config("dummy", "echo dummy")
        .build();

    project
        .acton()
        .run_script_cmd("unknown")
        .run()
        .code(1)
        .assert_stderr_contains("Script unknown not found in Acton.toml");
}

#[test]
fn test_run_no_scripts_section() {
    let project = ProjectBuilder::new("run-no-scripts").build();

    project
        .acton()
        .run_script_cmd("unknown")
        .run()
        .code(1)
        .assert_stderr_contains("No [scripts] section found in Acton.toml");
}

#[test]
#[cfg_attr(not(unix), ignore)]
fn test_run_failing_script() {
    let project = ProjectBuilder::new("run-fail")
        .script_config("fail", "exit 1")
        .build();

    project.acton().run_script_cmd("fail").run().code(1);
}
