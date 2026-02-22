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
