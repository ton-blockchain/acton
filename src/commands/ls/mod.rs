use crate::{paths, stdlib};
use acton_config::config::{ActonConfig, project_root as configured_project_root};
use dashmap::DashMap;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tolk_resolver::file_db::FileDb;
use ton_ls::{Backend, SelfContainedLanguageRegistry};
use tower_lsp::{LspService, Server};

pub async fn ls_cmd(
    port: Option<u16>,
    stdio: bool,
    log_file: Option<String>,
    no_log: bool,
) -> anyhow::Result<()> {
    if !no_log {
        setup_ls_logging(log_file)?;
    }

    let project_root = dunce::canonicalize(configured_project_root())
        .unwrap_or_else(|_| configured_project_root().to_path_buf());
    if port.is_none() {
        stdlib::ensure_latest_quiet(&project_root)?;
    } else {
        stdlib::ensure_latest(&project_root)?;
    }

    let stdlib_path = dunce::canonicalize(project_root.join(".acton/tolk-stdlib"))?;
    let acton_stdlib_path = dunce::canonicalize(project_root.join(".acton"))
        .unwrap_or_else(|_| project_root.join(".acton"));
    let common_tolk = stdlib_path.join("common.tolk");

    let file_db = FileDb::new(stdlib_path, Some(acton_stdlib_path));
    if common_tolk.exists() {
        let _ = file_db.process(&common_tolk);
    }
    let mappings = match ActonConfig::load() {
        Ok(config) => config.mappings(),
        Err(e) => {
            eprintln!("  ⚠ Failed to load Acton.toml: {e:#}");
            None
        }
    };

    if port.is_none() && !stdio {
        // default to stdio if no port is provided and stdio is not explicitly set
        return ls_cmd_internal(port, true, file_db, project_root, mappings).await;
    }

    ls_cmd_internal(port, stdio, file_db, project_root, mappings).await
}

async fn ls_cmd_internal(
    port: Option<u16>,
    stdio: bool,
    file_db: FileDb,
    project_root: PathBuf,
    mappings: Option<BTreeMap<String, String>>,
) -> anyhow::Result<()> {
    let (service, socket) = LspService::new(|client| {
        #[cfg(feature = "profiling")]
        let profiling = Arc::new(ton_ls::ProfilingContext::new());

        #[cfg(feature = "profiling")]
        {
            let profiling = profiling.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
                loop {
                    interval.tick().await;
                    profiling.log_stats();
                }
            });
        }

        Backend {
            client,
            file_db: Arc::new(file_db),
            project_root: project_root.clone(),
            mappings: mappings.clone(),
            documents: DashMap::new(),
            analysis: DashMap::new(),
            file_urls: DashMap::new(),
            registry: SelfContainedLanguageRegistry::new(),
            #[cfg(feature = "profiling")]
            profiling,
        }
    });

    if let Some(port) = port {
        let listener = TcpListener::bind(format!("127.0.0.1:{port}")).await?;
        println!("LSP server listening on port {port}");
        let (stream, _) = listener.accept().await?;
        let (reader, writer) = tokio::io::split(stream);
        Server::new(reader, writer, socket).serve(service).await;
    } else if stdio {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        Server::new(stdin, stdout, socket).serve(service).await;
    } else {
        anyhow::bail!("Either --port or --stdio must be specified");
    }

    Ok(())
}

fn setup_ls_logging(log_file: Option<String>) -> anyhow::Result<()> {
    let log_path = log_file.unwrap_or_else(|| {
        paths::language_server_log_path(configured_project_root())
            .to_string_lossy()
            .to_string()
    });

    if let Some(parent) = std::path::Path::new(&log_path).parent() {
        std::fs::create_dir_all(parent)?;
    }

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ));
        })
        .level(log::LevelFilter::Debug)
        .chain(fern::log_file(log_path)?)
        .apply()?;
    Ok(())
}
