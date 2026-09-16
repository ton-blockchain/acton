use crate::common::acton_exe;
use anyhow::Context as _;
use expect_test::expect;
use serde_json::{Value, json};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout, Command};
use tokio::time::timeout;

#[tokio::test]
async fn shutdown_and_exit_stop_the_process_with_stdin_open() -> anyhow::Result<()> {
    expect![[r#"
        {
          "exitCode": 0,
          "stderr": ""
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(
        &exit_with_stdin_open(true).await?,
    )?);
    Ok(())
}

#[tokio::test]
async fn exit_without_shutdown_stops_the_process_with_an_error() -> anyhow::Result<()> {
    expect![[r#"
        {
          "exitCode": 1,
          "stderr": "Error: language server received exit before shutdown\n"
        }"#]]
    .assert_eq(&serde_json::to_string_pretty(
        &exit_with_stdin_open(false).await?,
    )?);
    Ok(())
}

async fn exit_with_stdin_open(shutdown: bool) -> anyhow::Result<Value> {
    let workspace = tempfile::tempdir()?;
    let mut server = Command::new(acton_exe())
        .args(["ls", "--stdio", "--no-log", "--color", "never"])
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let mut writer = server.stdin.take().expect("server stdin");
    let mut reader = BufReader::new(server.stdout.take().expect("server stdout"));

    write_message(
        &mut writer,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "processId": null, "rootUri": null, "capabilities": {}
        }}),
    )
    .await?;
    let initialized = timeout(Duration::from_secs(10), read_response(&mut reader, 1)).await??;
    if initialized.get("error").is_some() {
        anyhow::bail!("initialize failed: {initialized}");
    }
    write_message(
        &mut writer,
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
    )
    .await?;

    if shutdown {
        write_message(
            &mut writer,
            json!({"jsonrpc": "2.0", "id": 2, "method": "shutdown"}),
        )
        .await?;
        let response = timeout(Duration::from_secs(10), read_response(&mut reader, 2)).await??;
        expect![[r#"{"id":2,"jsonrpc":"2.0","result":null}"#]].assert_eq(&response.to_string());
    }

    write_message(&mut writer, json!({"jsonrpc": "2.0", "method": "exit"})).await?;

    // Child::wait closes stdin unless its handle was taken above. Keep that handle
    // alive until the process exits, as language clients do after sending exit.
    let status = timeout(Duration::from_secs(5), server.wait())
        .await
        .context("language server did not exit while stdin remained open")??;
    drop(writer);

    let mut stderr = String::new();
    server
        .stderr
        .take()
        .expect("server stderr")
        .read_to_string(&mut stderr)
        .await?;
    Ok(json!({"exitCode": status.code(), "stderr": stderr}))
}

async fn write_message(writer: &mut ChildStdin, message: Value) -> anyhow::Result<()> {
    let body = serde_json::to_vec(&message)?;
    writer
        .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
        .await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}

async fn read_response(reader: &mut BufReader<ChildStdout>, id: u64) -> anyhow::Result<Value> {
    loop {
        let mut content_length = None;
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).await? == 0 {
                anyhow::bail!("language server closed stdout before response {id}");
            }
            if header == "\r\n" {
                break;
            }
            if let Some(length) = header.strip_prefix("Content-Length:") {
                content_length = Some(length.trim().parse::<usize>()?);
            }
        }

        let mut body =
            vec![0; content_length.ok_or_else(|| anyhow::anyhow!("missing Content-Length"))?];
        reader.read_exact(&mut body).await?;
        let message: Value = serde_json::from_slice(&body)?;
        if message.get("id").and_then(Value::as_u64) == Some(id) {
            return Ok(message);
        }
    }
}
