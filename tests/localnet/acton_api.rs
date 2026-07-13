use crate::common::assertion;
use crate::support::localnet::pretty_json_for_snapshot;
use crate::support::project::ProjectBuilder;
use serde_json::{Value, json};
use ton_api::toncenter::v2::responses::{TonlibErrorResponse, TonlibResponse};
use ton_localnet::server::models::{
    BuildSourceTraceRequest, SourceTraceBundleRequest, SourceTraceCompilerRequest,
    SourceTraceFileRequest, SourceTraceResponse,
};

const MINIMAL_CONTRACT: &str = r"
fun onInternalMessage(_: InMessage) {}
fun onBouncedMessage(_: InMessageBounced) {}
";

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
