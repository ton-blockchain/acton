use expect_test::expect;
use std::fs;
use ton_language_server_native::resolve_tolk_stdlib_root;

#[test]
fn resolves_project_and_explicit_stdlib_paths() -> anyhow::Result<()> {
    let project = tempfile::tempdir()?;
    let project_without_stdlib = tempfile::tempdir()?;
    let project_stdlib = project.path().join(".acton/tolk-stdlib");
    let node_stdlib = project
        .path()
        .join("node_modules/@ton/tolk-js/dist/tolk-stdlib");
    let explicit_stdlib = project.path().join("custom-stdlib");
    fs::create_dir_all(&project_stdlib)?;
    fs::create_dir_all(&node_stdlib)?;
    fs::create_dir_all(&explicit_stdlib)?;
    let project_root = project.path();

    let automatic = resolve_tolk_stdlib_root(project.path(), None)?;
    let missing_automatic = resolve_tolk_stdlib_root(project_without_stdlib.path(), None)?;
    let explicit = resolve_tolk_stdlib_root(project.path(), Some(explicit_stdlib))?;
    let invalid = resolve_tolk_stdlib_root(project.path(), Some(project.path().join("missing")))
        .expect_err("a missing explicit stdlib must be rejected");
    let actual = format!(
        "automatic={}\nmissing automatic={}\nexplicit={}\ninvalid={}",
        automatic
            .as_deref()
            .and_then(|path| path.strip_prefix(project_root).ok())
            .map_or_else(String::new, |path| path.display().to_string()),
        missing_automatic
            .as_deref()
            .is_none_or(std::path::Path::is_dir),
        explicit
            .as_deref()
            .and_then(|path| path.strip_prefix(project_root).ok())
            .map_or_else(String::new, |path| path.display().to_string()),
        invalid
            .to_string()
            .replace(&project.path().display().to_string(), "$ROOT"),
    );

    expect![["
        automatic=.acton/tolk-stdlib
        missing automatic=true
        explicit=custom-stdlib
        invalid=Tolk stdlib path is not a directory: $ROOT/missing"]]
    .assert_eq(&actual);
    Ok(())
}
