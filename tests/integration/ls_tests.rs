use crate::support::{TestOutputExt, project::ProjectBuilder};
use std::fs;

#[test]
fn test_ls_ensure_latest_uses_project_root_from_nested_directory() {
    let project = ProjectBuilder::new("ls-ensure-latest-project-root").build();

    let nested_dir = project.path().join("nested");
    fs::create_dir_all(&nested_dir).expect("Failed to create nested test directory");

    let root_stdlib = project.path().join(".acton/tolk-stdlib");
    let nested_stdlib = nested_dir.join(".acton/tolk-stdlib");
    assert!(
        !root_stdlib.exists(),
        "stdlib must not exist before ls command"
    );
    assert!(
        !nested_stdlib.exists(),
        "stdlib must not exist in nested cwd before ls command"
    );

    let output = project
        .acton()
        .arg("--project-root")
        .arg("..")
        .arg("ls")
        .arg("--stdio")
        .arg("--no-log")
        .current_dir(&nested_dir)
        .run()
        .success();
    output.assert_not_contains("Installing standard library");
    output.assert_not_contains("Updating standard library");

    assert!(
        root_stdlib.exists(),
        "stdlib should be installed in project root"
    );
    assert!(
        !nested_stdlib.exists(),
        "stdlib should not be installed relative to nested cwd"
    );
}
