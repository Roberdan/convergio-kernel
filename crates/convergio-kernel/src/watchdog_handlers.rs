//! Watchdog intent handlers — fetch real data from Convergio APIs.
//!
//! Every handler queries the daemon REST API and returns factual data.
//! The "general" handler builds context from multiple endpoints so the
//! model answers grounded in real system state — never hallucinated.
//! MCP tools are called additively to enrich context; failures are silent.

use serde_json::Value;

use crate::mcp_bridge;

/// Result from grounded inference, with optional cloud escalation flag.
pub struct InferenceResult {
    pub response: String,
    pub needs_cloud_escalation: bool,
}

/// Dispatch to the right handler based on classified intent.
pub async fn dispatch_intent(base: &str, intent: &str, text: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    match intent {
        "status" => fetch_status(&client, base).await,
        "plan" => fetch_plans(&client, base).await,
        "cost" => fetch_costs(&client, base).await,
        "help" => Ok(crate::watchdog::HELP_TEXT.to_string()),
        "code" | "escalate" => {
            let r = grounded_inference(&client, base, text, intent).await?;
            Ok(r.response)
        }
        _ => {
            let r = grounded_inference(&client, base, text, intent).await?;
            Ok(r.response)
        }
    }
}

/// Build a system-context string from live Convergio APIs.
/// When intent is provided, only fetches data relevant to that intent.
pub async fn fetch_system_context(
    client: &reqwest::Client,
    base: &str,
    intent: Option<&str>,
) -> String {
    let mut ctx = String::with_capacity(512);

    // Identity and version — always included to prevent hallucination
    let health_basic = api_get(client, base, "/api/health").await;
    let version = env!("CARGO_PKG_VERSION");
    ctx.push_str(&format!(
        "Convergio daemon v{version}\n\
         Timestamp: {}\n",
        health_basic["timestamp"].as_str().unwrap_or("unknown")
    ));

    // Always include health for baseline context
    let health = api_get(client, base, "/api/health/deep").await;
    ctx.push_str(&format_health(&health));

    // Smart context selection based on intent
    let needs_plans = matches!(intent, None | Some("plan" | "general" | "deploy" | "code"));
    let needs_agents = matches!(
        intent,
        None | Some("status" | "general" | "night-agents" | "deploy")
    );
    let needs_peers = matches!(intent, None | Some("mesh" | "status" | "general"));

    if needs_plans {
        let plans = api_get(client, base, "/api/plan-db/list").await;
        ctx.push_str(&format_plans(&plans));
    }
    if needs_agents {
        let agents = api_get(client, base, "/api/ipc/agents").await;
        ctx.push_str(&format_agents(&agents));
    }
    if needs_peers {
        let peers = api_get(client, base, "/api/mesh/peers").await;
        ctx.push_str(&format_peers(&peers));
    }

    // MCP enrichment: call relevant tools and append their output.
    // Failures are silently ignored — MCP is additive, never blocking.
    let mcp_tools = mcp_bridge::relevant_tools_for_intent(intent.unwrap_or("general"));
    for tool_name in mcp_tools {
        if let Some(data) =
            mcp_bridge::call_mcp_tool(client, base, tool_name, &serde_json::json!({})).await
        {
            ctx.push_str(&mcp_bridge::format_mcp_context(tool_name, &data));
        }
    }
    ctx
}

/// Topics that should be escalated to a cloud model.
const CLOUD_ESCALATION_KW: &[&str] = &[
    "architettura",
    "architecture",
    "security",
    "sicurezza",
    "planning",
    "pianificazione",
    "design",
];

/// Answer user question grounded in real system context.
/// Returns response text and a cloud escalation flag.
pub async fn grounded_inference(
    client: &reqwest::Client,
    base: &str,
    text: &str,
    intent: &str,
) -> Result<InferenceResult, String> {
    let context = fetch_system_context(client, base, Some(intent)).await;
    let lower = text.to_lowercase();
    let needs_cloud = CLOUD_ESCALATION_KW.iter().any(|kw| lower.contains(kw));

    // Include MCP tool catalogue so the LLM knows available capabilities
    let tools = mcp_bridge::fetch_mcp_tools(client, base).await;
    let tool_catalogue = mcp_bridge::format_tool_catalogue(&tools);

    let prompt = format!(
        "Sei Jarvis, l'assistente IA di Convergio. Personalita': efficiente, \
         leale, leggermente ironico — come il Jarvis di Tony Stark.\n\
         Rispondi preferibilmente in italiano, a meno che l'utente non parli inglese.\n\n\
         ## Dati di sistema (UNICA fonte di verita'):\n{context}\n\n\
         ## {tool_catalogue}\n\
         ## Domanda dell'utente:\n{text}\n\n\
         ## Regole:\n\
         - Rispondi in modo conciso e strutturato (usa bullet points se utile)\n\
         - Basa le risposte SOLO sui dati di sistema — MAI inventare\n\
         - NON inventare MAI versioni, date, numeri o nomi che non siano nei dati\n\
         - Se i dati non coprono la domanda, rispondi: 'Non ho questa informazione nei dati disponibili'\n\
         - Se serve un'azione, suggeriscila con il comando esatto"
    );
    let response = crate::watchdog::call_inference(base, &prompt, 500).await?;
    Ok(InferenceResult {
        response,
        needs_cloud_escalation: needs_cloud,
    })
}

/// Enriched status: health + active plans + agents + mesh.
pub async fn fetch_status(client: &reqwest::Client, base: &str) -> Result<String, String> {
    let ctx = fetch_system_context(client, base, Some("status")).await;
    Ok(format!("<b>System Status</b>\n{ctx}"))
}

pub async fn fetch_plans(client: &reqwest::Client, base: &str) -> Result<String, String> {
    let resp = api_get(client, base, "/api/plan-db/list").await;
    let plans = resp.as_array().or_else(|| resp["plans"].as_array());
    let count = plans.map(|a| a.len()).unwrap_or(0);
    let mut out = format!("<b>Plans</b> ({count} total)\n");
    if let Some(arr) = plans {
        for p in arr.iter().take(5) {
            let id = p["id"].as_i64().unwrap_or(0);
            let st = p["status"].as_str().unwrap_or("?");
            let nm = p["name"].as_str().unwrap_or("unnamed");
            out.push_str(&format!("  #{id} {nm} [{st}]\n"));
        }
    }
    Ok(out)
}

pub async fn fetch_costs(client: &reqwest::Client, base: &str) -> Result<String, String> {
    let resp = api_get(client, base, "/api/inference/costs").await;
    let total = resp["total_cost_usd"].as_f64().unwrap_or(0.0);
    Ok(format!("<b>Inference Costs</b>\nTotal: ${total:.4}"))
}

// ── helpers ──────────────────────────────────────────────

async fn api_get(client: &reqwest::Client, base: &str, path: &str) -> Value {
    let resp = client.get(format!("{base}{path}")).send().await;
    match resp {
        Ok(r) => r.json().await.unwrap_or(Value::Null),
        Err(_) => Value::Null,
    }
}

pub(crate) fn format_health(v: &Value) -> String {
    let comps = v["components"].as_array();
    let total = comps.map(|a| a.len()).unwrap_or(0);
    let ok = comps
        .map(|a| a.iter().filter(|c| c["status"] == "ok").count())
        .unwrap_or(0);
    format!("Health: {ok}/{total} components OK\n")
}

pub(crate) fn format_plans(v: &Value) -> String {
    // Response is array directly OR {"plans": [...]}
    let arr = v.as_array().or_else(|| v["plans"].as_array());
    let count = arr.map(|a| a.len()).unwrap_or(0);
    format!("Active plans: {count}\n")
}

pub(crate) fn format_agents(v: &Value) -> String {
    // Response is array directly
    let arr = v.as_array().or_else(|| v["agents"].as_array());
    let count = arr.map(|a| a.len()).unwrap_or(0);
    format!("Registered agents: {count}\n")
}

pub(crate) fn format_peers(v: &Value) -> String {
    // Response is array directly
    let arr = v.as_array().or_else(|| v["peers"].as_array());
    let count = arr.map(|a| a.len()).unwrap_or(0);
    format!("Mesh nodes: {count}\n")
}

// Tests in tests_watchdog.rs
