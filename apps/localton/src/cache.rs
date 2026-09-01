//! Shared per-user cache paths for downloaded Localton resources.

use std::{env, ffi::OsString, path::PathBuf};

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use anyhow::bail;
use anyhow::{Context, Result};

const CACHE_DIR_ENV: &str = "LOCALTON_CACHE_DIR";

/// Resolves the shared cache used by resources reusable across state directories.
///
/// `LOCALTON_CACHE_DIR` owns the explicit override. Platform cache conventions
/// are used otherwise so downloads never become part of durable network state.
pub(crate) fn root() -> Result<PathBuf> {
    root_from(
        env::var_os(CACHE_DIR_ENV),
        env::var_os("HOME"),
        env::var_os("XDG_CACHE_HOME"),
    )
}

fn root_from(
    override_dir: Option<OsString>,
    home_dir: Option<OsString>,
    xdg_cache_dir: Option<OsString>,
) -> Result<PathBuf> {
    if let Some(path) = non_empty_path(override_dir) {
        return Ok(path);
    }

    #[cfg(target_os = "macos")]
    {
        let _ = xdg_cache_dir;
        let home = non_empty_path(home_dir)
            .context("HOME is not set; set LOCALTON_CACHE_DIR for the Localton cache")?;
        Ok(home.join("Library/Caches/localton"))
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(path) = non_empty_path(xdg_cache_dir) {
            return Ok(path.join("localton"));
        }
        let home = non_empty_path(home_dir)
            .context("HOME is not set; set LOCALTON_CACHE_DIR for the Localton cache")?;
        Ok(home.join(".cache/localton"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (home_dir, xdg_cache_dir);
        bail!("set LOCALTON_CACHE_DIR for the Localton cache")
    }
}

fn non_empty_path(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_cache_directory_has_highest_priority() {
        let root = root_from(
            Some(OsString::from("/custom/localton-cache")),
            Some(OsString::from("/users/test")),
            Some(OsString::from("/xdg-cache")),
        )
        .unwrap();

        assert_eq!(root, PathBuf::from("/custom/localton-cache"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_cache_uses_library_caches() {
        let root = root_from(None, Some(OsString::from("/users/test")), None).unwrap();

        assert_eq!(root, PathBuf::from("/users/test/Library/Caches/localton"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_cache_prefers_xdg_cache_home() {
        let root = root_from(
            None,
            Some(OsString::from("/users/test")),
            Some(OsString::from("/xdg-cache")),
        )
        .unwrap();

        assert_eq!(root, PathBuf::from("/xdg-cache/localton"));
    }
}
