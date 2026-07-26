use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
#[cfg(not(debug_assertions))]
use axum::http::Uri;
use axum::response::Json;
#[cfg(not(debug_assertions))]
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get};
#[cfg(not(debug_assertions))]
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
#[cfg(debug_assertions)]
use tower_http::services::{ServeDir, ServeFile};

pub const DEFAULT_STUDIO_PORT: u16 = 3015;
pub const STUDIO_API_VERSION: u32 = 1;
pub const STUDIO_HEALTH_PATH: &str = "/api/v1/health";
pub const STUDIO_INFO_PATH: &str = "/api/v1/info";

#[cfg(not(debug_assertions))]
static UI_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../packages/studio-ui/dist");

#[derive(Clone, Debug)]
pub struct StudioWorkspace {
    name: String,
    root: PathBuf,
}

impl StudioWorkspace {
    pub fn new(name: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            root: root.into(),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
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

#[derive(Clone, Debug)]
pub struct StudioServer {
    config: StudioServerConfig,
}

impl StudioServer {
    #[must_use]
    pub const fn new(config: StudioServerConfig) -> Self {
        Self { config }
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
                    }),
            },
        };
        let api = Router::new()
            .route("/health", get(health))
            .route("/info", get(info))
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
        axum::serve(listener, self.router())
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(|source| StudioServerError::Serve { source })
    }
}

#[derive(Clone, Debug)]
struct StudioState {
    info: StudioInfo,
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
}

#[derive(Debug, thiserror::Error)]
pub enum StudioServerError {
    #[error("Studio server stopped with an error")]
    Serve { source: io::Error },
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn info(State(state): State<StudioState>) -> Json<StudioInfo> {
    Json(state.info)
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
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
