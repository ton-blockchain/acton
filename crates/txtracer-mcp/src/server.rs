use crate::debugger::DebuggerState;
use retrace::Network;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct JsonRpcMessage {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: Option<String>,
    #[serde(default)]
    pub params: Value,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

pub struct McpServer {
    debugger: Arc<Mutex<DebuggerState>>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            debugger: Arc::new(Mutex::new(DebuggerState::new())),
        }
    }

    pub async fn run(&self) -> io::Result<()> {
        let mut stdin = BufReader::new(io::stdin());
        let mut line = String::new();

        while stdin.read_line(&mut line).await? > 0 {
            let msg: JsonRpcMessage = match serde_json::from_str(&line) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("Failed to parse line: {}. Error: {}", line, e);
                    line.clear();
                    continue;
                }
            };

            if let Some(method) = msg.method {
                if let Some(id) = msg.id {
                    let response = self.handle_request(id, method, msg.params).await;
                    self.send_response(response).await?;
                } else {
                    self.handle_notification(method, msg.params).await;
                }
            }
            line.clear();
        }

        Ok(())
    }

    async fn handle_request(&self, id: Value, method: String, params: Value) -> JsonRpcResponse {
        match method.as_str() {
            "initialize" => self.handle_initialize(id).await,
            "tools/list" => self.handle_list_tools(id).await,
            "resources/list" => self.handle_list_resources(id).await,
            "prompts/list" => self.handle_list_prompts(id).await,
            "tools/call" => self.handle_call_tool(id, params).await,
            _ => {
                eprintln!("Unknown method: {}", method);
                self.make_error(id, -32601, &format!("Method not found: {}", method))
            }
        }
    }

    async fn handle_notification(&self, method: String, _params: Value) {
        match method.as_str() {
            "notifications/initialized" => {
                eprintln!("Client initialized");
            }
            _ => {
                eprintln!("Received notification: {}", method);
            }
        }
    }

    async fn handle_initialize(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "txtracer-mcp",
                    "version": "0.1.0"
                }
            })),
            error: None,
        }
    }

    async fn handle_list_tools(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "tools": [
                    {
                        "name": "init_trace",
                        "description": "Initialize a trace from a transaction hash.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "hash": { "type": "string", "description": "Transaction hex hash" },
                                "network": { "type": "string", "enum": ["mainnet", "testnet"], "default": "mainnet" }
                            },
                            "required": ["hash"]
                        }
                    },
                    {
                        "name": "reset_trace",
                        "description": "Resets the debugger to the initial state (Step 0).",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "step",
                        "description": "Move the execution pointer forward or backward.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "delta": { "type": "integer", "description": "Number of steps to move (positive for forward, negative for backward)" }
                            },
                            "required": ["delta"]
                        }
                    },
                    {
                        "name": "get_state",
                        "description": "Get full details of the current execution step.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "inspect_stack",
                        "description": "Get specific items from the stack.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "depth": { "type": "integer", "description": "Number of items to return from top of stack" }
                            }
                        }
                    },
                    {
                        "name": "search_opcode",
                        "description": "Find the nearest step containing a specific opcode.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string", "description": "Opcode name to search for (case-insensitive substring)" },
                                "direction": { "type": "string", "enum": ["forward", "backward"], "default": "forward" }
                            },
                            "required": ["name"]
                        }
                    }
                ]
            })),
            error: None,
        }
    }

    async fn handle_list_resources(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({ "resources": [] })),
            error: None,
        }
    }

    async fn handle_list_prompts(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({ "prompts": [] })),
            error: None,
        }
    }

    async fn handle_call_tool(&self, id: Value, params: Value) -> JsonRpcResponse {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        let mut dbg = self.debugger.lock().await;

        let result: Result<Value, anyhow::Error> = match name {
            "init_trace" => {
                let hash = arguments.get("hash").and_then(|v| v.as_str()).unwrap_or("");
                let network_str = arguments
                    .get("network")
                    .and_then(|v| v.as_str())
                    .unwrap_or("mainnet");
                let network = if network_str == "testnet" {
                    Network::Testnet
                } else {
                    Network::Mainnet
                };

                match dbg.init_from_hash(network, hash).await {
                    Ok(_) => {
                        Ok(json!({ "message": format!("Trace initialized for hash {}", hash) }))
                    }
                    Err(e) => Err(e),
                }
            }
            "reset_trace" => {
                dbg.reset();
                Ok(json!({ "message": "Trace reset to step 0" }))
            }
            "step" => {
                let delta = arguments.get("delta").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
                let step = dbg.step(delta).cloned();
                if let Some(step) = step {
                    let current_step = dbg.current_step;
                    Ok(json!({
                        "current_step": current_step,
                        "step": step
                    }))
                } else {
                    Ok(json!({ "error": "No trace initialized or step out of bounds" }))
                }
            }
            "get_state" => {
                let current_step = dbg.current_step;
                let total_steps = dbg.total_steps();
                let step = dbg.get_current_step().cloned();
                let tx = dbg.get_transaction_details().cloned();

                if let (Some(step), Some(tx)) = (step, tx) {
                    Ok(json!({
                        "current_step": current_step,
                        "total_steps": total_steps,
                        "step": step,
                        "transaction": tx
                    }))
                } else {
                    Ok(json!({ "error": "No trace initialized" }))
                }
            }
            "inspect_stack" => {
                let _depth = arguments
                    .get("depth")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(10) as usize;
                if let Some(step) = dbg.get_current_step() {
                    match step {
                        retrace::trace::TraceStep::Execute { stack, .. } => {
                            Ok(json!({ "stack": stack }))
                        }
                        _ => Ok(json!({ "error": "Current step is not an execution step" })),
                    }
                } else {
                    Ok(json!({ "error": "No trace initialized" }))
                }
            }
            "search_opcode" => {
                let op_name = arguments.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let forward =
                    arguments.get("direction").and_then(|v| v.as_str()) != Some("backward");
                let step = dbg.search_opcode(op_name, forward).cloned();
                if let Some(step) = step {
                    let step_index = dbg.current_step;
                    Ok(json!({
                        "found": true,
                        "step_index": step_index,
                        "step": step
                    }))
                } else {
                    Ok(json!({ "found": false, "message": "Opcode not found" }))
                }
            }
            _ => return self.make_error(id, -32602, &format!("Unknown tool: {}", name)),
        };

        match result {
            Ok(res) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "content": [
                        {
                            "type": "text",
                            "text": serde_json::to_string_pretty(&res).unwrap()
                        }
                    ]
                })),
                error: None,
            },
            Err(e) => self.make_error(id, -32000, &format!("Internal error: {}", e)),
        }
    }

    fn make_error(&self, id: Value, code: i32, message: &str) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(json!({
                "code": code,
                "message": message
            })),
        }
    }

    async fn send_response(&self, resp: JsonRpcResponse) -> io::Result<()> {
        let mut stdout = io::stdout();
        let json = serde_json::to_string(&resp)?;
        stdout.write_all(json.as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
        Ok(())
    }
}
