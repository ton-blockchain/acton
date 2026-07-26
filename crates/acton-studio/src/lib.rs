use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
#[cfg(not(debug_assertions))]
use axum::http::Uri;
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{any, get, post};
#[cfg(not(debug_assertions))]
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
#[cfg(debug_assertions)]
use tower_http::services::{ServeDir, ServeFile};

mod environment;
mod local_process;

pub use environment::{
    CreateEnvironmentRequest, EnvironmentConfig, EnvironmentRuntime, EnvironmentRuntimeError,
    EnvironmentRuntimeFuture, EnvironmentStatus, StudioEnvironment,
};
pub use local_process::LocalProcessEnvironmentRuntime;

pub const DEFAULT_STUDIO_PORT: u16 = 3015;
pub const STUDIO_API_VERSION: u32 = 1;
pub const STUDIO_ENVIRONMENTS_PATH: &str = "/api/v1/environments";
pub const STUDIO_HEALTH_PATH: &str = "/api/v1/health";
pub const STUDIO_INFO_PATH: &str = "/api/v1/info";

#[cfg(not(debug_assertions))]
static UI_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../packages/studio-ui/dist");

#[derive(Clone, Debug)]
pub struct StudioWorkspace {
    name: String,
    root: PathBuf,
    wallet_names: Vec<String>,
}

impl StudioWorkspace {
    pub fn new(name: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            root: root.into(),
            wallet_names: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_wallet_names(mut self, wallet_names: Vec<String>) -> Self {
        self.wallet_names = wallet_names;
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    #[must_use]
    pub fn wallet_names(&self) -> &[String] {
        &self.wallet_names
    }
}

#[derive(Clone, Debug)]
pub struct StudioServerConfig {
    server_version: String,
    workspace: Option<StudioWorkspace>,
}

impl StudioServerConfig {
    pub fn new(server_version: impl Into<String>) -> Self {
        Self {
            server_version: server_version.into(),
            workspace: None,
        }
    }

    #[must_use]
    pub fn with_workspace(mut self, workspace: StudioWorkspace) -> Self {
        self.workspace = Some(workspace);
        self
    }
}

#[derive(Clone)]
pub struct StudioServer {
    config: StudioServerConfig,
    environment_runtime: Arc<dyn EnvironmentRuntime>,
}

impl StudioServer {
    #[must_use]
    pub fn new(config: StudioServerConfig) -> Self {
        Self {
            config,
            environment_runtime: Arc::new(environment::EmptyEnvironmentRuntime),
        }
    }

    #[must_use]
    pub fn with_environment_runtime<R>(mut self, environment_runtime: R) -> Self
    where
        R: EnvironmentRuntime + 'static,
    {
        self.environment_runtime = Arc::new(environment_runtime);
        self
    }

    #[must_use]
    pub const fn workspace(&self) -> Option<&StudioWorkspace> {
        self.config.workspace.as_ref()
    }

    pub fn router(&self) -> Router {
        let state = StudioState {
            info: StudioInfo {
                protocol_version: STUDIO_API_VERSION,
                server_version: self.config.server_version.clone(),
                workspace: self
                    .config
                    .workspace
                    .as_ref()
                    .map(|workspace| WorkspaceInfo {
                        name: workspace.name.clone(),
                        wallet_names: workspace.wallet_names.clone(),
                    }),
            },
            environment_runtime: Arc::clone(&self.environment_runtime),
        };
        let api = Router::new()
            .route("/health", get(health))
            .route("/info", get(info))
            .route(
                "/environments",
                get(list_environments).post(create_environment),
            )
            .route(
                "/environments/{environment_id}/stop",
                post(stop_environment),
            )
            .route(
                "/environments/{environment_id}/restart",
                post(restart_environment),
            )
            .fallback(api_not_found);
        let app = Router::new()
            .nest("/api/v1", api)
            .route("/api", any(api_not_found))
            .route("/api/{*path}", any(api_not_found))
            .with_state(state);

        #[cfg(debug_assertions)]
        {
            let dist_path = PathBuf::from(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../packages/studio-ui/dist"
            ));
            app.fallback_service(
                ServeDir::new(&dist_path).fallback(ServeFile::new(dist_path.join("index.html"))),
            )
        }

        #[cfg(not(debug_assertions))]
        {
            app.fallback(embedded_ui)
        }
    }

    pub async fn serve<F>(
        &self,
        listener: TcpListener,
        shutdown: F,
    ) -> Result<(), StudioServerError>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let serve_result = axum::serve(listener, self.router())
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|source| StudioServerError::Serve { source });
        let shutdown_result = self.environment_runtime.shutdown().await;

        serve_result?;
        shutdown_result.map_err(|source| StudioServerError::EnvironmentShutdown { source })
    }
}

#[derive(Clone)]
struct StudioState {
    info: StudioInfo,
    environment_runtime: Arc<dyn EnvironmentRuntime>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioInfo {
    pub protocol_version: u32,
    pub server_version: String,
    pub workspace: Option<WorkspaceInfo>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wallet_names: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum StudioServerError {
    #[error("Studio server stopped with an error")]
    Serve { source: io::Error },
    #[error("Studio environments failed to stop")]
    EnvironmentShutdown { source: EnvironmentRuntimeError },
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn info(State(state): State<StudioState>) -> Json<StudioInfo> {
    Json(state.info)
}

async fn list_environments(
    State(state): State<StudioState>,
) -> Result<Json<Vec<StudioEnvironment>>, StudioApiError> {
    state
        .environment_runtime
        .list()
        .await
        .map(Json)
        .map_err(StudioApiError)
}

async fn create_environment(
    State(state): State<StudioState>,
    Json(request): Json<CreateEnvironmentRequest>,
) -> Result<(StatusCode, Json<StudioEnvironment>), StudioApiError> {
    state
        .environment_runtime
        .create(request)
        .await
        .map(|environment| (StatusCode::CREATED, Json(environment)))
        .map_err(StudioApiError)
}

async fn stop_environment(
    State(state): State<StudioState>,
    AxumPath(environment_id): AxumPath<String>,
) -> Result<Json<StudioEnvironment>, StudioApiError> {
    state
        .environment_runtime
        .stop(&environment_id)
        .await
        .map(Json)
        .map_err(StudioApiError)
}

async fn restart_environment(
    State(state): State<StudioState>,
    AxumPath(environment_id): AxumPath<String>,
) -> Result<Json<StudioEnvironment>, StudioApiError> {
    state
        .environment_runtime
        .restart(&environment_id)
        .await
        .map(Json)
        .map_err(StudioApiError)
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

struct StudioApiError(EnvironmentRuntimeError);

#[derive(Serialize)]
struct StudioApiErrorBody {
    error: StudioApiErrorDetails,
}

#[derive(Serialize)]
struct StudioApiErrorDetails {
    code: &'static str,
    message: String,
}

impl IntoResponse for StudioApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            EnvironmentRuntimeError::InvalidRequest { .. } => StatusCode::BAD_REQUEST,
            EnvironmentRuntimeError::Conflict { .. } => StatusCode::CONFLICT,
            EnvironmentRuntimeError::NotFound { .. } => StatusCode::NOT_FOUND,
            EnvironmentRuntimeError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let code = match &self.0 {
            EnvironmentRuntimeError::InvalidRequest { code, .. }
            | EnvironmentRuntimeError::Conflict { code, .. }
            | EnvironmentRuntimeError::Internal { code, .. } => *code,
            EnvironmentRuntimeError::NotFound { .. } => "environment_not_found",
        };
        (
            status,
            Json(StudioApiErrorBody {
                error: StudioApiErrorDetails {
                    code,
                    message: self.0.to_string(),
                },
            }),
        )
            .into_response()
    }
}

#[cfg(not(debug_assertions))]
async fn embedded_ui(uri: Uri) -> Response {
    let requested_path = uri.path().trim_start_matches('/');
    let requested_path = if requested_path.is_empty() {
        "index.html"
    } else {
        requested_path
    };

    if let Some(file) = UI_DIR.get_file(requested_path) {
        return ui_file_response(requested_path, file.contents());
    }

    UI_DIR
        .get_file("index.html")
        .map(|index| ui_file_response("index.html", index.contents()))
        .unwrap_or_else(|| StatusCode::NOT_FOUND.into_response())
}

#[cfg(not(debug_assertions))]
fn ui_file_response(path: &str, contents: &'static [u8]) -> Response {
    let content_type = match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("ico") => "image/x-icon",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") | Some("map") => "application/json",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    };

    ([("content-type", content_type)], contents).into_response()
}
