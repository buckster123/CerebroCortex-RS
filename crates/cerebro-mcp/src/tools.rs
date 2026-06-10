use serde_json::{json, Value};

/// Tool schema registry — 63 tools mirroring the Python MCP server.
/// Descriptions are verbatim from Python mcp_server.py (agent-facing strings).
/// TODO: populate in build-order step 9. Stubs return the tool name + empty schema.
pub fn all_tool_schemas() -> Vec<Value> {
    TOOL_NAMES
        .iter()
        .map(|name| json!({
            "name": name,
            "description": format!("(stub) {name}"),
            "inputSchema": { "type": "object", "properties": {}, "required": [] }
        }))
        .collect()
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
