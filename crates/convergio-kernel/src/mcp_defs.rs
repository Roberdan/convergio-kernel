//! MCP tool definitions for the kernel extension (Jarvis).

use convergio_types::extension::McpToolDef;
use serde_json::json;

pub fn kernel_tools() -> Vec<McpToolDef> {
    vec![
        McpToolDef {
            name: "cvg_kernel_status".into(),
            description: "Get kernel status: models loaded, uptime, readiness.".into(),
            method: "GET".into(),
            path: "/api/kernel/status".into(),
            input_schema: json!({"type": "object", "properties": {}}),
            min_ring: "community".into(),
            path_params: vec![],
        },
        McpToolDef {
            name: "cvg_kernel_ask".into(),
            description: "Ask the local LLM a question with platform context.".into(),
            method: "POST".into(),
            path: "/api/kernel/ask".into(),
            input_schema: json!({
                "type": "object",
                "properties": {"prompt": {"type": "string", "description": "Question to ask"}},
                "required": ["prompt"]
            }),
            min_ring: "trusted".into(),
            path_params: vec![],
        },
    ]
}
