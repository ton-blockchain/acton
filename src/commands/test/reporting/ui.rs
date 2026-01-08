use crate::commands::test::reporting::{TestReport, TestReporter};
use axum::{
    Router,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
};
#[cfg(not(debug_assertions))]
use include_dir::{Dir, include_dir};
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
#[cfg(debug_assertions)]
use tower_http::services::ServeDir;

// Static directory containing UI assets, embedded into the binary during release builds.
#[cfg(not(debug_assertions))]
static UI_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/crates/acton-test-ui/dist");

#[cfg(target_os = "macos")]
static OPEN_CHROME_SCRIPT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/open_chrome.applescript"
));

pub struct UiServerState {
    pub reports: Arc<Vec<TestReport>>,
    pub trace_dir: Option<String>,
}

pub struct UiReporter {
    reports: Arc<Mutex<Vec<TestReport>>>,
}

impl UiReporter {
    pub fn new() -> Self {
        Self {
            reports: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_reports_arc(&self) -> Arc<Mutex<Vec<TestReport>>> {
        Arc::clone(&self.reports)
    }
}

impl TestReporter for UiReporter {
    fn on_test_finished(&mut self, test: &TestReport) -> anyhow::Result<()> {
        self.reports
            .lock()
            .expect("cannot lock mutex")
            .push(test.clone());
        Ok(())
    }
}

pub async fn start_ui_server(
    reports: Vec<TestReport>,
    trace_dir: Option<String>,
    port: u16,
) -> anyhow::Result<()> {
    let state = Arc::new(UiServerState {
        reports: Arc::new(reports),
        trace_dir,
    });

    let app = Router::new()
        .route("/api/reports", get(handle_api_reports))
        .route("/api/trace/{name}", get(handle_api_trace))
        .route("/api/contract/{name}", get(handle_api_contract));

    // In debug mode, serve UI assets directly from the filesystem for faster development.
    #[cfg(debug_assertions)]
    let app = {
        let dist_path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/crates/acton-test-ui/dist"
        ));
        app.fallback_service(
            ServeDir::new(&dist_path).fallback(ServeDir::new(dist_path.join("index.html"))),
        )
    };

    // In release mode, serve UI assets embedded within the binary.
    #[cfg(not(debug_assertions))]
    let app = app.fallback(handle_embedded_ui);

    let app = app.layer(CorsLayer::permissive()).with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    let url = format!("http://127.0.0.1:{}", port);
    println!("     {} UI server at {}", "Starting".green().bold(), url);

    open_browser(&url);

    axum::serve(listener, app).await?;
    Ok(())
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    {
        let chromium_browsers = [
            "Google Chrome",
            "Arc",
            "Brave Browser",
            "Microsoft Edge",
            "Vivaldi",
        ];

        for browser in chromium_browsers {
            if is_process_running(browser) {
                // Execute embedded AppleScript with arguments
                let child = std::process::Command::new("osascript")
                    .arg("-")
                    .arg(url)
                    .arg(browser)
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .ok();

                if let Some(mut child) = child {
                    use std::io::Write;
                    if let Some(mut stdin) = child.stdin.take() {
                        let _ = stdin.write_all(OPEN_CHROME_SCRIPT.as_bytes());
                    }
                    let status = child.wait().ok();
                    if status.map(|s| s.success()).unwrap_or(false) {
                        return;
                    }
                }
            }
        }
    }

    if let Err(e) = opener::open(url) {
        eprintln!("Warning: Failed to open browser: {}", e);
    }
}

#[cfg(target_os = "macos")]
fn is_process_running(process_name: &str) -> bool {
    let output = std::process::Command::new("ps").arg("-cax").output().ok();

    if let Some(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // We look for the exact process name in the list
        stdout.lines().any(|line| line.contains(process_name))
    } else {
        false
    }
}

/// Handles requests for UI assets when they are embedded in the binary (release mode).
#[cfg(not(debug_assertions))]
async fn handle_embedded_ui(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    // default to index.html for root requests
    let path = if path.is_empty() { "index.html" } else { path };

    if let Some(file) = UI_DIR.get_file(path) {
        // Map common file extensions to their respective MIME types.
        let content_type = match path.split('.').last() {
            Some("html") => "text/html",
            Some("js") => "application/javascript",
            Some("css") => "text/css",
            Some("svg") => "image/svg+xml",
            Some("png") => "image/png",
            Some("json") => "application/json",
            _ => "application/octet-stream",
        };
        return (([("content-type", content_type)]), file.contents()).into_response();
    }

    // fallback to index.html for SPA routing.
    // this allows browser refreshes on sub-routes to work correctly
    if let Some(index) = UI_DIR.get_file("index.html") {
        return (([("content-type", "text/html")]), index.contents()).into_response();
    }

    StatusCode::NOT_FOUND.into_response()
}

async fn handle_api_reports(State(state): State<Arc<UiServerState>>) -> impl IntoResponse {
    Json(state.reports.as_ref().clone())
}

async fn handle_api_trace(
    AxumPath(name): AxumPath<String>,
    State(state): State<Arc<UiServerState>>,
) -> impl IntoResponse {
    let Some(trace_dir) = &state.trace_dir else {
        return (StatusCode::NOT_FOUND, "Traces not enabled").into_response();
    };

    let trace_path = PathBuf::from(trace_dir).join(name);
    match tokio::fs::read_to_string(trace_path).await {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(json) => Json(json).into_response(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Invalid trace JSON").into_response(),
        },
        Err(_) => (StatusCode::NOT_FOUND, "Trace not found").into_response(),
    }
}

async fn handle_api_contract(
    AxumPath(name): AxumPath<String>,
    State(state): State<Arc<UiServerState>>,
) -> impl IntoResponse {
    let Some(trace_dir) = &state.trace_dir else {
        return (StatusCode::NOT_FOUND, "Traces not enabled").into_response();
    };

    let contract_path = PathBuf::from(trace_dir)
        .join("contracts")
        .join(format!("{}.json", name));

    match tokio::fs::read_to_string(contract_path).await {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(json) => Json(json).into_response(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Invalid contract JSON").into_response(),
        },
        Err(_) => (StatusCode::NOT_FOUND, "Contract not found").into_response(),
    }
}
