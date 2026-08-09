mod support;

use expect_test::expect;
use serde_json::{Value, json};
use std::fs;
use support::LspTestClient;
use ton_language_server_native::ServerConfig;
use tower_lsp::lsp_types::Url;

#[tokio::test]
async fn dynamically_registers_workspace_file_watchers() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(workspace.path().join("Acton.toml"), "")?;
    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
    let (mut client, server) = LspTestClient::start(ServerConfig::new(workspace.path())).await;

    client
        .request(
            "initialize",
            json!({
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {
                    "workspace": {
                        "didChangeWatchedFiles": {
                            "dynamicRegistration": true
                        }
                    }
                }
            }),
        )
        .await?;
    client.notify("initialized", json!({})).await?;
    let registration = client.receive_server_request().await?;

    expect![[r#"
        {
          "method": "client/registerCapability",
          "params": {
            "registrations": [
              {
                "id": "ton-language-server-watched-files",
                "method": "workspace/didChangeWatchedFiles",
                "registerOptions": {
                  "watchers": [
                    {
                      "globPattern": "**/*.{tolk,tasm,fif,fift,tlb}"
                    },
                    {
                      "globPattern": "**/Acton.toml"
                    }
                  ]
                }
              }
            ]
          }
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&json!({
        "method": registration["method"],
        "params": registration["params"],
    }))?);

    client.shutdown(server).await
}

#[tokio::test]
async fn tolk_inlay_hints_link_types_and_parameters_to_declarations() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(workspace.path().join("Acton.toml"), "")?;
    let source = concat!(
        "struct Payload { value: int }\n",
        "struct map<K, V> {}\n",
        "fun createPayload(): Payload { return { value: 1 }; }\n",
        "fun createPayloadMap(): map<int, Payload> asm \"\";\n",
        "fun consumePayload(payload: Payload) {}\n",
        "fun main() {\n",
        "    val value = createPayload();\n",
        "    val values = createPayloadMap();\n",
        "    consumePayload(createPayload());\n",
        "}\n",
    );
    let main_path = workspace.path().join("main.tolk");
    fs::write(&main_path, source)?;
    let main_uri = Url::from_file_path(main_path)
        .map_err(|()| anyhow::anyhow!("cannot convert main file path to URI"))?;
    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
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
                    "text": source,
                }
            }),
        )
        .await?;
    let hints = client
        .request(
            "textDocument/inlayHint",
            json!({
                "textDocument": {"uri": main_uri},
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 9, "character": 1},
                }
            }),
        )
        .await?;

    let mut clickable = hints
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|hint| hint["label"].as_array())
        .map(|parts| {
            let label = parts
                .iter()
                .filter_map(|part| part["value"].as_str())
                .collect::<String>();
            json!({
                "label": label,
                "parts": parts.iter().map(|part| json!({
                    "value": part["value"],
                    "target": part.get("location").map(|location| json!({
                        "sameDocument": location["uri"] == main_uri.as_str(),
                        "range": location["range"],
                    })),
                })).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    clickable.sort_by(|left, right| left["label"].as_str().cmp(&right["label"].as_str()));
    expect![[r#"
        [
          {
            "label": ": Payload",
            "parts": [
              {
                "target": null,
                "value": ": "
              },
              {
                "target": {
                  "range": {
                    "end": {
                      "character": 14,
                      "line": 0
                    },
                    "start": {
                      "character": 7,
                      "line": 0
                    }
                  },
                  "sameDocument": true
                },
                "value": "Payload"
              }
            ]
          },
          {
            "label": ": map<int, Payload>",
            "parts": [
              {
                "target": null,
                "value": ": "
              },
              {
                "target": {
                  "range": {
                    "end": {
                      "character": 10,
                      "line": 1
                    },
                    "start": {
                      "character": 7,
                      "line": 1
                    }
                  },
                  "sameDocument": true
                },
                "value": "map"
              },
              {
                "target": null,
                "value": "<int, "
              },
              {
                "target": {
                  "range": {
                    "end": {
                      "character": 14,
                      "line": 0
                    },
                    "start": {
                      "character": 7,
                      "line": 0
                    }
                  },
                  "sameDocument": true
                },
                "value": "Payload"
              },
              {
                "target": null,
                "value": ">"
              }
            ]
          },
          {
            "label": "payload:",
            "parts": [
              {
                "target": {
                  "range": {
                    "end": {
                      "character": 26,
                      "line": 4
                    },
                    "start": {
                      "character": 19,
                      "line": 4
                    }
                  },
                  "sameDocument": true
                },
                "value": "payload"
              },
              {
                "target": null,
                "value": ":"
              }
            ]
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&clickable)?);

    client.shutdown(server).await
}

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
async fn publishes_linter_diagnostics_on_open_change_and_close() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(
        workspace.path().join("Acton.toml"),
        "[lint.rules]\nname-case-checker = \"deny\"\n",
    )?;
    let main_uri = Url::from_file_path(workspace.path().join("main.tolk"))
        .map_err(|()| anyhow::anyhow!("cannot convert main file path to URI"))?;
    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
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
                    "text": "fun BadName() {}",
                }
            }),
        )
        .await?;
    let after_open = wait_for_published_diagnostics(&mut client, 0).await?;
    let after_open_count = published_diagnostics_count(&client);

    client
        .notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": main_uri, "version": 2},
                "contentChanges": [{"text": "fun goodName() {}"}],
            }),
        )
        .await?;
    let after_change = wait_for_published_diagnostics(&mut client, after_open_count).await?;
    let after_change_count = published_diagnostics_count(&client);

    client
        .notify(
            "textDocument/didClose",
            json!({"textDocument": {"uri": main_uri}}),
        )
        .await?;
    let after_close = wait_for_published_diagnostics(&mut client, after_change_count).await?;

    expect![[r#"
        {
          "change": {
            "diagnostics": [],
            "uri": "$MAIN",
            "version": 2
          },
          "close": {
            "diagnostics": [],
            "uri": "$MAIN"
          },
          "open": {
            "diagnostics": [
              {
                "code": "S001",
                "message": "name should be in the expected case\nnot camelCase: `BadName`",
                "range": {
                  "end": {
                    "character": 11,
                    "line": 0
                  },
                  "start": {
                    "character": 4,
                    "line": 0
                  }
                },
                "severity": 1,
                "source": "acton"
              }
            ],
            "uri": "$MAIN",
            "version": 1
          }
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&json!({
        "open": normalize_diagnostic_uri(after_open, &main_uri),
        "change": normalize_diagnostic_uri(after_change, &main_uri),
        "close": normalize_diagnostic_uri(after_close, &main_uri),
    }))?);

    client.shutdown(server).await
}

#[tokio::test]
async fn compiler_diagnostics_handle_cyrillic_return_type() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(workspace.path().join("Acton.toml"), "")?;
    let main_uri = Url::from_file_path(workspace.path().join("main.tolk"))
        .map_err(|()| anyhow::anyhow!("cannot convert main file path to URI"))?;
    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
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
                    "text": "fun name(): ж asm \"\"",
                }
            }),
        )
        .await?;
    let initial_diagnostic_count = published_diagnostics_count(&client);
    let diagnostics = wait_for_published_diagnostics(&mut client, initial_diagnostic_count).await?;

    let compiler_diagnostics = diagnostics["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|diagnostic| diagnostic["source"] == "tolk-compiler")
        .collect::<Vec<_>>();
    expect![[r#"
        [
          {
            "code": "C001",
            "message": "failed to parse",
            "range": {
              "end": {
                "character": 13,
                "line": 0
              },
              "start": {
                "character": 12,
                "line": 0
              }
            },
            "severity": 1,
            "source": "tolk-compiler"
          }
        ]"#]]
    .assert_eq(&serde_json::to_string_pretty(&compiler_diagnostics)?);

    client.shutdown(server).await
}

#[tokio::test]
async fn diagnostic_settings_apply_at_initialization_and_runtime() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(
        workspace.path().join("Acton.toml"),
        "[lint.rules]\nname-case-checker = \"deny\"\n",
    )?;
    let main_uri = Url::from_file_path(workspace.path().join("main.tolk"))
        .map_err(|()| anyhow::anyhow!("cannot convert main file path to URI"))?;
    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
    let (mut client, server) = LspTestClient::start(ServerConfig::new(workspace.path())).await;

    client
        .request(
            "initialize",
            json!({
                "processId": null,
                "rootUri": root_uri,
                "capabilities": {},
                "initializationOptions": {
                    "tolk": {
                        "diagnostics": {
                            "linter": {"enabled": false}
                        }
                    }
                }
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
                    "text": "fun BadName(): int { return missingName; }",
                }
            }),
        )
        .await?;
    let compiler_only = wait_for_published_diagnostics(&mut client, 0).await?;
    let compiler_only_count = published_diagnostics_count(&client);

    client
        .notify(
            "workspace/didChangeConfiguration",
            json!({
                "settings": {
                    "ton": {
                        "tolk": {
                            "diagnostics": {
                                "linter": {"enabled": true},
                                "compiler": {"enabled": false}
                            }
                        }
                    }
                }
            }),
        )
        .await?;
    let linter_only = wait_for_published_diagnostics(&mut client, compiler_only_count).await?;
    let linter_only_count = published_diagnostics_count(&client);

    client
        .notify(
            "workspace/didChangeConfiguration",
            json!({
                "settings": {
                    "tolk": {
                        "diagnostics": {"enabled": false}
                    }
                }
            }),
        )
        .await?;
    let disabled = wait_for_published_diagnostics(&mut client, linter_only_count).await?;

    let diagnostic_ids = |publish: &Value| {
        publish["diagnostics"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|diagnostic| {
                format!(
                    "{}:{}",
                    diagnostic["source"].as_str().unwrap_or_default(),
                    diagnostic["code"].as_str().unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
    };
    expect![[r#"
        {
          "compilerOnly": [
            "tolk-compiler:C001"
          ],
          "disabled": [],
          "linterOnly": [
            "acton:S001"
          ]
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&json!({
        "compilerOnly": diagnostic_ids(&compiler_only),
        "linterOnly": diagnostic_ids(&linter_only),
        "disabled": diagnostic_ids(&disabled),
    }))?);

    client.shutdown(server).await
}

#[tokio::test]
async fn compiler_diagnostics_use_unsaved_document_text() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(workspace.path().join("Acton.toml"), "")?;
    let main_uri = Url::from_file_path(workspace.path().join("new.tolk"))
        .map_err(|()| anyhow::anyhow!("cannot convert source file path to URI"))?;
    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
    let mut config = ServerConfig::new(workspace.path());
    config.enable_profiling = true;
    let (mut client, server) = LspTestClient::start(config).await;

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
                    "text": "fun helper(): int { val face = \"😀\"; return missingName; }\n",
                }
            }),
        )
        .await?;
    let after_open = wait_for_published_diagnostics(&mut client, 0).await?;
    let after_open_count = published_diagnostics_count(&client);

    client
        .notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": main_uri, "version": 2},
                "contentChanges": [{"text": "fun helper(): int { val face = \"😀\"; return 1; }\n"}],
            }),
        )
        .await?;
    let after_change = wait_for_published_diagnostics(&mut client, after_open_count).await?;
    let profile = client.request("ton/profile", json!({})).await?;

    let compiler_diagnostics = |publish: &Value| {
        publish["diagnostics"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|diagnostic| diagnostic["source"] == "tolk-compiler")
            .cloned()
            .collect::<Vec<_>>()
    };
    let actual = json!({
        "open": compiler_diagnostics(&after_open),
        "change": compiler_diagnostics(&after_change),
        "fileExists": workspace.path().join("new.tolk").exists(),
        "profileCounts": profile["spans"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(name, _)| name.starts_with("tolk.diagnostics.compiler."))
            .map(|(name, span)| (name.clone(), span["count"].clone()))
            .collect::<serde_json::Map<_, _>>(),
    });
    expect![[r#"
        {
          "change": [],
          "fileExists": false,
          "open": [
            {
              "code": "C001",
              "message": "undefined symbol `missingName`",
              "range": {
                "end": {
                  "character": 55,
                  "line": 0
                },
                "start": {
                  "character": 44,
                  "line": 0
                }
              },
              "severity": 1,
              "source": "tolk-compiler"
            }
          ],
          "profileCounts": {
            "tolk.diagnostics.compiler.check": 2,
            "tolk.diagnostics.compiler.convert": 2,
            "tolk.diagnostics.compiler.prepare": 2
          }
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&actual)?);

    client.shutdown(server).await
}

#[tokio::test]
async fn call_hierarchy_serves_prepare_incoming_and_outgoing_requests() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(workspace.path().join("Acton.toml"), "")?;
    fs::write(
        workspace.path().join("lib.tolk"),
        "fun helper(): int { return 1; }\nfun fromLib(): int { return helper(); }\n",
    )?;
    let main_source = concat!(
        "import \"lib\"\n",
        "fun caller(): int { return helper() + helper(); }\n",
    );
    let main_path = workspace.path().join("main.tolk");
    fs::write(&main_path, main_source)?;

    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
    let main_uri = Url::from_file_path(&main_path)
        .map_err(|()| anyhow::anyhow!("cannot convert main file path to URI"))?;
    let (mut client, server) = LspTestClient::start(ServerConfig::new(workspace.path())).await;
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

    let caller_items = client
        .request(
            "textDocument/prepareCallHierarchy",
            text_document_position(&main_uri, 1, 5),
        )
        .await?;
    let caller = caller_items
        .as_array()
        .and_then(|items| items.first())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("missing caller hierarchy item"))?;
    let outgoing = client
        .request("callHierarchy/outgoingCalls", json!({"item": caller}))
        .await?;

    let helper_items = client
        .request(
            "textDocument/prepareCallHierarchy",
            text_document_position(&main_uri, 1, 29),
        )
        .await?;
    let helper = helper_items
        .as_array()
        .and_then(|items| items.first())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("missing helper hierarchy item"))?;
    let incoming = client
        .request("callHierarchy/incomingCalls", json!({"item": helper}))
        .await?;

    let root = root_uri.as_str().trim_end_matches('/');
    let item_label = |item: &Value| {
        format!(
            "{}@{}",
            item["name"].as_str().unwrap_or("<missing>"),
            item["uri"]
                .as_str()
                .unwrap_or("<missing>")
                .replace(root, "$ROOT"),
        )
    };
    let call_ranges = |call: &Value| {
        call["fromRanges"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|range| format!("{}:{}", range["start"]["line"], range["start"]["character"]))
            .collect::<Vec<_>>()
    };
    let actual = json!({
        "capability": initialize["capabilities"]["callHierarchyProvider"],
        "caller": item_label(&caller),
        "helper": item_label(&helper),
        "incoming": incoming.as_array().into_iter().flatten().map(|call| json!({
            "from": item_label(&call["from"]),
            "ranges": call_ranges(call),
        })).collect::<Vec<_>>(),
        "outgoing": outgoing.as_array().into_iter().flatten().map(|call| json!({
            "to": item_label(&call["to"]),
            "ranges": call_ranges(call),
        })).collect::<Vec<_>>(),
    });
    expect![[r#"
        {
          "caller": "caller@$ROOT/main.tolk",
          "capability": true,
          "helper": "helper@$ROOT/lib.tolk",
          "incoming": [
            {
              "from": "fromLib@$ROOT/lib.tolk",
              "ranges": [
                "1:28"
              ]
            },
            {
              "from": "caller@$ROOT/main.tolk",
              "ranges": [
                "1:27",
                "1:38"
              ]
            }
          ],
          "outgoing": [
            {
              "ranges": [
                "1:27",
                "1:38"
              ],
              "to": "helper@$ROOT/lib.tolk"
            }
          ]
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&actual)?);

    client.shutdown(server).await
}

#[tokio::test]
async fn selection_ranges_advertise_capability_and_preserve_position_order() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(workspace.path().join("Acton.toml"), "")?;
    let source = "fun main() { val face = \"😀\"; return face; }\n";
    let main_path = workspace.path().join("main.tolk");
    fs::write(&main_path, source)?;

    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
    let main_uri = Url::from_file_path(&main_path)
        .map_err(|()| anyhow::anyhow!("cannot convert main file path to URI"))?;
    let (mut client, server) = LspTestClient::start(ServerConfig::new(workspace.path())).await;
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
                    "text": source,
                }
            }),
        )
        .await?;

    let ranges = client
        .request(
            "textDocument/selectionRange",
            json!({
                "textDocument": {"uri": main_uri},
                "positions": [
                    {"line": 0, "character": 37},
                    {"line": 0, "character": 4},
                ],
            }),
        )
        .await?;

    let actual = json!({
        "provider": initialize["capabilities"]["selectionRangeProvider"],
        "results": ranges
            .as_array()
            .into_iter()
            .flatten()
            .map(selection_range_chain)
            .collect::<Vec<_>>(),
    });
    expect![[r#"
        {
          "provider": true,
          "results": [
            [
              "0:37-0:41",
              "0:30-0:41",
              "0:11-0:44",
              "0:0-0:44",
              "0:0-1:0"
            ],
            [
              "0:4-0:8",
              "0:0-0:44",
              "0:0-1:0"
            ]
          ]
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(&actual)?);

    client.shutdown(server).await
}

#[tokio::test]
async fn formatting_errors_are_reported_as_request_failures() -> anyhow::Result<()> {
    let workspace = tempfile::tempdir()?;
    fs::write(workspace.path().join("Acton.toml"), "")?;
    let file_uri = Url::from_file_path(workspace.path().join("main.tolk"))
        .map_err(|()| anyhow::anyhow!("cannot convert file path to URI"))?;
    let root_uri = Url::from_directory_path(workspace.path())
        .map_err(|()| anyhow::anyhow!("cannot convert workspace path to URI"))?;
    let mut config = ServerConfig::new(workspace.path());
    config.enable_profiling = true;
    let (mut client, server) = LspTestClient::start(config).await;

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
                    "uri": file_uri,
                    "languageId": "tolk",
                    "version": 1,
                    "text": "fun main( {\n",
                }
            }),
        )
        .await?;

    let response = client
        .request_response(
            "textDocument/formatting",
            json!({
                "textDocument": {"uri": file_uri},
                "options": {"tabSize": 4, "insertSpaces": true},
            }),
        )
        .await?;
    let range_response = client
        .request_response(
            "textDocument/rangeFormatting",
            json!({
                "textDocument": {"uri": file_uri},
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 0},
                },
                "options": {"tabSize": 4, "insertSpaces": true},
            }),
        )
        .await?;
    let profile = client.request("ton/profile", json!({})).await?;
    let actual = json!({
        "document": {
            "code": response["error"]["code"],
            "message": response["error"]["message"],
        },
        "range": {
            "code": range_response["error"]["code"],
            "message": range_response["error"]["message"],
        },
        "serverStillResponds": profile.is_object(),
    });

    expect![[r#"
        {
          "document": {
            "code": -32803,
            "message": "Cannot format code with syntax error"
          },
          "range": {
            "code": -32803,
            "message": "Cannot format code with syntax error"
          },
          "serverStillResponds": true
        }"#]]
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

fn selection_range_chain(value: &Value) -> Vec<String> {
    let mut ranges = Vec::new();
    let mut current = Some(value);
    while let Some(range) = current {
        let start = &range["range"]["start"];
        let end = &range["range"]["end"];
        ranges.push(format!(
            "{}:{}-{}:{}",
            start["line"], start["character"], end["line"], end["character"]
        ));
        current = range.get("parent");
    }
    ranges
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

fn last_published_diagnostics(client: &LspTestClient) -> Option<&Value> {
    client
        .notifications()
        .iter()
        .rev()
        .find(|notification| notification["method"] == "textDocument/publishDiagnostics")
        .map(|notification| &notification["params"])
}

fn published_diagnostics_count(client: &LspTestClient) -> usize {
    client
        .notifications()
        .iter()
        .filter(|notification| notification["method"] == "textDocument/publishDiagnostics")
        .count()
}

async fn wait_for_published_diagnostics(
    client: &mut LspTestClient,
    previous_count: usize,
) -> anyhow::Result<Value> {
    for _ in 0..20 {
        client.request("ton/profile", json!({})).await?;
        if published_diagnostics_count(client) > previous_count {
            return last_published_diagnostics(client)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("missing published diagnostics"));
        }
        tokio::task::yield_now().await;
    }
    anyhow::bail!("language server did not publish diagnostics")
}

fn normalize_diagnostic_uri(mut params: Value, uri: &Url) -> Value {
    if params["uri"] == uri.as_str() {
        params["uri"] = Value::String("$MAIN".to_owned());
    }
    params
}
