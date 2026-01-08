use crate::commands::test::reporting::{TestReport, TestReporter};
use axum::{
    Router,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
};
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

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
) -> anyhow::Result<()> {
    let state = Arc::new(UiServerState {
        reports: Arc::new(reports),
        trace_dir,
    });

    // Path to the frontend dist directory
    let dist_path = PathBuf::from("crates/acton-test-ui/dist");

    let app = Router::new()
        .route("/api/reports", get(handle_api_reports))
        .route("/api/trace/{name}", get(handle_api_trace))
        .route("/api/contract/{name}", get(handle_api_contract))
        .fallback_service(
            ServeDir::new(dist_path)
                .fallback(ServeDir::new("crates/acton-test-ui/dist/index.html")),
        )
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    let url = "http://127.0.0.1:3000";
    println!("     {} UI server at {}", "Starting".green().bold(), url);

    // Open browser
    if let Err(e) = opener::open(url) {
        eprintln!("Warning: Failed to open browser: {}", e);
    }

    axum::serve(listener, app).await?;
    Ok(())
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
