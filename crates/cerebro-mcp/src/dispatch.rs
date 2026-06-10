use std::sync::Arc;

use cerebro::{
    models::AssociativeLink,
    types::{AgentId, LinkType, MemoryId, MemoryType, VisibilityScope},
    CerebroCortex,
};
use serde_json::{json, Value};

use crate::tools;

pub fn handle_initialize(req: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": req["id"],
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

pub fn tools_list(req: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": req["id"],
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
        Ok(v)  => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "content": [{ "type": "text", "text": v.to_string() }] }
        }),
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32603, "message": e.to_string() }
        }),
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
// Tool routing
// ---------------------------------------------------------------------------

async fn route(name: &str, args: &Value, brain: Arc<CerebroCortex>) -> anyhow::Result<Value> {
    match name {
        "remember" => {
            let content = args["content"].as_str()
                .ok_or_else(|| anyhow::anyhow!("content is required"))?.to_string();
            let memory_type: Option<MemoryType> =
                serde_json::from_value(args["memory_type"].clone()).ok();
            let tags: Option<Vec<String>> = args["tags"].as_array().map(|arr| {
                arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
            });
            let salience = args["salience"].as_f64().map(|f| f as f32);
            let scope    = agent_scope(args);
            let node = brain.remember(content, memory_type, tags, salience, scope).await?;
            Ok(serde_json::to_value(&node)?)
        }

        "recall" => {
            let query = args["query"].as_str()
                .ok_or_else(|| anyhow::anyhow!("query is required"))?;
            let k     = args["top_k"].as_u64().unwrap_or(10) as usize;
            let scope = agent_scope(args);
            let results = brain.recall(query, k, scope).await?;
            let out: Vec<Value> = results.into_iter()
                .map(|(node, score)| json!({ "memory": node, "score": score }))
                .collect();
            Ok(json!(out))
        }

        "associate" => {
            let src = args["source_id"].as_str()
                .ok_or_else(|| anyhow::anyhow!("source_id is required"))?.to_string();
            let tgt = args["target_id"].as_str()
                .ok_or_else(|| anyhow::anyhow!("target_id is required"))?.to_string();
            let link_type: LinkType =
                serde_json::from_value(args["link_type"].clone()).unwrap_or(LinkType::Semantic);
            let weight = args["weight"].as_f64().unwrap_or(0.5) as f32;
            let link = AssociativeLink::new(
                MemoryId(src.clone()), MemoryId(tgt.clone()), link_type, weight,
            );
            brain.associate(MemoryId(src), MemoryId(tgt), link).await?;
            Ok(json!({ "status": "ok" }))
        }

        "get_memory" => {
            let id = args["memory_id"].as_str()
                .ok_or_else(|| anyhow::anyhow!("memory_id is required"))?;
            let scope = agent_scope(args);
            let node  = brain.storage.read().await
                .sqlite.get_memory(&MemoryId(id.to_string()), &scope).await?;
            match node {
                Some(n) => Ok(serde_json::to_value(&n)?),
                None    => Err(anyhow::anyhow!("memory not found: {id}")),
            }
        }

        "delete_memory" => {
            let id = args["memory_id"].as_str()
                .ok_or_else(|| anyhow::anyhow!("memory_id is required"))?;
            let deleted = brain.storage.read().await
                .sqlite.delete_memory(&MemoryId(id.to_string())).await?;
            Ok(json!({ "deleted": deleted }))
        }

        "update_memory" => {
            let id = args["memory_id"].as_str()
                .ok_or_else(|| anyhow::anyhow!("memory_id is required"))?;
            let scope = agent_scope(args);
            let storage = brain.storage.read().await;
            let mut node = storage.sqlite.get_memory(&MemoryId(id.to_string()), &scope).await?
                .ok_or_else(|| anyhow::anyhow!("memory not found: {id}"))?;
            drop(storage);

            let content_changed = args["content"].as_str().is_some();
            if let Some(c) = args["content"].as_str()  { node.content = c.to_string(); }
            if let Some(s) = args["salience"].as_f64()  { node.salience = s as f32; }
            if let Some(arr) = args["tags"].as_array() {
                node.tags = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
            }

            let storage = brain.storage.read().await;
            storage.sqlite.update_memory(&node).await?;
            if content_changed {
                storage.vector.embed_and_store(&node.id, &node.content).await?;
            }
            Ok(serde_json::to_value(&node)?)
        }

        // Aliases — same underlying logic, different param names
        "memory_store" | "memory_search" => {
            if name == "memory_store" {
                let content = args["content"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("content is required"))?.to_string();
                let scope = agent_scope(args);
                let node = brain.remember(content, None, None, None, scope).await?;
                Ok(serde_json::to_value(&node)?)
            } else {
                let query = args["query"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("query is required"))?;
                let k     = args["top_k"].as_u64().unwrap_or(10) as usize;
                let scope = agent_scope(args);
                let results = brain.recall(query, k, scope).await?;
                let out: Vec<Value> = results.into_iter()
                    .map(|(node, score)| json!({ "memory": node, "score": score }))
                    .collect();
                Ok(json!(out))
            }
        }

        "memory_neighbors" => {
            let id    = args["memory_id"].as_str()
                .ok_or_else(|| anyhow::anyhow!("memory_id is required"))?;
            let scope = agent_scope(args);
            let storage = brain.storage.read().await;
            let neighbor_ids: Vec<MemoryId> = storage.graph
                .neighbors(&MemoryId(id.to_string()))
                .into_iter().map(|id| id.clone()).collect();
            let nodes = storage.sqlite.get_memories_by_ids(&neighbor_ids, &scope).await?;
            Ok(serde_json::to_value(&nodes)?)
        }

        "find_path" => {
            let src = args["source_id"].as_str()
                .ok_or_else(|| anyhow::anyhow!("source_id is required"))?;
            let tgt = args["target_id"].as_str()
                .ok_or_else(|| anyhow::anyhow!("target_id is required"))?;
            let storage = brain.storage.read().await;
            let path = brain.association.find_path(
                &storage.graph, &MemoryId(src.to_string()), &MemoryId(tgt.to_string()),
            );
            match path {
                Some(ids) => Ok(json!({ "found": true, "path": ids })),
                None      => Ok(json!({ "found": false, "path": [] })),
            }
        }

        "common_neighbors" => {
            let a = args["memory_id_a"].as_str()
                .ok_or_else(|| anyhow::anyhow!("memory_id_a is required"))?;
            let b = args["memory_id_b"].as_str()
                .ok_or_else(|| anyhow::anyhow!("memory_id_b is required"))?;
            let scope   = agent_scope(args);
            let storage = brain.storage.read().await;
            let common  = brain.association.get_common_neighbors(
                &storage.graph, &MemoryId(a.to_string()), &MemoryId(b.to_string()),
            );
            let ids: Vec<MemoryId> = common;
            let nodes = storage.sqlite.get_memories_by_ids(&ids, &scope).await?;
            Ok(serde_json::to_value(&nodes)?)
        }

        "cortex_stats" => {
            let stats = brain.storage.read().await.sqlite.memory_stats().await?;
            Ok(stats)
        }

        "memory_graph_stats" => {
            let storage = brain.storage.read().await;
            Ok(json!({
                "node_count": storage.graph.graph.node_count(),
                "edge_count": storage.graph.graph.edge_count(),
            }))
        }

        _ => Ok(json!({ "status": "not_yet_implemented", "tool": name })),
    }
}

// ---------------------------------------------------------------------------
// Helper: build a VisibilityScope from an agent_id argument
// ---------------------------------------------------------------------------

fn agent_scope(args: &Value) -> VisibilityScope {
    match args["agent_id"].as_str() {
        Some(id) if !id.is_empty() => VisibilityScope::for_agent(AgentId(id.to_string())),
        _ => VisibilityScope::global(),
    }
}

// ---------------------------------------------------------------------------
// Tests — dispatch logic without stdio (no actual MCP session required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cerebro::config::Config;
    use tempfile::TempDir;

    async fn make_brain() -> (Arc<CerebroCortex>, TempDir) {
        let dir = TempDir::new().unwrap();
        let config = Config {
            db_path:       dir.path().join("test.db"),
            anthropic_key: None,
            embed_model:   "".into(),
        };
        let brain = Arc::new(CerebroCortex::new(config).await.unwrap());
        (brain, dir)
    }

    #[test]
    fn initialize_returns_capabilities_with_echoed_id() {
        let req  = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}});
        let resp = handle_initialize(&req);
        assert_eq!(resp["id"], 1, "id must be echoed");
        assert!(resp["result"]["capabilities"]["tools"].is_object());
        assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(resp["result"]["serverInfo"]["name"], "cerebro-mcp");
    }

    #[test]
    fn tools_list_echoes_id_and_contains_63_tools() {
        let req  = json!({"jsonrpc":"2.0","id":42,"method":"tools/list","params":{}});
        let resp = tools_list(&req);
        assert_eq!(resp["id"], 42, "id must be echoed");
        let tools = resp["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 66);
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert!(names.contains(&"remember"));
        assert!(names.contains(&"recall"));
    }

    #[test]
    fn tools_list_remember_has_proper_schema() {
        let req  = json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}});
        let resp = tools_list(&req);
        let tools = resp["result"]["tools"].as_array().unwrap();
        let remember = tools.iter().find(|t| t["name"] == "remember").unwrap();
        let schema = &remember["inputSchema"];
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["content"].is_object(), "content property must exist");
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "content"), "content must be required");
    }

    #[tokio::test]
    async fn dispatch_remember_stores_and_returns_node() {
        let (brain, _dir) = make_brain().await;
        let msg = json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {
                "name": "remember",
                "arguments": { "content": "Rust is a memory-safe systems language" }
            }
        });
        let resp = dispatch_tool(msg, brain).await;
        assert!(resp["error"].is_null(), "unexpected error: {}", resp["error"]);
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        let node: Value = serde_json::from_str(text).unwrap();
        assert!(node["id"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(node["salience"].as_f64().is_some_and(|s| s > 0.0));
    }

    #[tokio::test]
    async fn dispatch_remember_rejects_short_content() {
        let (brain, _dir) = make_brain().await;
        let msg = json!({
            "jsonrpc":"2.0","id":4,"method":"tools/call",
            "params":{"name":"remember","arguments":{"content":"hi"}}
        });
        let resp = dispatch_tool(msg, brain).await;
        assert!(!resp["error"].is_null(), "short content should produce an error");
    }

    #[tokio::test]
    async fn dispatch_recall_returns_remembered_node_at_top() {
        let (brain, _dir) = make_brain().await;

        // Store first
        let store_msg = json!({
            "jsonrpc":"2.0","id":5,"method":"tools/call",
            "params":{"name":"remember","arguments":{
                "content": "sqlite fts5 full text search is the keyword fallback path"
            }}
        });
        let store_resp = dispatch_tool(store_msg, Arc::clone(&brain)).await;
        assert!(store_resp["error"].is_null());
        let text = store_resp["result"]["content"][0]["text"].as_str().unwrap();
        let stored: Value = serde_json::from_str(text).unwrap();
        let stored_id = stored["id"].as_str().unwrap();

        // Recall
        let recall_msg = json!({
            "jsonrpc":"2.0","id":6,"method":"tools/call",
            "params":{"name":"recall","arguments":{"query":"sqlite fts5 keyword search","top_k":5}}
        });
        let recall_resp = dispatch_tool(recall_msg, brain).await;
        assert!(recall_resp["error"].is_null());
        let text = recall_resp["result"]["content"][0]["text"].as_str().unwrap();
        let results: Vec<Value> = serde_json::from_str(text).unwrap();
        assert!(!results.is_empty(), "should return at least one result");
        assert_eq!(results[0]["memory"]["id"].as_str().unwrap(), stored_id,
            "stored memory should rank first");
    }

    #[tokio::test]
    async fn dispatch_associate_creates_link() {
        let (brain, _dir) = make_brain().await;

        let store = |content: &'static str, brain: Arc<CerebroCortex>| async move {
            let msg = json!({
                "jsonrpc":"2.0","id":0,"method":"tools/call",
                "params":{"name":"remember","arguments":{"content":content}}
            });
            let resp = dispatch_tool(msg, brain).await;
            let text = resp["result"]["content"][0]["text"].as_str().unwrap().to_string();
            let node: Value = serde_json::from_str(&text).unwrap();
            node["id"].as_str().unwrap().to_string()
        };

        let a_id = store("Rust ownership model prevents memory leaks at compile time", Arc::clone(&brain)).await;
        let b_id = store("C++ uses RAII for deterministic resource management patterns", Arc::clone(&brain)).await;

        let assoc_msg = json!({
            "jsonrpc":"2.0","id":7,"method":"tools/call",
            "params":{"name":"associate","arguments":{
                "source_id": a_id,
                "target_id": b_id,
                "link_type": "semantic",
                "weight": 0.8
            }}
        });
        let resp = dispatch_tool(assoc_msg, Arc::clone(&brain)).await;
        assert!(resp["error"].is_null(), "associate should not error: {}", resp["error"]);
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        let result: Value = serde_json::from_str(text).unwrap();
        assert_eq!(result["status"], "ok");

        // Verify edge in graph
        let storage = brain.storage.read().await;
        let neighbors = storage.graph.neighbors(&MemoryId(a_id));
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0], &MemoryId(b_id));
    }

    #[tokio::test]
    async fn dispatch_stub_tool_returns_not_implemented() {
        let (brain, _dir) = make_brain().await;
        let msg = json!({
            "jsonrpc":"2.0","id":8,"method":"tools/call",
            "params":{"name":"dream_run","arguments":{}}
        });
        let resp = dispatch_tool(msg, brain).await;
        // Stub tools return a result (not an error), with not_yet_implemented status
        assert!(resp["error"].is_null(), "stub should not produce a protocol error");
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        let result: Value = serde_json::from_str(text).unwrap();
        assert_eq!(result["status"], "not_yet_implemented");
    }
}
