//! MCP bridge — async helpers that call daemon REST endpoints to fetch
//! the same data the MCP tools expose. Used by Jarvis watchdog to enrich
//! system context with richer data (cost summaries, node readiness, etc.).
//!
//! All functions are fail-safe: network errors return None/empty so the
//! existing REST-based flow is never broken.

use serde_json::{json, Value};

/// Describes one MCP tool capability for the LLM prompt.
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
}

/// Fetch the list of MCP tool definitions from the daemon.
/// Returns tool names + descriptions so Jarvis knows what it can do.
pub async fn fetch_mcp_tools(_client: &reqwest::Client, _base: &str) -> Vec<McpToolInfo> {
    // The MCP tool catalogue is exposed at /api/plan-db/list, /api/ipc/agents,
    // etc. We build a static catalogue matching convergio-mcp's tool registry.
    // This avoids a circular dependency on the MCP crate.
    static_tool_catalogue()
}

/// Call an MCP-equivalent endpoint and return the JSON result.
/// Returns None on any error (timeout, 4xx, 5xx, parse failure).
pub async fn call_mcp_tool(
    client: &reqwest::Client,
    base: &str,
    tool_name: &str,
    args: &Value,
) -> Option<Value> {
    let result = match tool_name {
        "cvg_list_plans" => api_get(client, base, "/api/plan-db/list").await,
        "cvg_list_agents" => api_get(client, base, "/api/ipc/agents").await,
        "cvg_mesh_status" => api_get(client, base, "/api/mesh/peers").await,
        "cvg_kernel_status" => api_get(client, base, "/api/kernel/status").await,
        "cvg_cost_summary" => build_cost_summary(client, base).await,
        "cvg_node_readiness" => api_get(client, base, "/api/node/readiness").await,
        "cvg_get_plan" => {
            let plan_id = args.get("plan_id").and_then(|v| v.as_i64())?;
            api_get(client, base, &format!("/api/plan-db/json/{plan_id}")).await
        }
        _ => None,
    };
    result
}

/// Pick which MCP tools are relevant for a given intent.
pub fn relevant_tools_for_intent(intent: &str) -> Vec<&'static str> {
    match intent {
        "status" | "general" => vec![
            "cvg_kernel_status",
            "cvg_cost_summary",
            "cvg_node_readiness",
        ],
        "plan" | "code" => vec!["cvg_list_plans", "cvg_cost_summary"],
        "cost" => vec!["cvg_cost_summary"],
        "mesh" => vec!["cvg_mesh_status", "cvg_node_readiness"],
        "deploy" => vec!["cvg_node_readiness", "cvg_list_agents"],
        "night-agents" => vec!["cvg_list_agents"],
        _ => vec![],
    }
}

/// Format MCP tool results into a context block for the LLM prompt.
pub fn format_mcp_context(tool_name: &str, data: &Value) -> String {
    match tool_name {
        "cvg_cost_summary" => format_cost(data),
        "cvg_kernel_status" => format_kernel(data),
        "cvg_node_readiness" => format_readiness(data),
        _ => {
            // Generic: truncated JSON
            let s = serde_json::to_string(data).unwrap_or_default();
            let truncated = if s.len() > 300 { &s[..300] } else { &s };
            format!("[MCP:{tool_name}] {truncated}\n")
        }
    }
}

/// Format the tool catalogue as a string for the LLM system prompt.
pub fn format_tool_catalogue(tools: &[McpToolInfo]) -> String {
    if tools.is_empty() {
        return String::new();
    }
    let mut out = String::from("Available MCP tools:\n");
    for t in tools {
        out.push_str(&format!("  - {}: {}\n", t.name, t.description));
    }
    out
}

// ── Internal helpers ────────────────────────────────────────────────────────

async fn api_get(client: &reqwest::Client, base: &str, path: &str) -> Option<Value> {
    let resp = client
        .get(format!("{base}{path}"))
        .timeout(std::time::Duration::from_secs(3))
        .send()
        .await
        .ok()?;
    resp.json().await.ok()
}

async fn build_cost_summary(client: &reqwest::Client, base: &str) -> Option<Value> {
    let body = api_get(client, base, "/api/plan-db/list").await?;
    let plans = body
        .get("plans")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    let total_cost: f64 = plans
        .iter()
        .filter_map(|p| p.get("total_cost").and_then(|v| v.as_f64()))
        .sum();
    let active = plans
        .iter()
        .filter(|p| p.get("status").and_then(|s| s.as_str()) == Some("doing"))
        .count();
    Some(json!({
        "total_cost": total_cost,
        "active_plans": active,
        "total_plans": plans.len(),
    }))
}

fn format_cost(v: &Value) -> String {
    let total = v["total_cost"].as_f64().unwrap_or(0.0);
    let active = v["active_plans"].as_u64().unwrap_or(0);
    let total_plans = v["total_plans"].as_u64().unwrap_or(0);
    format!("Costs: ${total:.4} | Plans: {active} active / {total_plans} total\n")
}

fn format_kernel(v: &Value) -> String {
    let model = v["model"].as_str().unwrap_or("unknown");
    let uptime = v["uptime_secs"].as_u64().unwrap_or(0);
    let hours = uptime / 3600;
    let mins = (uptime % 3600) / 60;
    format!("Kernel: model={model}, uptime={hours}h{mins}m\n")
}

fn format_readiness(v: &Value) -> String {
    let ready = v["ready"].as_bool().unwrap_or(false);
    let checks = v["checks"].as_array();
    let total = checks.map(|a| a.len()).unwrap_or(0);
    let passed = checks
        .map(|a| a.iter().filter(|c| c["passed"] == true).count())
        .unwrap_or(0);
    let tag = if ready { "READY" } else { "NOT READY" };
    format!("Node: {tag} ({passed}/{total} checks passed)\n")
}

/// Static tool catalogue matching convergio-mcp tool definitions.
/// Kept in sync manually — only read-safe (Sandboxed/Community) tools.
fn static_tool_catalogue() -> Vec<McpToolInfo> {
    vec![
        McpToolInfo {
            name: "cvg_list_plans".into(),
            description: "List all plans with optional status filter".into(),
        },
        McpToolInfo {
            name: "cvg_get_plan".into(),
            description: "Get full plan details by ID".into(),
        },
        McpToolInfo {
            name: "cvg_list_agents".into(),
            description: "List registered agents with status".into(),
        },
        McpToolInfo {
            name: "cvg_mesh_status".into(),
            description: "Get peer topology and connections".into(),
        },
        McpToolInfo {
            name: "cvg_node_readiness".into(),
            description: "Run node health checks".into(),
        },
        McpToolInfo {
            name: "cvg_cost_summary".into(),
            description: "Spending overview: total cost, plans".into(),
        },
        McpToolInfo {
            name: "cvg_kernel_status".into(),
            description: "Kernel status: models loaded, uptime".into(),
        },
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_catalogue_not_empty() {
        let tools = static_tool_catalogue();
        assert!(!tools.is_empty());
        // All tools should have non-empty name and description
        for t in &tools {
            assert!(!t.name.is_empty());
            assert!(!t.description.is_empty());
        }
    }

    #[test]
    fn relevant_tools_status_has_entries() {
        let tools = relevant_tools_for_intent("status");
        assert!(!tools.is_empty());
    }

    #[test]
    fn relevant_tools_unknown_is_empty() {
        let tools = relevant_tools_for_intent("nonexistent-intent");
        assert!(tools.is_empty());
    }

    #[test]
    fn format_cost_output() {
        let v = serde_json::json!({"total_cost": 1.5, "active_plans": 2, "total_plans": 5});
        let out = format_cost(&v);
        assert!(out.contains("$1.5"));
        assert!(out.contains("2 active"));
        assert!(out.contains("5 total"));
    }

    #[test]
    fn format_kernel_output() {
        let v = serde_json::json!({"model": "qwen-7b", "uptime_secs": 7260});
        let out = format_kernel(&v);
        assert!(out.contains("qwen-7b"));
        assert!(out.contains("2h1m"));
    }

    #[test]
    fn format_readiness_ready() {
        let v = serde_json::json!({"ready": true, "checks": [{"passed": true}]});
        let out = format_readiness(&v);
        assert!(out.contains("READY"));
        assert!(out.contains("1/1"));
    }

    #[test]
    fn format_readiness_not_ready() {
        let v = serde_json::json!({"ready": false, "checks": []});
        let out = format_readiness(&v);
        assert!(out.contains("NOT READY"));
    }

    #[test]
    fn format_mcp_context_generic() {
        let v = serde_json::json!({"foo": "bar"});
        let out = format_mcp_context("cvg_unknown_tool", &v);
        assert!(out.contains("[MCP:cvg_unknown_tool]"));
    }

    #[test]
    fn format_tool_catalogue_output() {
        let tools = static_tool_catalogue();
        let out = format_tool_catalogue(&tools);
        assert!(out.contains("Available MCP tools:"));
        assert!(out.contains("cvg_list_plans"));
    }

    #[test]
    fn format_tool_catalogue_empty() {
        let out = format_tool_catalogue(&[]);
        assert!(out.is_empty());
    }
}
