use anyhow::{Context, anyhow};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

pub const DEFAULT_BUILD_CACHE_DIR: &str = "build/cache";
pub const DEFAULT_BUILD_TRACES_DIR: &str = "build/traces";
pub const DEFAULT_BUILD_MUTATION_SESSIONS_DIR: &str = "build/mutation-sessions";
pub const DEFAULT_BUILD_LOGS_DIR: &str = "build/logs";
pub const DEFAULT_LANGUAGE_SERVER_LOG_PATH: &str = "build/logs/tolk-language-server.log";

#[must_use]
pub fn build_dir(project_root: &Path) -> PathBuf {
    project_root.join("build")
}

#[must_use]
pub fn build_cache_dir(project_root: &Path) -> PathBuf {
    build_dir(project_root).join("cache")
}

#[must_use]
pub fn build_traces_dir(project_root: &Path) -> PathBuf {
    build_dir(project_root).join("traces")
}

#[must_use]
pub fn build_mutation_sessions_dir(project_root: &Path) -> PathBuf {
    build_dir(project_root).join("mutation-sessions")
}

#[must_use]
pub fn build_logs_dir(project_root: &Path) -> PathBuf {
    build_dir(project_root).join("logs")
}

#[must_use]
pub fn language_server_log_path(project_root: &Path) -> PathBuf {
    build_logs_dir(project_root).join("tolk-language-server.log")
}

pub(crate) fn resolve_manifest_write_path(
    project_root: &Path,
    raw_path: &str,
    field_name: &str,
) -> anyhow::Result<PathBuf> {
    let raw_path = raw_path.trim();
    if raw_path.is_empty()
        || has_windows_prefix(raw_path)
        || raw_path.starts_with('\\')
        || raw_path.starts_with("//")
    {
        return Err(manifest_write_path_error(field_name));
    }

    let mut relative = PathBuf::new();
    for component in Path::new(raw_path).components() {
        match component {
            Component::Normal(value) => relative.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(manifest_write_path_error(field_name));
            }
        }
    }

    let target = project_root.join(&relative);
    let canonical_project_root = project_root.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize project root {}",
            project_root.display()
        )
    })?;
    validate_existing_manifest_components(
        project_root,
        &relative,
        &canonical_project_root,
        field_name,
    )?;
    let existing_ancestor = nearest_existing_ancestor(&target).ok_or_else(|| {
        anyhow!(
            "failed to find an existing ancestor for manifest path {}",
            target.display()
        )
    })?;
    let canonical_ancestor = existing_ancestor.canonicalize().with_context(|| {
        format!(
            "failed to canonicalize existing path {}",
            existing_ancestor.display()
        )
    })?;

    if !canonical_ancestor.starts_with(&canonical_project_root) {
        return Err(manifest_write_path_error(field_name));
    }

    Ok(target)
}

fn has_windows_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn manifest_write_path_error(field_name: &str) -> anyhow::Error {
    anyhow!(
        "{field_name} must be a project-relative path inside the project root; use an explicit CLI output flag for paths outside the project"
    )
}

fn validate_existing_manifest_components(
    project_root: &Path,
    relative_path: &Path,
    canonical_project_root: &Path,
    field_name: &str,
) -> anyhow::Result<()> {
    let mut current = project_root.to_path_buf();
    for component in relative_path.components() {
        current.push(component.as_os_str());
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == io::ErrorKind::NotFound => break,
            Err(err) => {
                return Err(anyhow!(
                    "failed to inspect manifest path {}: {}",
                    current.display(),
                    err
                ));
            }
        };

        if metadata.file_type().is_symlink() {
            let Ok(canonical_path) = current.canonicalize() else {
                return Err(manifest_write_path_error(field_name));
            };
            if !canonical_path.starts_with(canonical_project_root) {
                return Err(manifest_write_path_error(field_name));
            }
        }
    }

    Ok(())
}

fn nearest_existing_ancestor(path: &Path) -> Option<&Path> {
    let mut cursor = Some(path);
    while let Some(path) = cursor {
        if path.exists() {
            return Some(path);
        }
        cursor = path.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::resolve_manifest_write_path;

    #[test]
    fn manifest_write_path_accepts_project_relative_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_root = temp_dir.path();

        assert_eq!(
            resolve_manifest_write_path(project_root, "artifacts", "[build].out-dir").unwrap(),
            project_root.join("artifacts")
        );
        assert_eq!(
            resolve_manifest_write_path(project_root, "./artifacts/gen", "[build].gen-dir")
                .unwrap(),
            project_root.join("artifacts").join("gen")
        );
        assert_eq!(
            resolve_manifest_write_path(
                project_root,
                "wrappers-ts",
                "[wrappers.typescript].output-dir"
            )
            .unwrap(),
            project_root.join("wrappers-ts")
        );
    }

    #[test]
    fn manifest_write_path_rejects_paths_outside_project_root() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_root = temp_dir.path();

        for path in [
            "/tmp/out",
            r"\tmp\out",
            r"C:\tmp\out",
            "C:tmp",
            r"\\server\share\out",
            "../out",
            "artifacts/../../out",
        ] {
            let err = resolve_manifest_write_path(project_root, path, "[build].out-dir")
                .expect_err("path should be rejected");
            assert!(
                err.to_string()
                    .contains("must be a project-relative path inside the project root"),
                "unexpected error for {path}: {err}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn manifest_write_path_rejects_existing_symlink_parent_escape() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().unwrap();
        let outside_dir = tempfile::tempdir().unwrap();
        let project_root = temp_dir.path();
        symlink(outside_dir.path(), project_root.join("linked")).unwrap();

        let err = resolve_manifest_write_path(
            project_root,
            "linked/generated.tolk",
            "[wrappers.tolk].output-dir",
        )
        .expect_err("symlinked parent should be rejected");

        assert!(
            err.to_string()
                .contains("must be a project-relative path inside the project root"),
            "unexpected error: {err}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn manifest_write_path_rejects_dangling_symlink_target() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().unwrap();
        let outside_dir = tempfile::tempdir().unwrap();
        let project_root = temp_dir.path();
        symlink(
            outside_dir.path().join("missing-output"),
            project_root.join("dangling"),
        )
        .unwrap();

        let err = resolve_manifest_write_path(project_root, "dangling", "[build].out-dir")
            .expect_err("dangling symlink target should be rejected");

        assert!(
            err.to_string()
                .contains("must be a project-relative path inside the project root"),
            "unexpected error: {err}"
        );
    }
}
