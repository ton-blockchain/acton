use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tolk_ls::Backend;
use tolk_resolver::file_db::FileDb;
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

    let stdlib_path = PathBuf::from(".acton/tolk-stdlib")
        .canonicalize()
        .expect("Failed to canonicalize");
    let acton_stdlib_path = PathBuf::from(".acton/")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(".acton/"));
    let common_tolk = stdlib_path.join("common.tolk");

    let file_db = FileDb::new(stdlib_path, Some(acton_stdlib_path));
    if common_tolk.exists() {
        let _ = file_db.process(&common_tolk);
    }

    if port.is_none() && !stdio {
        // default to stdio if no port is provided and stdio is not explicitly set
        return ls_cmd_internal(port, true, file_db).await;
    }

    ls_cmd_internal(port, stdio, file_db).await
}

async fn ls_cmd_internal(port: Option<u16>, stdio: bool, file_db: FileDb) -> anyhow::Result<()> {
    let (service, socket) = LspService::new(|client| Backend {
        client,
        file_db: Arc::new(file_db),
        documents: DashMap::new(),
        analysis: DashMap::new(),
        line_offsets: DashMap::new(),
        file_urls: DashMap::new(),
    });

    if let Some(port) = port {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
        println!("LSP server listening on port {}", port);
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
    let log_path = log_file.unwrap_or_else(|| ".acton/tolk-language-server.log".to_string());

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
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(fern::log_file(log_path)?)
        .apply()?;
    Ok(())
}
