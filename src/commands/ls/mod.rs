use crate::paths;
use acton_config::config::project_root as configured_project_root;
use std::env;
use std::path::{Path, PathBuf};
use ton_language_server_native::{LogLevel, NativeLoggingConfig, ServerConfig};

pub async fn ls_cmd(
    port: Option<u16>,
    stdio: bool,
    log_file: Option<String>,
    no_log: bool,
    log_level: String,
    stdlib_path: Option<PathBuf>,
    profile: bool,
) -> anyhow::Result<()> {
    let log_level = log_level.parse::<LogLevel>()?;
    let project_root = configured_project_root().to_path_buf();
    let tolk_stdlib_root = resolve_tolk_stdlib_root(&project_root, stdlib_path)?;
    let logging = if no_log {
        None
    } else {
        Some(NativeLoggingConfig::new(
            log_file.map_or_else(
                || paths::language_server_log_path(configured_project_root()),
                PathBuf::from,
            ),
            log_level,
        ))
    };
    let config = ServerConfig {
        project_root,
        tolk_stdlib_root,
        logging,
        enable_profiling: profile || cfg!(feature = "profiling"),
    };

    match (port, stdio) {
        (Some(port), _) => ton_language_server_native::serve_tcp(config, port).await,
        (None, true) | (None, false) => ton_language_server_native::serve_stdio(config).await,
    }
}

fn resolve_tolk_stdlib_root(
    project_root: &Path,
    stdlib_path: Option<PathBuf>,
) -> anyhow::Result<Option<PathBuf>> {
    resolve_tolk_stdlib_root_from_candidates(
        stdlib_path,
        default_tolk_stdlib_candidates(project_root),
    )
}

fn resolve_tolk_stdlib_root_from_candidates(
    stdlib_path: Option<PathBuf>,
    candidates: Vec<PathBuf>,
) -> anyhow::Result<Option<PathBuf>> {
    if let Some(path) = stdlib_path {
        if !path.is_dir() {
            anyhow::bail!("Tolk stdlib path is not a directory: {}", path.display());
        }
        return Ok(Some(dunce::canonicalize(path)?));
    }

    if let Some(path) = find_existing_tolk_stdlib_candidate(&candidates)? {
        return Ok(Some(path));
    }

    Ok(candidates.into_iter().next())
}

fn default_tolk_stdlib_candidates(project_root: &Path) -> Vec<PathBuf> {
    default_tolk_stdlib_candidates_with_env(
        project_root,
        path_from_env("TEST_TOLK_STDLIB_PATH"),
        path_from_env("TOLK_STDLIB"),
        platform_tolk_stdlib_candidates(),
    )
}

fn default_tolk_stdlib_candidates_with_env(
    project_root: &Path,
    test_stdlib_path: Option<PathBuf>,
    tolk_stdlib_path: Option<PathBuf>,
    platform_candidates: Vec<PathBuf>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = test_stdlib_path {
        candidates.push(path);
    }
    candidates.extend([
        project_root.join(".acton").join("tolk-stdlib"),
        project_root
            .join("node_modules")
            .join("@ton")
            .join("tolk-js")
            .join("dist")
            .join("tolk-stdlib"),
        project_root.join("stdlib"),
        project_root.join("tolk-stdlib"),
    ]);
    if let Some(path) = tolk_stdlib_path {
        candidates.push(path);
    }
    candidates.extend(platform_candidates);
    candidates
}

fn find_existing_tolk_stdlib_candidate(candidates: &[PathBuf]) -> anyhow::Result<Option<PathBuf>> {
    candidates
        .iter()
        .find(|path| path.is_dir())
        .map(dunce::canonicalize)
        .transpose()
        .map_err(Into::into)
}

fn path_from_env(name: &str) -> Option<PathBuf> {
    let value = env::var_os(name)?;
    (!value.is_empty()).then_some(PathBuf::from(value))
}

fn platform_tolk_stdlib_candidates() -> Vec<PathBuf> {
    if cfg!(target_os = "linux") {
        vec![PathBuf::from("/usr/share/ton/smartcont/tolk-stdlib")]
    } else if cfg!(target_os = "macos") {
        vec![
            PathBuf::from("/opt/homebrew/share/ton/ton/smartcont/tolk-stdlib"),
            PathBuf::from("/usr/local/share/ton/ton/smartcont/tolk-stdlib"),
        ]
    } else if cfg!(target_os = "windows") {
        vec![PathBuf::from(
            "C:\\ProgramData\\chocolatey\\lib\\ton\\smartcont\\tolk-stdlib",
        )]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn explicit_tolk_stdlib_path_wins_over_defaults() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let default_path = dir.path().join(".acton").join("tolk-stdlib");
        let explicit_path = dir.path().join("custom-stdlib");
        fs::create_dir_all(&default_path)?;
        fs::create_dir_all(&explicit_path)?;

        let resolved = resolve_tolk_stdlib_root(dir.path(), Some(explicit_path.clone()))?;

        assert_eq!(resolved, Some(dunce::canonicalize(explicit_path)?));
        Ok(())
    }

    #[test]
    fn default_tolk_stdlib_candidates_match_ton_vscode_order() {
        let root = Path::new("/workspace");

        let candidates = default_tolk_stdlib_candidates_with_env(
            root,
            Some(PathBuf::from("/test-stdlib")),
            Some(PathBuf::from("/env-stdlib")),
            vec![PathBuf::from("/platform-stdlib")],
        );

        assert_eq!(
            candidates,
            vec![
                PathBuf::from("/test-stdlib"),
                PathBuf::from("/workspace/.acton/tolk-stdlib"),
                PathBuf::from("/workspace/node_modules/@ton/tolk-js/dist/tolk-stdlib"),
                PathBuf::from("/workspace/stdlib"),
                PathBuf::from("/workspace/tolk-stdlib"),
                PathBuf::from("/env-stdlib"),
                PathBuf::from("/platform-stdlib"),
            ]
        );
    }

    #[test]
    fn finds_first_existing_tolk_stdlib_candidate() -> anyhow::Result<()> {
        let dir = tempfile::tempdir()?;
        let node_stdlib = dir
            .path()
            .join("node_modules")
            .join("@ton")
            .join("tolk-js")
            .join("dist")
            .join("tolk-stdlib");
        let lower_priority_stdlib = dir.path().join("stdlib");
        fs::create_dir_all(&node_stdlib)?;
        fs::create_dir_all(&lower_priority_stdlib)?;

        let candidates = vec![
            dir.path().join(".acton").join("tolk-stdlib"),
            node_stdlib.clone(),
            lower_priority_stdlib,
        ];
        let resolved = find_existing_tolk_stdlib_candidate(&candidates)?;

        assert_eq!(resolved, Some(dunce::canonicalize(node_stdlib)?));
        Ok(())
    }

    #[test]
    fn returns_first_default_candidate_when_stdlib_is_missing() -> anyhow::Result<()> {
        let first_candidate = PathBuf::from("/missing/.acton/tolk-stdlib");
        let resolved = resolve_tolk_stdlib_root_from_candidates(
            None,
            vec![
                first_candidate.clone(),
                PathBuf::from("/missing/node_modules/@ton/tolk-js/dist/tolk-stdlib"),
            ],
        )?;

        assert_eq!(resolved, Some(first_candidate));
        Ok(())
    }
}
