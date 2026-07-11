use crate::paths;
use acton_config::config::project_root as configured_project_root;
use std::path::PathBuf;
use ton_language_server_native::{LogLevel, NativeLoggingConfig, ServerConfig};

pub async fn ls_cmd(
    port: Option<u16>,
    stdio: bool,
    log_file: Option<String>,
    no_log: bool,
    log_level: String,
    tolk_stdlib_path: Option<PathBuf>,
    enable_profiling: bool,
) -> anyhow::Result<()> {
    let log_level = log_level.parse::<LogLevel>()?;
    let project_root = configured_project_root().to_path_buf();
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
        tolk_stdlib_path,
        logging,
        enable_profiling,
    };

    match (port, stdio) {
        (Some(port), _) => ton_language_server_native::serve_tcp(config, port).await,
        (None, true) | (None, false) => ton_language_server_native::serve_stdio(config).await,
    }
}
