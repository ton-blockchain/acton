mod support;

use expect_test::expect;
use serde_json::{Value, json};
use std::fs;
use support::LspTestClient;
use ton_language_server_native::ServerConfig;
use tower_lsp::lsp_types::Url;

#[tokio::test]
async fn initialize_selects_one_root_and_keeps_partial_indexes_usable() -> anyhow::Result<()> {
    let fallback = tempfile::tempdir()?;
    let workspace = tempfile::tempdir()?;
    let stdlib = workspace.path().join(".acton/tolk-stdlib");
    fs::create_dir_all(&stdlib)?;
    fs::write(workspace.path().join("Acton.toml"), "")?;
    fs::write(
        workspace.path().join("lib.tolk"),
        "fun helper(): int { return 1; }\n",
    )?;
    fs::write(
        stdlib.join("common.tolk"),
        "fun stdlibHelper(): int { return 2; }\n",
    )?;
    fs::write(workspace.path().join("broken.tolk"), [0xff, 0xfe])?;

    let main_source = concat!(
        "import \"lib\"\n",
        "import \"@stdlib/common\"\n",
        "fun main(): int { return helper() + stdlibHelper(); }\n",
    );
    let main_path = workspace.path().join("main.tolk");
    fs::write(&main_path, main_source)?;

    let workspace_root = workspace.path();
    let root_uri = Url::from_directory_path(workspace_root)
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
    let main_uri = Url::from_file_path(workspace_root.join("main.tolk"))
        .map_err(|()| anyhow::anyhow!("cannot convert main file path to URI"))?;
    let mut config = ServerConfig::new(fallback.path());
    config.enable_profiling = true;
    config.server_version = Some("1.1.0 (test-hash 2026-08-08)".to_owned());
    let (mut client, server) = LspTestClient::start(config).await;

    let initialize = client
        .request(
            "initialize",
            json!({
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {},
            }),
        )
        .await?;
    client.notify("initialized", json!({})).await?;
    client
        .notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "tolk",
                    "version": 1,
                    "text": main_source,
                }
            }),
        )
        .await?;

    let import_completion = client
        .request(
            "textDocument/completion",
            text_document_position(&main_uri, 0, 11),
        )
        .await?;
    let import_item = import_completion["items"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["label"] == "lib"))
        .ok_or_else(|| anyhow::anyhow!("missing lib import completion"))?;

    let helper_definition = client
        .request(
            "textDocument/definition",
            text_document_position(&main_uri, 2, 27),
        )
        .await?;
    let stdlib_definition = client
        .request(
            "textDocument/definition",
            text_document_position(&main_uri, 2, 40),
        )
        .await?;
    let stdlib_references = client
        .request(
            "textDocument/references",
            json!({
                "textDocument": {"uri": main_uri},
                "position": {"line": 2, "character": 40},
                "context": {"includeDeclaration": true},
            }),
        )
        .await?;

    client
        .notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": main_uri, "version": 2},
                "contentChanges": [{
                    "range": {
                        "start": {"line": 2, "character": 25},
                        "end": {"line": 2, "character": 31},
                    },
                    "text": "stdlibHelper",
                }],
            }),
        )
        .await?;
    let edited_definition = client
        .request(
            "textDocument/definition",
            text_document_position(&main_uri, 2, 30),
        )
        .await?;

    client
        .notify(
            "textDocument/didClose",
            json!({"textDocument": {"uri": main_uri}}),
        )
        .await?;
    client
        .notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "tolk",
                    "version": 3,
                    "text": main_source,
                }
            }),
        )
        .await?;
    let reopened_definition = client
        .request(
            "textDocument/definition",
            text_document_position(&main_uri, 2, 27),
        )
        .await?;
    let profile = client.request("ton/profile", json!({})).await?;

    let root = root_uri.as_str().trim_end_matches('/');
    let actual = json!({
        "server": initialize["serverInfo"]["name"],
        "serverVersion": initialize["serverInfo"]["version"],
        "completionTriggers": initialize["capabilities"]["completionProvider"]["triggerCharacters"],
        "importCompletion": {
            "labelDetails": import_item["labelDetails"],
            "detail": import_item["detail"],
        },
        "helper": normalized_definition_uri(&helper_definition, root),
        "stdlib": normalized_definition_uri(&stdlib_definition, root),
        "stdlibReferences": stdlib_references
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|location| location["uri"].as_str())
            .map(|uri| uri.replace(root, "$ROOT"))
            .collect::<Vec<_>>(),
        "edited": normalized_definition_uri(&edited_definition, root),
        "reopened": normalized_definition_uri(&reopened_definition, root),
        "warnings": client.notifications().iter().filter_map(log_warning).collect::<Vec<_>>(),
        "documentEdits": profile["counters"]["document.edit"],
        "documentOpens": profile["counters"]["document.open"],
    });
    expect![[r##"
        {
          "completionTriggers": [
            ".",
            "@",
            "#"
          ],
          "documentEdits": 1,
          "documentOpens": 2,
          "edited": "$ROOT/.acton/tolk-stdlib/common.tolk",
          "helper": "$ROOT/lib.tolk",
          "importCompletion": {
            "detail": null,
            "labelDetails": {
              "detail": ".tolk"
            }
          },
          "reopened": "$ROOT/lib.tolk",
          "server": "Acton Language Server",
          "serverVersion": "1.1.0 (test-hash 2026-08-08)",
          "stdlib": "$ROOT/.acton/tolk-stdlib/common.tolk",
          "stdlibReferences": [
            "$ROOT/main.tolk"
          ],
          "warnings": [
            "workspace.scan: indexed 3 Tolk source files and skipped 1 files or directories"
          ]
        }"##]]
    .assert_eq(&serde_json::to_string_pretty(&actual)?);

    client.shutdown(server).await
}

#[tokio::test]
async fn rename_keeps_open_document_changes_addressable() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(workspace.path().join("Acton.toml"), "")?;
    let old_path = workspace.path().join("old.tolk");
    let new_path = workspace.path().join("new.tolk");
    let old_source = "fun beforeRename(): int { return 1; }\n";
    let new_source = "fun afterRename(): int { return 2; }\n";
    fs::write(&old_path, old_source)?;

    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
    let old_uri = Url::from_file_path(&old_path)
        .map_err(|()| anyhow::anyhow!("cannot convert old file path to URI"))?;
    let new_uri = Url::from_file_path(&new_path)
        .map_err(|()| anyhow::anyhow!("cannot convert new file path to URI"))?;
    let (mut client, server) = LspTestClient::start(ServerConfig::new(workspace.path())).await;

    client
        .request(
            "initialize",
            json!({
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {},
            }),
        )
        .await?;
    client.notify("initialized", json!({})).await?;
    client
        .notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": old_uri,
                    "languageId": "tolk",
                    "version": 1,
                    "text": old_source,
                }
            }),
        )
        .await?;

    fs::rename(&old_path, &new_path)?;
    client
        .notify(
            "workspace/didRenameFiles",
            json!({
                "files": [{"oldUri": old_uri, "newUri": new_uri}],
            }),
        )
        .await?;
    client
        .notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": new_uri, "version": 2},
                "contentChanges": [{"text": new_source}],
            }),
        )
        .await?;

    let symbols = client
        .request(
            "textDocument/documentSymbol",
            json!({"textDocument": {"uri": new_uri}}),
        )
        .await?;
    let actual = json!({
        "symbols": symbols
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|symbol| symbol["name"].as_str())
            .collect::<Vec<_>>(),
        "errors": client
            .notifications()
            .iter()
            .filter_map(|notification| {
                (notification["method"] == "window/logMessage"
                    && notification["params"]["type"] == 1)
                    .then(|| notification["params"]["message"].as_str())
                    .flatten()
            })
            .collect::<Vec<_>>(),
    });
    expect![[r#"
        {
          "errors": [],
          "symbols": [
            "afterRename"
          ]
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&actual)?);

    client.shutdown(server).await
}

#[tokio::test]
async fn nested_manifest_does_not_replace_workspace_configuration() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    let root_manifest = concat!(
        "[package]\n",
        "name = \"fixture\"\n",
        "version = \"0.1.0\"\n\n",
        "[import-mappings]\n",
        "lib = \"./root-lib\"\n",
    );
    let nested_manifest = concat!(
        "[package]\n",
        "name = \"nested\"\n",
        "version = \"0.1.0\"\n\n",
        "[import-mappings]\n",
        "lib = \"./shadow-lib\"\n",
    );
    fs::create_dir_all(workspace.path().join("root-lib"))?;
    fs::create_dir_all(workspace.path().join("shadow-lib"))?;
    fs::create_dir_all(workspace.path().join("nested"))?;
    fs::write(workspace.path().join("Acton.toml"), root_manifest)?;
    fs::write(
        workspace.path().join("root-lib/helper.tolk"),
        "fun helper(): int { return 1; }\n",
    )?;
    fs::write(
        workspace.path().join("shadow-lib/helper.tolk"),
        "fun helper(): int { return 2; }\n",
    )?;
    let nested_manifest_path = workspace.path().join("nested/Acton.toml");
    fs::write(&nested_manifest_path, nested_manifest)?;
    let main_source = "import \"@lib/helper\"\nfun main(): int { return helper(); }\n";
    let main_path = workspace.path().join("main.tolk");
    fs::write(&main_path, main_source)?;

    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
    let main_uri = Url::from_file_path(&main_path)
        .map_err(|()| anyhow::anyhow!("cannot convert main file path to URI"))?;
    let nested_manifest_uri = Url::from_file_path(&nested_manifest_path)
        .map_err(|()| anyhow::anyhow!("cannot convert nested manifest path to URI"))?;
    let (mut client, server) = LspTestClient::start(ServerConfig::new(workspace.path())).await;

    client
        .request(
            "initialize",
            json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
        )
        .await?;
    client.notify("initialized", json!({})).await?;
    client
        .notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "tolk",
                    "version": 1,
                    "text": main_source,
                }
            }),
        )
        .await?;
    client
        .notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": nested_manifest_uri,
                    "languageId": "toml",
                    "version": 1,
                    "text": nested_manifest,
                }
            }),
        )
        .await?;
    client
        .notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": nested_manifest_uri, "version": 2},
                "contentChanges": [{"text": nested_manifest}],
            }),
        )
        .await?;
    client
        .notify(
            "textDocument/didSave",
            json!({"textDocument": {"uri": nested_manifest_uri}, "text": nested_manifest}),
        )
        .await?;
    client
        .notify(
            "workspace/didChangeWatchedFiles",
            json!({"changes": [{"uri": nested_manifest_uri, "type": 2}]}),
        )
        .await?;

    let definition = client
        .request(
            "textDocument/definition",
            text_document_position(&main_uri, 1, 27),
        )
        .await?;
    let actual = normalized_definition_uri(&definition, root_uri.as_str().trim_end_matches('/'));
    expect!["$ROOT/root-lib/helper.tolk"].assert_eq(&actual);

    client.shutdown(server).await
}

#[tokio::test]
async fn untitled_documents_use_their_declared_language_settings() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(workspace.path().join("Acton.toml"), "")?;
    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
    let untitled_uri = Url::parse("untitled:Untitled-1")?;
    let source = "const COMPUTED = 1 + 2\n";
    let (mut client, server) = LspTestClient::start(ServerConfig::new(workspace.path())).await;

    client
        .request(
            "initialize",
            json!({"processId": null, "rootUri": root_uri, "capabilities": {}}),
        )
        .await?;
    client.notify("initialized", json!({})).await?;
    client
        .notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": untitled_uri,
                    "languageId": "tolk",
                    "version": 1,
                    "text": source,
                }
            }),
        )
        .await?;

    let range = json!({
        "textDocument": {"uri": untitled_uri},
        "range": {
            "start": {"line": 0, "character": 0},
            "end": {"line": 1, "character": 0},
        },
    });
    let before = client
        .request("textDocument/inlayHint", range.clone())
        .await?;
    client
        .notify(
            "workspace/didChangeConfiguration",
            json!({"settings": {"ton": {"tolk": {"hints": {"disable": true}}}}}),
        )
        .await?;
    let after = client.request("textDocument/inlayHint", range).await?;

    let actual = json!({
        "before": before
            .as_array()
            .into_iter()
            .flatten()
            .map(|hint| hint["label"].clone())
            .collect::<Vec<_>>(),
        "after": after,
    });
    expect![[r#"
        {
          "after": [],
          "before": [
            ": int",
            " /* = 3 (0x3) */"
          ]
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&actual)?);

    client.shutdown(server).await
}

#[tokio::test]
async fn initialize_uses_cli_root_when_the_client_omits_its_root() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(workspace.path().join("Acton.toml"), "")?;
    fs::write(
        workspace.path().join("lib.tolk"),
        "fun helper(): int { return 1; }\n",
    )?;

    let main_source = "import \"lib\"\nfun main(): int { return helper(); }\n";
    let main_uri = Url::from_file_path(workspace.path().join("main.tolk"))
        .map_err(|()| anyhow::anyhow!("cannot convert main file path to URI"))?;
    fs::write(workspace.path().join("main.tolk"), main_source)?;

    let (mut client, server) = LspTestClient::start(ServerConfig::new(workspace.path())).await;
    client
        .request(
            "initialize",
            json!({
                "processId": null,
                "capabilities": {},
            }),
        )
        .await?;
    client.notify("initialized", json!({})).await?;
    client
        .notify(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": main_uri,
                    "languageId": "tolk",
                    "version": 1,
                    "text": main_source,
                }
            }),
        )
        .await?;

    let definition = client
        .request(
            "textDocument/definition",
            text_document_position(&main_uri, 1, 27),
        )
        .await?;
    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert CLI root to URI"))?;
    let actual = normalized_definition_uri(&definition, root_uri.as_str().trim_end_matches('/'));

    expect!["$ROOT/lib.tolk"].assert_eq(&actual);
    client.shutdown(server).await
}

fn text_document_position(uri: &Url, line: u32, character: u32) -> Value {
    json!({
        "textDocument": {"uri": uri},
        "position": {"line": line, "character": character},
    })
}

fn normalized_definition_uri(definition: &Value, root: &str) -> String {
    let definition = definition
        .as_array()
        .and_then(|items| items.first())
        .unwrap_or(definition);
    definition
        .get("uri")
        .or_else(|| definition.get("targetUri"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .replace(root, "$ROOT")
}

fn log_warning(notification: &Value) -> Option<&str> {
    (notification["method"] == "window/logMessage" && notification["params"]["type"] == 2)
        .then(|| notification["params"]["message"].as_str())
        .flatten()
}
