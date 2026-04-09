use std::path::{Path, PathBuf};

pub const DEFAULT_BUILD_CACHE_DIR: &str = "build/cache";
pub const DEFAULT_BUILD_TRACES_DIR: &str = "build/traces";
pub const DEFAULT_BUILD_MUTATION_SESSIONS_DIR: &str = "build/mutation-sessions";
pub const DEFAULT_BUILD_LOGS_DIR: &str = "build/logs";
pub const DEFAULT_LANGUAGE_SERVER_LOG_PATH: &str = "build/logs/tolk-language-server.log";

pub fn build_dir(project_root: &Path) -> PathBuf {
    project_root.join("build")
}

pub fn build_cache_dir(project_root: &Path) -> PathBuf {
    build_dir(project_root).join("cache")
}

pub fn build_traces_dir(project_root: &Path) -> PathBuf {
    build_dir(project_root).join("traces")
}

pub fn build_mutation_sessions_dir(project_root: &Path) -> PathBuf {
    build_dir(project_root).join("mutation-sessions")
}

pub fn build_logs_dir(project_root: &Path) -> PathBuf {
    build_dir(project_root).join("logs")
}

pub fn language_server_log_path(project_root: &Path) -> PathBuf {
    build_logs_dir(project_root).join("tolk-language-server.log")
}
