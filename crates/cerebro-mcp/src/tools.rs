use serde_json::{json, Value};

/// Tool schema registry — 63 tools mirroring the Python MCP server.
/// Descriptions are verbatim from the Python mcp_server.py (agent-facing strings).
/// Step 8: remember, recall, associate, get_memory have full schemas.
/// Step 9: remaining tools will be filled in.
pub fn all_tool_schemas() -> Vec<Value> {
    TOOL_NAMES.iter().map(|&name| tool_schema(name)).collect()
}

fn tool_schema(name: &str) -> Value {
    match name {
        "remember" => json!({
            "name": "remember",
            "description": "Save information to long-term memory. Automatically detects duplicates, categorizes the content, and connects it to related memories. Use this to store facts, decisions, or anything worth remembering.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content":     { "type": "string", "description": "The memory content to store" },
                    "memory_type": {
                        "type": "string",
                        "enum": ["episodic","semantic","procedural","affective","prospective","schematic"],
                        "description": "Memory type (auto-classified if omitted)"
                    },
                    "tags":     {
                        "anyOf": [{"type":"array","items":{"type":"string"}},{"type":"string"}],
                        "description": "Tags for categorization"
                    },
                    "salience": { "type": "number", "description": "Importance 0-1 (auto-estimated if omitted)" },
                    "agent_id": { "type": "string", "description": "Agent storing this memory" },
                    "visibility": {
                        "type": "string",
                        "enum": ["private","shared","thread"],
                        "description": "Who can see this memory"
                    }
                },
                "required": ["content"]
            }
        }),

        "recall" => json!({
            "name": "recall",
            "description": "Search your memories by meaning, not just keywords. Returns the most relevant memories ranked by relevance, importance, and recency.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query":    { "type": "string", "description": "Search query text" },
                    "top_k":   { "type": "integer", "description": "Max results to return (default: 10)" },
                    "agent_id": { "type": "string", "description": "Filter to this agent's memories" }
                },
                "required": ["query"]
            }
        }),

        "associate" => json!({
            "name": "associate",
            "description": "Create a typed link between two existing memories. Strengthens the association graph for spreading activation during recall.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source_id": { "type": "string", "description": "ID of the source memory" },
                    "target_id": { "type": "string", "description": "ID of the target memory" },
                    "link_type": {
                        "type": "string",
                        "enum": ["temporal","causal","semantic","affective","contextual","contradicts","supports","derived_from","part_of"],
                        "description": "Relationship type (default: semantic)"
                    },
                    "weight": { "type": "number", "description": "Link strength 0-1 (default: 0.5)" },
                    "agent_id": { "type": "string", "description": "Agent creating this link" }
                },
                "required": ["source_id","target_id"]
            }
        }),

        "get_memory" => json!({
            "name": "get_memory",
            "description": "Retrieve a specific memory by ID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "memory_id": { "type": "string", "description": "The memory UUID" },
                    "agent_id":  { "type": "string", "description": "Agent scope for access control" }
                },
                "required": ["memory_id"]
            }
        }),

        _ => json!({
            "name": name,
            "description": format!("(stub) {name}"),
            "inputSchema": { "type": "object", "properties": {}, "required": [] }
        }),
    }
}

/// All 63 tool names — derived from Python mcp_server.py tool registry.
pub const TOOL_NAMES: &[&str] = &[
    "remember",
    "recall",
    "get_memory",
    "update_memory",
    "delete_memory",
    "associate",
    "memory_search",
    "memory_store",
    "memory_neighbors",
    "common_neighbors",
    "find_path",
    "check_near_duplicates",
    "session_save",
    "session_recall",
    "get_thread_memories",
    "prune_thread",
    "episode_start",
    "episode_add_step",
    "episode_end",
    "get_episode",
    "get_episode_memories",
    "list_episodes",
    "dream_run",
    "dream_status",
    "store_intention",
    "list_intentions",
    "resolve_intention",
    "store_procedure",
    "list_procedures",
    "find_relevant_procedures",
    "record_procedure_outcome",
    "emotional_summary",
    "activation_curve",
    "activation_heatmap",
    "activation_at_risk",
    "memory_health",
    "cortex_stats",
    "memory_graph_stats",
    "audit_summary",
    "query_audit",
    "list_tags",
    "delete_tag",
    "rename_tag",
    "merge_tags",
    "create_schema",
    "list_schemas",
    "find_matching_schemas",
    "get_schema_sources",
    "register_agent",
    "list_agents",
    "share_memory",
    "send_message",
    "check_inbox",
    "list_threads",
    "cognitive_bootstrap",
    "ingest_file",
    "describe_image",
    "search_vision",
    "export_memories",
    "list_deleted",
    "restore_memory",
    "purge_memory",
    "bulk_delete",
    "purge_all_deleted",
    "get_memory_versions",
    "restore_version",
];
