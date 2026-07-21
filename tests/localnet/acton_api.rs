use crate::common::assertion;
use crate::support::localnet::pretty_json_for_snapshot;
use crate::support::project::ProjectBuilder;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use ton_api::toncenter::v2::responses::{TonlibErrorResponse, TonlibResponse};
use ton_api::toncenter::v3::responses::RequestError;
use ton_localnet::server::models::{
    BuildSourceTraceRequest, SourceTraceBundleRequest, SourceTraceCompilerRequest,
    SourceTraceFileRequest, SourceTraceResponse,
};

const MINIMAL_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

#[test]
fn unknown_api_and_control_routes_return_authenticated_json_errors() {
    let project = ProjectBuilder::new("localnet-unknown-routes").build();
    let node = project.localnet().require_auth().start();

    let client = Client::new();
    let unauthorized_api = client
        .get(format!("{}/api/v2/doesNotExist", node.base_url()))
        .send()
        .expect("unauthorized unknown API request must be sent");
    let unauthorized_api_status = unauthorized_api.status().as_u16();
    let unauthorized_api: TonlibErrorResponse = unauthorized_api
        .json()
        .expect("unauthorized unknown API response must be typed JSON");
    let unauthorized_control = client
        .get(format!("{}/acton_doesNotExist", node.base_url()))
        .send()
        .expect("unauthorized unknown control request must be sent");
    let unauthorized_control_status = unauthorized_control.status().as_u16();
    let unauthorized_control: TonlibErrorResponse = unauthorized_control
        .json()
        .expect("unauthorized unknown control response must be typed JSON");

    let (v2_status, v2): (u16, TonlibErrorResponse) =
        node.get_json_with_status_as("/api/v2/doesNotExist");
    let (v3_status, v3): (u16, RequestError) = node.get_json_with_status_as("/api/v3/doesNotExist");
    let (emulate_status, emulate): (u16, RequestError) =
        node.post_json_with_status_as("/api/emulate/v1/doesNotExist", &json!({}));
    let (streaming_status, streaming): (u16, RequestError) =
        node.get_json_with_status_as("/api/streaming/v2/doesNotExist");
    let (control_status, control): (u16, TonlibErrorResponse) =
        node.get_json_with_status_as("/acton_doesNotExist");

    let ui = client
        .get(format!("{}/unknown/ui/route", node.base_url()))
        .send()
        .expect("unknown UI route request must be sent");
    let ui_status = ui.status().as_u16();
    let ui_content_type = ui
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let ui_body = ui.text().expect("unknown UI route body must be readable");

    let summary = json!({
        "unauthorized": {
            "api": {
                "status": unauthorized_api_status,
                "code": unauthorized_api.code,
                "error": unauthorized_api.error,
            },
            "control": {
                "status": unauthorized_control_status,
                "code": unauthorized_control.code,
                "error": unauthorized_control.error,
            },
        },
        "not_found": {
            "v2": { "status": v2_status, "code": v2.code, "error": v2.error },
            "v3": { "status": v3_status, "code": v3.code, "error": v3.error },
            "emulate": {
                "status": emulate_status,
                "code": emulate.code,
                "error": emulate.error,
            },
            "streaming": {
                "status": streaming_status,
                "code": streaming.code,
                "error": streaming.error,
            },
            "control": {
                "status": control_status,
                "code": control.code,
                "error": control.error,
            },
        },
        "ui": {
            "status": ui_status,
            "content_type": ui_content_type,
            "html_shell": ui_body.contains("<div id=\"root\"></div>"),
        },
    });
    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/unknown_routes.json"),
    );

    node.stop();
}

#[test]
fn build_source_trace_deserializes_canonical_response() {
    let project = ProjectBuilder::new("localnet-build-source-trace-response").build();
    let node = project.localnet().start();
    let request = BuildSourceTraceRequest {
        vm_logs: String::new(),
        code_hash: "e67eec3bd481c7910c87a061e60ca509e82edd687a0e1c8bf1b437e6de3e6973".to_owned(),
        source_bundle: SourceTraceBundleRequest {
            source_bundle_hash: "typed-response".to_owned(),
            entrypoint: "main.tolk".to_owned(),
            compiler: SourceTraceCompilerRequest {
                language: "tolk".to_owned(),
                version: "1.4.0".to_owned(),
                params: json!({}),
            },
            files: vec![SourceTraceFileRequest {
                path: "main.tolk".to_owned(),
                content: MINIMAL_CONTRACT.to_owned(),
            }],
        },
        context: None,
    };

    let response: TonlibResponse<SourceTraceResponse> =
        node.post_json_as("/acton_buildSourceTrace", &request);
    let summary = json!({
        "ok": response.ok,
        "code_hash": response.result.code_hash,
        "files": response
            .result
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        "step_count": response.result.steps.len(),
        "truncated": response.result.truncated,
        "has_extra": !response.extra.is_empty(),
    });

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/acton_build_source_trace_response.json"),
    );

    node.stop();
}

#[test]
fn build_source_trace_validates_bundle_and_paths() {
    let project = ProjectBuilder::new("localnet-build-source-trace-errors").build();
    let node = project.localnet().start();
    let base_request = BuildSourceTraceRequest {
        vm_logs: String::new(),
        code_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
        source_bundle: SourceTraceBundleRequest {
            source_bundle_hash: "validation".to_owned(),
            entrypoint: "main.tolk".to_owned(),
            compiler: SourceTraceCompilerRequest {
                language: "tolk".to_owned(),
                version: "1.4.0".to_owned(),
                params: json!({}),
            },
            files: vec![SourceTraceFileRequest {
                path: "main.tolk".to_owned(),
                content: MINIMAL_CONTRACT.to_owned(),
            }],
        },
        context: None,
    };

    let mut wrong_language = base_request.clone();
    wrong_language.source_bundle.compiler.language = "func".to_owned();
    let mut old_compiler = base_request.clone();
    old_compiler.source_bundle.compiler.version = "1.3.9".to_owned();
    let mut no_files = base_request.clone();
    no_files.source_bundle.files.clear();
    let mut no_entrypoint = base_request.clone();
    no_entrypoint.source_bundle.entrypoint = " ".to_owned();
    let mut unsafe_path = base_request;
    unsafe_path.source_bundle.entrypoint = "../main.tolk".to_owned();

    let mut summary = Vec::new();
    for (case, request) in [
        ("wrong language", wrong_language),
        ("old compiler", old_compiler),
        ("no files", no_files),
        ("no entrypoint", no_entrypoint),
        ("unsafe path", unsafe_path),
    ] {
        let (status, error): (u16, TonlibErrorResponse) =
            node.post_json_with_status_as("/acton_buildSourceTrace", &request);
        summary.push(json!({
            "case": case,
            "status": status,
            "code": error.code,
            "error": error.error,
        }));
    }

    assertion().eq(
        pretty_json_for_snapshot(&Value::Array(summary), project.path()),
        snapbox::file!("snapshots/acton_build_source_trace_errors.json"),
    );

    node.stop();
}

#[test]
fn failed_forced_snapshot_import_preserves_existing_recovery_point() {
    let project = ProjectBuilder::new("localnet-force-import-atomicity").build();
    let node = project.localnet().args(["--no-mining"]).start();
    let created = node.post_json("/acton_snapshot", &json!({ "name": "stable" }));
    let missing_path = project.path().join("missing-snapshot.json");

    let (status, error): (u16, TonlibErrorResponse) = node.post_json_with_status_as(
        "/acton_importSnapshot",
        &json!({
            "name": "stable",
            "path": missing_path.display().to_string(),
            "force": true,
        }),
    );
    let listed = node.post_json("/acton_listSnapshots", &json!({}));
    let reverted = node.post_json("/acton_revert", &json!({ "name": "stable" }));

    let summary = json!({
        "created": {
            "ok": created["ok"],
            "result": created["result"],
        },
        "failed_import": {
            "status": status,
            "code": error.code,
            "reported_error": !error.error.is_empty(),
        },
        "list_after_failure": {
            "ok": listed["ok"],
            "result": listed["result"],
        },
        "reverted_original": {
            "ok": reverted["ok"],
            "result": reverted["result"],
        },
    });
    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/acton_force_import_is_atomic.json"),
    );

    node.stop();
}
