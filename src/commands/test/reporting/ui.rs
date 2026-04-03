use crate::commands::common::error_fmt;
use crate::commands::test::reporting::{FuzzExecutionContext, TestReport, TestReporter};
use acton_config::color::OwoColorize;
use anyhow::Context;
use axum::{
    Router,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
};
#[cfg(not(debug_assertions))]
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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

pub(crate) struct UiServerState {
    pub reports: Arc<Vec<TestReport>>,
    pub trace_dir: Option<PathBuf>,
    pub project_root: String,
    pub project_root_path: PathBuf,
    pub coverage_lcov: Option<Arc<str>>,
}

pub(crate) struct UiReporter {
    reports: Arc<Mutex<Vec<TestReport>>>,
}

#[derive(Serialize)]
struct UiExecutionSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fuzz: Option<FuzzExecutionContext>,
}

#[derive(Serialize)]
struct UiTestReport {
    name: Arc<str>,
    suite_name: Arc<str>,
    file_path: PathBuf,
    row: usize,
    column: usize,
    duration: std::time::Duration,
    status: crate::commands::test::reporting::TestStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    detailed_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    failed_transactions: Option<Vec<crate::commands::test::trace::TransactionInfo>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    failed_transaction_context: Option<crate::commands::test::reporting::FailedTransactionContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    details: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    location: Option<ton_source_map::SourceLocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    execution: Option<UiExecutionSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    trace_path: Option<String>,
}

impl From<&TestReport> for UiTestReport {
    fn from(test: &TestReport) -> Self {
        Self {
            name: test.name.clone(),
            suite_name: test.suite_name.clone(),
            file_path: test.file_path.clone(),
            row: test.row,
            column: test.column,
            duration: test.duration,
            status: test.status.clone(),
            message: test.message.clone(),
            detailed_message: test.detailed_message.clone(),
            failed_transactions: test.failed_transactions.clone(),
            failed_transaction_context: test.failed_transaction_context.clone(),
            details: test.details.clone(),
            location: test.location.clone(),
            execution: test.execution.as_ref().and_then(|execution| {
                execution
                    .fuzz
                    .clone()
                    .map(|fuzz| UiExecutionSummary { fuzz: Some(fuzz) })
            }),
            trace_path: test.trace_path.clone(),
        }
    }
}

impl UiReporter {
    pub(crate) fn new() -> Self {
        Self {
            reports: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(crate) fn get_reports_arc(&self) -> Arc<Mutex<Vec<TestReport>>> {
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

pub(crate) fn reserve_ui_listener(port: u16) -> anyhow::Result<std::net::TcpListener> {
    let address = format!("127.0.0.1:{port}");
    std::net::TcpListener::bind(&address)
        .with_context(|| error_fmt::port_bind_failure("UI server", &address, "--ui-port"))
}

pub(crate) async fn start_ui_server(
    reports: Vec<TestReport>,
    trace_dir: Option<String>,
    project_root: String,
    coverage_lcov: Option<String>,
    listener: std::net::TcpListener,
) -> anyhow::Result<()> {
    let project_root_path =
        dunce::canonicalize(&project_root).unwrap_or_else(|_| PathBuf::from(&project_root));
    let trace_dir = trace_dir
        .map(PathBuf::from)
        .map(|path| dunce::canonicalize(&path).unwrap_or(path));
    let state = Arc::new(UiServerState {
        reports: Arc::new(reports),
        trace_dir,
        project_root,
        project_root_path,
        coverage_lcov: coverage_lcov.map(Arc::<str>::from),
    });

    let app = build_ui_api_router(state);

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

    let address = listener
        .local_addr()
        .context("Failed to inspect reserved UI server address")?;
    listener
        .set_nonblocking(true)
        .with_context(|| format!("Failed to configure UI server socket on {address}"))?;
    let listener = tokio::net::TcpListener::from_std(listener)
        .with_context(|| format!("Failed to activate UI server on {address}"))?;
    let url = format!("http://{address}");
    println!("     {} UI server at {}", "Starting".green().bold(), url);

    open_browser(&url);

    axum::serve(listener, app).await?;
    Ok(())
}

fn build_ui_api_router(state: Arc<UiServerState>) -> Router {
    Router::new()
        .route("/api/reports", get(handle_api_reports))
        .route("/api/test-logs", get(handle_api_test_logs))
        .route("/api/trace/{name}", get(handle_api_trace))
        .route("/api/contract/{name}", get(handle_api_contract))
        .route("/api/file", get(handle_api_file))
        .route("/api/coverage.lcov", get(handle_api_coverage_lcov))
        .route("/api/config", get(handle_api_config))
        .with_state(state)
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
                    if status.is_some_and(|s| s.success()) {
                        return;
                    }
                }
            }
        }
    }

    if let Err(e) = opener::open(url) {
        eprintln!("Warning: Failed to open browser: {e}");
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
    let reports = state
        .reports
        .iter()
        .map(UiTestReport::from)
        .collect::<Vec<_>>();
    Json(reports)
}

#[derive(Deserialize)]
struct TestLogsQuery {
    file_path: String,
    name: String,
    row: usize,
    column: usize,
}

#[derive(Default, Serialize)]
struct TestExecutionLogsResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stdout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stderr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vm_log_diff: Option<String>,
}

async fn handle_api_test_logs(
    Query(query): Query<TestLogsQuery>,
    State(state): State<Arc<UiServerState>>,
) -> impl IntoResponse {
    let file_path = Path::new(&query.file_path);
    let Some(test) = state.reports.iter().find(|report| {
        report.file_path == file_path
            && report.name.as_ref() == query.name
            && report.row == query.row
            && report.column == query.column
    }) else {
        return (StatusCode::NOT_FOUND, "Test not found").into_response();
    };

    let response =
        test.execution
            .as_ref()
            .map_or_else(TestExecutionLogsResponse::default, |execution| {
                TestExecutionLogsResponse {
                    stdout: non_empty_text(&execution.stdout),
                    stderr: non_empty_text(&execution.stderr),
                    vm_log_diff: execution.vm_log_diff.clone(),
                }
            });

    Json(response).into_response()
}

#[derive(Deserialize)]
struct FileQuery {
    path: String,
}

async fn handle_api_file(
    Query(query): Query<FileQuery>,
    State(state): State<Arc<UiServerState>>,
) -> impl IntoResponse {
    let requested_path = PathBuf::from(&query.path);
    let Some(file_path) = resolve_path_within_root(&state.project_root_path, &requested_path)
    else {
        return (StatusCode::FORBIDDEN, "Access denied").into_response();
    };

    match tokio::fs::read_to_string(file_path).await {
        Ok(content) => content.into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}

#[derive(Serialize)]
struct ConfigResponse {
    project_root: String,
}

async fn handle_api_config(State(state): State<Arc<UiServerState>>) -> impl IntoResponse {
    Json(ConfigResponse {
        project_root: state.project_root.clone(),
    })
}

async fn handle_api_coverage_lcov(State(state): State<Arc<UiServerState>>) -> impl IntoResponse {
    let Some(coverage_lcov) = &state.coverage_lcov else {
        return (StatusCode::NOT_FOUND, "Coverage not enabled").into_response();
    };

    (
        [("content-type", "text/plain; charset=utf-8")],
        coverage_lcov.to_string(),
    )
        .into_response()
}

async fn handle_api_trace(
    AxumPath(name): AxumPath<String>,
    State(state): State<Arc<UiServerState>>,
) -> impl IntoResponse {
    let Some(trace_dir) = &state.trace_dir else {
        return (StatusCode::NOT_FOUND, "Traces not enabled").into_response();
    };

    let Some(trace_path) = resolve_path_within_root(trace_dir, Path::new(&name)) else {
        return (StatusCode::FORBIDDEN, "Access denied").into_response();
    };

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

    let contracts_dir = trace_dir.join("contracts");
    let contract_name = format!("{name}.json");
    let Some(contract_path) = resolve_path_within_root(&contracts_dir, Path::new(&contract_name))
    else {
        return (StatusCode::FORBIDDEN, "Access denied").into_response();
    };

    match tokio::fs::read_to_string(contract_path).await {
        Ok(content) => match serde_json::from_str::<serde_json::Value>(&content) {
            Ok(json) => Json(json).into_response(),
            Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Invalid contract JSON").into_response(),
        },
        Err(_) => (StatusCode::NOT_FOUND, "Contract not found").into_response(),
    }
}

fn resolve_path_within_root(root: &Path, requested: &Path) -> Option<PathBuf> {
    let candidate = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        root.join(requested)
    };
    let candidate = dunce::canonicalize(candidate).ok()?;
    candidate.starts_with(root).then_some(candidate)
}

fn non_empty_text(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_owned())
}
