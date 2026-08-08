use serde_json::{Value, json};
use tokio::io::{
    AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf,
};
use tokio::task::JoinHandle;
use ton_language_server_native::ServerConfig;

pub(crate) struct LspTestClient {
    reader: BufReader<ReadHalf<DuplexStream>>,
    writer: WriteHalf<DuplexStream>,
    next_request_id: u64,
    notifications: Vec<Value>,
}

impl LspTestClient {
    pub(crate) async fn start(config: ServerConfig) -> (Self, JoinHandle<anyhow::Result<()>>) {
        let (client_stream, server_stream) = tokio::io::duplex(1024 * 1024);
        let (client_reader, client_writer) = tokio::io::split(client_stream);
        let (server_reader, server_writer) = tokio::io::split(server_stream);
        let server = tokio::spawn(async move {
            ton_language_server_native::serve_stream(config, server_reader, server_writer).await
        });

        (
            Self {
                reader: BufReader::new(client_reader),
                writer: client_writer,
                next_request_id: 1,
                notifications: Vec::new(),
            },
            server,
        )
    }

    pub(crate) async fn request(&mut self, method: &str, params: Value) -> anyhow::Result<Value> {
        let id = self.next_request_id;
        self.next_request_id += 1;
        let mut message = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
        });
        if !params.is_null() {
            message["params"] = params;
        }
        self.write_message(&message).await?;

        loop {
            let message = self.read_message().await?;
            if message.get("id").and_then(Value::as_u64) == Some(id)
                && message.get("method").is_none()
            {
                if let Some(error) = message.get("error") {
                    anyhow::bail!("request {method} failed: {error}");
                }
                return Ok(message.get("result").cloned().unwrap_or(Value::Null));
            }
            if let (Some(request_id), Some(_)) = (message.get("id"), message.get("method")) {
                self.write_message(&json!({
                    "jsonrpc": "2.0",
                    "id": request_id,
                    "result": null,
                }))
                .await?;
            } else {
                self.notifications.push(message);
            }
        }
    }

    pub(crate) async fn notify(&mut self, method: &str, params: Value) -> anyhow::Result<()> {
        let mut message = json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        if !params.is_null() {
            message["params"] = params;
        }
        self.write_message(&message).await
    }

    pub(crate) fn notifications(&self) -> &[Value] {
        &self.notifications
    }

    pub(crate) async fn shutdown(
        mut self,
        server: JoinHandle<anyhow::Result<()>>,
    ) -> anyhow::Result<()> {
        self.request("shutdown", Value::Null).await?;
        self.notify("exit", Value::Null).await?;
        self.writer.shutdown().await?;
        drop(self);

        tokio::time::timeout(std::time::Duration::from_secs(5), server)
            .await
            .map_err(|_| anyhow::anyhow!("language server did not stop after exit"))??
    }

    async fn write_message(&mut self, message: &Value) -> anyhow::Result<()> {
        let body = serde_json::to_vec(message)?;
        self.writer
            .write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes())
            .await?;
        self.writer.write_all(&body).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn read_message(&mut self) -> anyhow::Result<Value> {
        let mut content_length = None;
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line).await? == 0 {
                anyhow::bail!("language server closed the transport");
            }
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length:") {
                content_length = Some(value.trim().parse::<usize>()?);
            }
        }

        let Some(content_length) = content_length else {
            anyhow::bail!("language server response has no Content-Length header");
        };
        let mut body = vec![0; content_length];
        self.reader.read_exact(&mut body).await?;
        Ok(serde_json::from_slice(&body)?)
    }
}
