use std::sync::Arc;

use cerebro::CerebroCortex;
use serde_json::{json, Value};

use crate::tools;

pub fn handle_initialize(req: &Value) -> Value {
    let id = &req["id"];
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": {
                "name": "cerebro-mcp",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    })
}

pub fn tools_list() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": null,
        "result": { "tools": tools::all_tool_schemas() }
    })
}

pub async fn dispatch_tool(msg: Value, brain: Arc<CerebroCortex>) -> Value {
    let id     = msg["id"].clone();
    let params = &msg["params"];
    let name   = params["name"].as_str().unwrap_or("");
    let args   = &params["arguments"];

    let result = route(name, args, brain).await;
    match result {
        Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": { "content": [{ "type": "text", "text": v.to_string() }] } }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32603, "message": e.to_string() } }),
    }
}

pub fn method_not_found(req: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": req["id"],
        "error": { "code": -32601, "message": "method not found" }
    })
}

// ---------------------------------------------------------------------------
// Tool routing — stub; will be fully wired in build-order step 8-9
// ---------------------------------------------------------------------------

async fn route(name: &str, args: &Value, brain: Arc<CerebroCortex>) -> anyhow::Result<Value> {
    use cerebro::types::{MemoryType, VisibilityScope};

    match name {
        "remember" => {
            let content     = args["content"].as_str().unwrap_or("").to_string();
            let memory_type = serde_json::from_value(args["memory_type"].clone())
                .unwrap_or(MemoryType::Semantic);
            let scope = VisibilityScope::global();
            let node = brain.remember(content, memory_type, scope).await?;
            Ok(serde_json::to_value(node)?)
        }
        "recall" => {
            let query = args["query"].as_str().unwrap_or("");
            let k     = args["k"].as_u64().unwrap_or(10) as usize;
            let scope = VisibilityScope::global();
            let results = brain.recall(query, k, scope).await?;
            Ok(serde_json::to_value(results)?)
        }
        _ => {
            // All other tools return a stub response until step 9
            Ok(json!({ "status": "not_yet_implemented", "tool": name }))
        }
    }
}
