use crate::support::TestOutputExt;
use crate::support::project::ProjectBuilder;

const INVALID_CONTRACT: &str = r"
fun main( {
";

fn has_ansi_escape(text: &str) -> bool {
    text.contains('\u{1b}')
}

#[test]
fn test_color_always_disasm_error_contains_ansi_and_matches_svg_snapshot() {
    let project = ProjectBuilder::new("color-disasm-always").build();

    let output = project
        .acton()
        .arg("--color")
        .arg("always")
        .disasm()
        .arg("--string")
        .arg("aaa")
        .arg("file.boc")
        .run()
        .failure();

    let stderr = output.get_stderr();
    assert!(
        has_ansi_escape(&stderr),
        "Expected ANSI escape sequences in stderr for --color always, got:\n{stderr}"
    );

    output.assert_stderr_svg_snapshot_matches(
        "integration/snapshots/color/test_color_always_disasm_error.stderr.svg",
    );
}

#[test]
fn test_color_never_disasm_error_has_no_ansi() {
    let project = ProjectBuilder::new("color-disasm-never").build();

    let output = project
        .acton()
        .arg("--color")
        .arg("never")
        .disasm()
        .arg("--string")
        .arg("aaa")
        .arg("file.boc")
        .run()
        .failure();

    let stderr = output.get_stderr();
    assert!(
        !has_ansi_escape(&stderr),
        "Expected no ANSI escape sequences in stderr for --color never, got:\n{stderr}"
    );
}

#[test]
fn test_color_always_check_diagnostics_contains_ansi_with_no_color_env() {
    let project = ProjectBuilder::new("color-check-always")
        .contract("main", INVALID_CONTRACT)
        .build();

    project.acton().init().run().success();

    let output = project
        .acton()
        .arg("--color")
        .arg("always")
        .check()
        .run()
        .success();

    let stderr = output.get_stderr();
    assert!(
        has_ansi_escape(&stderr),
        "Expected ANSI escape sequences in check diagnostics for --color always, got:\n{stderr}"
    );

    output.assert_stderr_svg_snapshot_matches(
        "integration/snapshots/color/test_color_always_check_diagnostics.stderr.svg",
    );
}

#[test]
fn test_color_never_check_diagnostics_has_no_ansi() {
    let project = ProjectBuilder::new("color-check-never")
        .contract("main", INVALID_CONTRACT)
        .build();

    project.acton().init().run().success();

    let output = project
        .acton()
        .arg("--color")
        .arg("never")
        .check()
        .run()
        .success();

    let stderr = output.get_stderr();
    assert!(
        !has_ansi_escape(&stderr),
        "Expected no ANSI escape sequences in check diagnostics for --color never, got:\n{stderr}"
    );
}

#[test]
fn test_color_auto_disasm_force_color_env_enables_ansi_on_non_tty() {
    let project = ProjectBuilder::new("color-auto-force-color").build();

    let output = project
        .acton()
        .keep_color_env()
        .env_remove("NO_COLOR")
        .env("FORCE_COLOR", "1")
        .env("CLICOLOR_FORCE", "0")
        .disasm()
        .arg("--string")
        .arg("aaa")
        .arg("file.boc")
        .run()
        .failure();

    let stderr = output.get_stderr();
    assert!(
        has_ansi_escape(&stderr),
        "Expected ANSI escape sequences in stderr for auto mode with FORCE_COLOR=1, got:\n{stderr}"
    );

    output.assert_stderr_svg_snapshot_matches(
        "integration/snapshots/color/test_color_auto_force_color_disasm_error.stderr.svg",
    );
}

#[test]
fn test_color_auto_disasm_without_force_has_no_ansi_on_non_tty() {
    let project = ProjectBuilder::new("color-auto-no-force").build();

    let output = project
        .acton()
        .keep_color_env()
        .env_remove("NO_COLOR")
        .env("FORCE_COLOR", "0")
        .env("CLICOLOR_FORCE", "0")
        .disasm()
        .arg("--string")
        .arg("aaa")
        .arg("file.boc")
        .run()
        .failure();

    let stderr = output.get_stderr();
    assert!(
        !has_ansi_escape(&stderr),
        "Expected no ANSI escape sequences in stderr for auto mode without forcing on non-TTY, got:\n{stderr}"
    );
}

#[test]
fn test_color_auto_disasm_no_color_overrides_force_color() {
    let project = ProjectBuilder::new("color-auto-no-color-overrides-force").build();

    let output = project
        .acton()
        .keep_color_env()
        .env("NO_COLOR", "1")
        .env("FORCE_COLOR", "1")
        .env("CLICOLOR_FORCE", "1")
        .disasm()
        .arg("--string")
        .arg("aaa")
        .arg("file.boc")
        .run()
        .failure();

    let stderr = output.get_stderr();
    assert!(
        !has_ansi_escape(&stderr),
        "Expected NO_COLOR to disable ANSI escape sequences in auto mode even with FORCE_COLOR, got:\n{stderr}"
    );
}
