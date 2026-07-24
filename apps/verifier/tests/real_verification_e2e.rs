mod support;

use std::{collections::BTreeSet, path::Path};

use axum::http::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use support::{
    get, owned_file_part, owned_text_part, post_verify, real_compiler_app_state, response_json,
};

const TOLK_CODE_HASH: &str = "a873d8c2d163f7fa10bbe38769706f0554505e8ea2dcea3f115288db8becf2ab";
const SIMPLE_TOLK_CODE_HASH: &str =
    "63600fb71c1bfc85ed75dfbbd7b8e857ca98bc003fb2f758f07708fd1664edae";
const FUNC_CODE_HASH: &str = "6ef6e4084167bca1464f9d2ddc8448bbd66df303c4014af50aeb5a109fdfb8cc";
const TACT_CODE_HASH: &str = "f6b6d11538f0cb19c9f5b2812cb66d907b56c752c673d1bea205f07bce4c7f52";
const ALL_FUNC_VERSIONS: &[&str] = &[
    "0.2.0",
    "0.3.0",
    "0.4.0",
    "0.4.1",
    "0.4.2",
    "0.4.3",
    "0.4.4",
    "0.4.4-newops",
    "0.4.4-newops.1",
    "0.4.5",
    "0.4.6",
    "0.4.6-wasmfix.0",
];
const ALL_TOLK_VERSIONS: &[&str] = &[
    "0.6.0", "0.7.0", "0.8.0", "0.9.0", "0.10.0", "0.11.0", "0.12.0", "0.13.0", "0.99.0", "1.0.0",
    "1.1.0", "1.2.0", "1.3.0", "1.4.0", "1.4.1", "1.4.2",
];
const TACT_SOURCE_BASE64: &str = "Y29udHJhY3QgU21va2UgeyBpbml0KCkge30gcmVjZWl2ZSgpIHt9IH0=";
const TACT_VERSION_GROUPS: &[TactVersionGroup] = &[
    TactVersionGroup {
        versions: &[
            "1.6.5", "1.6.6", "1.6.7", "1.6.10", "1.6.11", "1.6.12", "1.6.13",
        ],
        code_boc: "te6ccgEBAQEAVwAAqv8AII5MMAHQctch0gDSAPpAIRA0UGZvBPhhAvhi7UTQ0gAwkW2RbeICkVvgcCHXSSDCH5UxAdMfMJEy4sAAAcEhsJkwyH8BygDJ7VTgMPLAguHyyAs=",
        code_hash: TACT_CODE_HASH,
    },
    TactVersionGroup {
        versions: &["1.6.3", "1.6.4"],
        code_boc: "te6ccgEBAgEAWwABFP8A9KQT9LzyyAsBAJjTAdBy1yHSANIA+kAhEDRQZm8E+GEC+GLtRNDSADCRbZFt4gKRW+BwIddJIMIflTEB0x8wkTLiwAABwSGwmTDIfwHKAMntVOAw8sCC",
        code_hash: "c9117b48e02d012aad1f5f6b573eb50f9e55d3d2513f595fb6d5d5d9a17dc3af",
    },
    TactVersionGroup {
        versions: &["1.6.2"],
        code_boc: "te6ccgEBAQEAXAAAtP8AII5MMAHQctch0gDSAPpAIRA0UGZvBPhhAvhi7UTQ0gAwkW2RbeICkVvgcCHXSSDCH5UxAdMfMJEy4sAAAcEhsJkwyH8BygDJ7VTgMPLAguFtgBP0vPLICw==",
        code_hash: "54b93759513af3ff37fb587899e70af5a10efdfa7a85e8ffc15d52b00fffda51",
    },
    TactVersionGroup {
        versions: &[
            "1.4.1", "1.4.2", "1.4.3", "1.4.4", "1.5.0", "1.5.1", "1.5.2", "1.5.3", "1.5.4",
        ],
        code_boc: "te6ccgEBBwEAqgABFP8A9KQT9LzyyAsBAgFiAgMCktAB0NMDAXGwowH6QAEg10mBAQu68uCIINcLCiCBBP+68tCJgwm68uCIVFBTA28E+GEC+GLbPFnbPPLggjDI+EMBzH8BygDJ7VQEBQARoYV92omhpAADATTtRNDUAfhj0gAwkW3g+CjXCwqDCbry4InbPAYAPAGSMH/gcCHXScIflTAg1wsf3sAAAddJwSGwkX/gcAACbQ==",
        code_hash: "437d33ce3e8b433319dcb7ea72e2b5cd9e8fde8c489ea24bf52aadd63805a172",
    },
    TactVersionGroup {
        versions: &["1.3.1", "1.4.0"],
        code_boc: "te6ccgECCwEAAT4AART/APSkE/S88sgLAQIBYgIDApLQAdDTAwFxsKMB+kABINdJgQELuvLgiCDXCwoggQT/uvLQiYMJuvLgiFRQUwNvBPhhAvhi2zxZ2zzy4IIwyPhDAcx/AcoAye1UBAUCAVgHCAE07UTQ1AH4Y9IAMJFt4Pgo1wsKgwm68uCJ2zwGADwBkjB/4HAh10nCH5UwINcLH97AAAHXScEhsJF/4HAAAm0Albu9GCcFzsPV0srnsehOw51kqFG2aCcJ3WNS0rZHyzItOvLf3xYjmCcCBVwBuAZ2OUzlg6rkclssOCcJ2XTlqzTstzOg6WbZRm6KSAIBSAkKABGwr7tRNDSAAGAAdbJu40NWlwZnM6Ly9RbVVncDdHSkVqRHlEQzNMbVJpa0xDS0E3Y2YxQ29kbmdTWm5UM0NyOWdHdlRmgg",
        code_hash: "d3da9a6f003f842be5f37e274a3f1bf57ff26a80d460cd3683fcf6fd6ff55945",
    },
    TactVersionGroup {
        versions: &[
            "1.1.0", "1.1.1", "1.1.2", "1.1.3", "1.1.4", "1.1.5", "1.2.0", "1.3.0",
        ],
        code_boc: "te6ccgECCwEAAT4AART/APSkE/S88sgLAQIBYgIDApLQAdDTAwFxsKMB+kABINdJgQELuvLgiCDXCwoggQT/uvLQiYMJuvLgiFRQUwNvBPhhAvhi2zxZ2zzy4IIwyPhDAcx/AcoAye1UBAUCAVgHCAE07UTQ1AH4Y9IAMJFt4Pgo1wsKgwm68uCJ2zwGADwBkjB/4HAh10nCH5UwINcLH97AAAHXScEhsJF/4HAAAm0Albu9GCcFzsPV0srnsehOw51kqFG2aCcJ3WNS0rZHyzItOvLf3xYjmCcCBVwBuAZ2OUzlg6rkclssOCcJ2XTlqzTstzOg6WbZRm6KSAIBSAkKABGwr7tRNDSAAGAAdbJu40NWlwZnM6Ly9RbWRwQ3FXakdiNzQ3VWtQU3M1bVFDcFM4RjMzaWFhVnJTU0JIdUZtNGFkS21agg",
        code_hash: "219f44acd833359da1fba55cf457f444a8cf1ef65649dcbc21a18ec61ccb45ff",
    },
    TactVersionGroup {
        versions: &["1.0.0"],
        code_boc: "te6ccgEBBgEA8wABFP8A9KQT9LzyyAsBAgFiAgMC2NAB0NMDAXGwwAGRf5Fw4gH6QAEg10mBAQu68uCIINcLCiCDCbohgQT/urHy4IiDCbry4IlUUFMDbwT4YQL4Yu1E0NQB+GPSADCRbY6N+CjXCwqDCbry4InbPOJZ2zwwMMj4QwHMfwHKAMntVAQFAJWhd6ME4LnYerpZXPY9CdhzrJUKNs0E4TusalpWyPlmRadeW/vixHME4ECrgDcAzscpnLB1XI5LZYcE4TsunLVmnZbmdB0s2yjN0UkAAm0APnAh10nCH5UwINcLH94Cklt/4AHAAAHXScEhsJF/4HA=",
        code_hash: "a4a22ce9f054f2c9f3cae625098e15c3a9b77eae765047914b837135ce5777b9",
    },
];

#[test]
fn compiler_version_matrix_covers_all_npm_aliases() {
    assert_eq!(npm_alias_versions("func"), version_set(ALL_FUNC_VERSIONS));
    assert_eq!(npm_alias_versions("tolk"), version_set(ALL_TOLK_VERSIONS));

    let tact_versions = TACT_VERSION_GROUPS
        .iter()
        .flat_map(|group| group.versions.iter().copied())
        .collect::<Vec<_>>();
    assert_eq!(npm_alias_versions("tact"), version_set(&tact_versions));
}

#[tokio::test]
async fn verify_tolk_with_real_compiler_and_stores_generated_abi() {
    let state = real_compiler_app_state(&[]);
    let response =
        verify_fixture(state.clone(), TOLK_CODE_HASH, fixture("valid-minimal.json")).await;

    assert_verified(response, "tolk", TOLK_CODE_HASH).await;

    let response = get(
        state,
        &format!("/api/v1/verification/source?code_hash={TOLK_CODE_HASH}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationSourceResponse>(response).await;
    assert!(body.verified);
    let bundle = body
        .bundle
        .expect("verified source should include a bundle");
    assert_eq!(bundle.compiler.language, "tolk");
    assert_eq!(bundle.compiler.version, "1.4.1");
    assert_eq!(bundle.entrypoint, "main.tolk");
    let files = &bundle.files;
    let abi = files
        .iter()
        .find(|file| file.path == "output/main.abi.json")
        .expect("expected stored Tolk bundle to include generated ABI JSON");
    let abi =
        serde_json::from_str::<Value>(&abi.content).expect("generated Tolk ABI should be JSON");
    assert_eq!(abi["abi_schema_version"], "1.0");
    assert_eq!(abi["compiler_name"], "tolk");
    assert_eq!(abi["compiler_version"], "1.4.1");
}

#[tokio::test]
async fn verify_tolk_import_mappings_with_real_compiler() {
    let state = real_compiler_app_state(&[]);
    let response =
        verify_fixture(state, TOLK_CODE_HASH, fixture("valid-import-mapping.json")).await;

    assert_verified(response, "tolk", TOLK_CODE_HASH).await;
}

#[tokio::test]
async fn verify_all_tolk_npm_versions_with_real_compiler() {
    for compiler_version in ALL_TOLK_VERSIONS {
        let state = real_compiler_app_state(&[]);
        let response = verify_fixture(
            state,
            SIMPLE_TOLK_CODE_HASH,
            simple_tolk_fixture(compiler_version),
        )
        .await;

        assert_verified(response, "tolk", SIMPLE_TOLK_CODE_HASH).await;
    }
}

#[tokio::test]
async fn verify_func_with_real_compiler() {
    let state = real_compiler_app_state(&[]);
    let response = verify_fixture(state, FUNC_CODE_HASH, fixture("valid-func.json")).await;

    assert_verified(response, "func", FUNC_CODE_HASH).await;
}

#[tokio::test]
async fn verify_all_func_npm_versions_with_real_compiler() {
    for compiler_version in ALL_FUNC_VERSIONS {
        let state = real_compiler_app_state(&[]);
        let response = verify_fixture(state, FUNC_CODE_HASH, func_fixture(compiler_version)).await;

        assert_verified(response, "func", FUNC_CODE_HASH).await;
    }
}

#[tokio::test]
async fn verify_tact_with_real_compiler_and_stores_generated_sources() {
    let state = real_compiler_app_state(&[]);
    let response = verify_fixture(state.clone(), TACT_CODE_HASH, fixture("valid-tact.json")).await;
    assert_verified(response, "tact", TACT_CODE_HASH).await;

    let response = get(
        state.clone(),
        &format!("/api/v1/verification/source?code_hash={TACT_CODE_HASH}"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response_json::<VerificationSourceResponse>(response).await;
    assert!(body.verified);
    let bundle = body
        .bundle
        .expect("verified source should include a bundle");
    assert_eq!(bundle.compiler.language, "tact");
    assert_eq!(bundle.compiler.version, "1.6.13");
    assert_eq!(bundle.entrypoint, "contract/contract.tact");
    let files = &bundle.files;
    assert!(files.iter().any(|file| file.path == "contract.pkg"));
    assert!(
        files.iter().any(|file| has_extension(&file.path, "abi")),
        "expected stored Tact bundle to include generated ABI"
    );
    assert!(
        files.iter().any(|file| has_extension(&file.path, "tact")),
        "expected stored Tact bundle to include generated source"
    );
    let types = files
        .iter()
        .find(|file| file.path == "output/Smoke.types.tolk")
        .expect("expected stored Tact bundle to include generated Tolk types");
    assert!(types.content.contains("contract Smoke"));
    assert!(types.content.contains("storage: SmokeData"));

    let abi = files
        .iter()
        .find(|file| file.path == "output/Smoke.abi.json")
        .expect("expected stored Tact bundle to include generated Tolk ABI JSON");
    let abi =
        serde_json::from_str::<Value>(&abi.content).expect("generated Tolk ABI should be JSON");
    assert_eq!(abi["abi_schema_version"], "1.0");
    assert_eq!(abi["contract_name"], "Smoke");
    assert_eq!(abi["compiler_name"], "tolk");
    assert_eq!(abi["compiler_version"], "1.4.2");

    let response = get(state, &format!("/api/v1/abi?code_hash={TACT_CODE_HASH}")).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json::<Value>(response).await;
    assert_eq!(body["items"][0]["code_hash"], TACT_CODE_HASH);
    assert_eq!(body["items"][0]["abi"]["contract_name"], "Smoke");
}

#[tokio::test]
async fn verify_all_tact_npm_versions_with_real_compiler() {
    for group in TACT_VERSION_GROUPS {
        for compiler_version in group.versions {
            let state = real_compiler_app_state(&[]);
            let response = verify_fixture(
                state,
                group.code_hash,
                tact_fixture(compiler_version, group.code_boc),
            )
            .await;

            assert_verified(response, "tact", group.code_hash).await;
        }
    }
}

async fn verify_fixture(
    state: verifier::state::AppState,
    code_hash: &str,
    fixture: WorkerFixture,
) -> axum::response::Response {
    let WorkerFixture {
        language,
        compiler_version,
        import_mappings,
        entrypoint,
        sources,
    } = fixture;
    assert!(
        sources.iter().any(|source| source.path == entrypoint),
        "fixture entrypoint {entrypoint} should be present in sources"
    );

    let compiler_version = compiler_version.as_str();
    let compile_params = import_mappings.map_or_else(
        || json!({"compiler_version": compiler_version}),
        |import_mappings| {
            json!({"compiler_version": compiler_version, "import_mappings": import_mappings})
        },
    );
    let source_metadata = sources
        .iter()
        .map(|source| WorkerSourceMetadata::from_source(source, &entrypoint))
        .collect::<Vec<_>>();

    let mut parts = vec![
        owned_text_part("code_hash", code_hash.to_owned()),
        owned_text_part("language", language),
        owned_text_part("compile_params", compile_params.to_string()),
        owned_text_part(
            "sources",
            serde_json::to_string(&source_metadata).expect("source metadata should serialize"),
        ),
    ];
    for source in sources {
        parts.push(owned_file_part(
            "files",
            source.path,
            "text/plain",
            source.content,
        ));
    }

    post_verify(state, parts).await
}

async fn assert_verified(response: axum::response::Response, _language: &str, code_hash: &str) {
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("verification response body should be readable");
    assert_eq!(
        status,
        StatusCode::OK,
        "verification response: {}",
        String::from_utf8_lossy(&body)
    );
    let body = serde_json::from_slice::<VerifyResponse>(&body)
        .expect("verification response should contain valid JSON");
    assert_eq!(body.code_hash, code_hash);
    assert_eq!(body.compiled_code_hash.as_deref(), Some(code_hash));
    assert_eq!(body.verification_result, "match");
    assert!(body.source_bundle_hash.is_some());
    assert!(body.storage_revision.is_some());
}

fn fixture(name: &str) -> WorkerFixture {
    let path = Path::new("compiler-worker").join("fixtures").join(name);
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|err| panic!("fixture is not valid JSON {}: {err}", path.display()))
}

fn npm_alias_versions(language: &str) -> BTreeSet<String> {
    let path = Path::new("compiler-worker").join("package.json");
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    let package = serde_json::from_slice::<Value>(&bytes)
        .unwrap_or_else(|err| panic!("package is not valid JSON {}: {err}", path.display()));
    let dependencies = package["dependencies"]
        .as_object()
        .expect("compiler-worker dependencies should be an object");
    let prefix = format!("{language}-");

    dependencies
        .keys()
        .filter_map(|name| name.strip_prefix(&prefix).map(str::to_owned))
        .collect()
}

fn version_set(versions: &[&str]) -> BTreeSet<String> {
    versions.iter().map(ToString::to_string).collect()
}

fn simple_tolk_fixture(compiler_version: &str) -> WorkerFixture {
    WorkerFixture {
        language: "tolk".to_owned(),
        compiler_version: compiler_version.to_owned(),
        import_mappings: None,
        entrypoint: "main.tolk".to_owned(),
        sources: vec![WorkerSource {
            path: "main.tolk".to_owned(),
            content: "fun main(): int {\n    return 0;\n}\n".to_owned(),
            is_entrypoint: true,
            include_in_command: None,
            is_stdlib: None,
            has_include_directives: None,
        }],
    }
}

fn func_fixture(compiler_version: &str) -> WorkerFixture {
    let mut fixture = fixture("valid-func.json");
    compiler_version.clone_into(&mut fixture.compiler_version);
    fixture
}

fn tact_fixture(compiler_version: &str, code_boc: &str) -> WorkerFixture {
    let package = json!({
        "name": "Smoke",
        "code": code_boc,
        "abi": "{}",
        "init": {
            "kind": "direct",
            "args": [],
            "prefix": {"bits": 1, "value": 0},
            "deployment": {"kind": "direct"}
        },
        "sources": {"contract.tact": TACT_SOURCE_BASE64},
        "compiler": {
            "name": "tact",
            "version": compiler_version,
            "parameters": r#"{"entrypoint":"contract.tact","options":{}}"#
        }
    });

    WorkerFixture {
        language: "tact".to_owned(),
        compiler_version: compiler_version.to_owned(),
        import_mappings: None,
        entrypoint: "contract.pkg".to_owned(),
        sources: vec![WorkerSource {
            path: "contract.pkg".to_owned(),
            content: package.to_string(),
            is_entrypoint: true,
            include_in_command: None,
            is_stdlib: None,
            has_include_directives: None,
        }],
    }
}

fn has_extension(path: &str, extension: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|actual| actual.eq_ignore_ascii_case(extension))
}

#[derive(Debug, Deserialize)]
struct WorkerFixture {
    language: String,
    compiler_version: String,
    #[serde(default)]
    import_mappings: Option<Value>,
    entrypoint: String,
    sources: Vec<WorkerSource>,
}

struct TactVersionGroup {
    versions: &'static [&'static str],
    code_boc: &'static str,
    code_hash: &'static str,
}

#[derive(Debug, Deserialize)]
struct WorkerSource {
    path: String,
    content: String,
    #[serde(default)]
    is_entrypoint: bool,
    #[serde(default)]
    include_in_command: Option<bool>,
    #[serde(default)]
    is_stdlib: Option<bool>,
    #[serde(default)]
    has_include_directives: Option<bool>,
}

#[derive(serde::Serialize)]
struct WorkerSourceMetadata<'a> {
    path: &'a str,
    is_entrypoint: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_in_command: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_stdlib: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_include_directives: Option<bool>,
}

impl<'a> WorkerSourceMetadata<'a> {
    fn from_source(source: &'a WorkerSource, entrypoint: &str) -> Self {
        Self {
            path: &source.path,
            is_entrypoint: source.is_entrypoint || source.path == entrypoint,
            include_in_command: source.include_in_command,
            is_stdlib: source.is_stdlib,
            has_include_directives: source.has_include_directives,
        }
    }
}

#[derive(Debug, Deserialize)]
struct VerifyResponse {
    code_hash: String,
    compiled_code_hash: Option<String>,
    verification_result: String,
    source_bundle_hash: Option<String>,
    storage_revision: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VerificationSourceResponse {
    verified: bool,
    bundle: Option<VerifiedSourceBundle>,
}

#[derive(Debug, Deserialize)]
struct VerifiedSourceBundle {
    entrypoint: String,
    compiler: VerifiedCompiler,
    files: Vec<VerifiedSourceFile>,
}

#[derive(Debug, Deserialize)]
struct VerifiedCompiler {
    language: String,
    version: String,
}

#[derive(Debug, Deserialize)]
struct VerifiedSourceFile {
    path: String,
    content: String,
}
