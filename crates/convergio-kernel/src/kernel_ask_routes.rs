//! HTTP handlers for org-scoped intent classification and grounded inference.
//!
//! - POST /api/kernel/classify-intent — classify user intent for org questions
//! - POST /api/kernel/grounded-infer  — grounded inference against org context

use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::response::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::routes::{truncate_utf8, KernelState};
use crate::watchdog::call_inference;

/// Intent categories for org-scoped questions.
const ORG_INTENT_CATEGORIES: &[&str] = &["factual", "analytical", "escalation"];

#[derive(Deserialize)]
pub struct ClassifyIntentBody {
    pub question: String,
    #[serde(default)]
    pub context_hint: Option<String>,
}

/// POST /api/kernel/classify-intent
///
/// Classifies the user intent for an org question.
/// Returns one of: "factual", "analytical", "escalation" with a confidence score.
pub async fn handle_classify_intent(
    State(s): State<Arc<KernelState>>,
    Json(body): Json<ClassifyIntentBody>,
) -> Json<Value> {
    let lower = body.question.to_lowercase();

    // Keyword-first classification
    if let Some(intent) = keyword_classify_intent(&lower) {
        return Json(json!({
            "intent": intent,
            "confidence": 1.0,
        }));
    }

    // Inference fallback
    let base = "http://localhost:8420";
    let context_line = body
        .context_hint
        .as_deref()
        .map(|h| format!("\nContext hint: {h}"))
        .unwrap_or_default();
    let categories = ORG_INTENT_CATEGORIES.join(", ");
    let prompt = format!(
        "Classify this question into exactly one category.\n\
         Categories: {categories}\n\
         - factual: asks for a specific fact, status, or data point\n\
         - analytical: requires reasoning, trends, or multi-step analysis\n\
         - escalation: ambiguous, risky, or requires human review\n\
         Reply with ONLY the category name, nothing else.{context_line}\n\n\
         Question: {}",
        body.question
    );

    let (intent, confidence) = match call_inference(base, &prompt, 10).await {
        Ok(raw) => {
            let trimmed = raw.trim().to_lowercase();
            if ORG_INTENT_CATEGORIES.contains(&trimmed.as_str()) {
                (trimmed, 0.6_f64)
            } else {
                ("escalation".to_string(), 0.3_f64)
            }
        }
        Err(_) => ("escalation".to_string(), 0.0_f64),
    };

    // Log to kernel_events (truncate to prevent oversized rows)
    if let Ok(conn) = s.pool.get() {
        let _ = conn.execute(
            "INSERT INTO kernel_events (severity, source, message, action_taken) \
             VALUES ('ok', 'classify-intent', ?1, ?2)",
            rusqlite::params![truncate_utf8(&body.question, 500), intent],
        );
    }

    Json(json!({
        "intent": intent,
        "confidence": confidence,
    }))
}

/// Deterministic keyword matching for org intent categories.
fn keyword_classify_intent(lower: &str) -> Option<&'static str> {
    const FACTUAL_KW: &[&str] = &[
        "what is", "show me", "list", "how many", "count", "status", "state", "current", "latest",
        "last", "get",
    ];
    const ANALYTICAL_KW: &[&str] = &[
        "why", "how", "analyze", "compare", "trend", "reason", "explain", "impact", "effect",
    ];
    const ESCALATION_KW: &[&str] = &[
        "delete", "remove", "drop", "destroy", "override", "force", "bypass", "disable", "shutdown",
    ];

    for kw in ESCALATION_KW {
        if lower.contains(kw) {
            return Some("escalation");
        }
    }
    for kw in ANALYTICAL_KW {
        if lower.contains(kw) {
            return Some("analytical");
        }
    }
    for kw in FACTUAL_KW {
        if lower.contains(kw) {
            return Some("factual");
        }
    }
    None
}

#[derive(Deserialize)]
pub struct GroundedInferBody {
    pub question: String,
    pub context: String,
}

/// POST /api/kernel/grounded-infer
///
/// Runs grounded inference: answers a question using provided org context.
/// Returns `{ answer, agent, latency_ms }`.
pub async fn handle_grounded_infer(
    State(s): State<Arc<KernelState>>,
    Json(body): Json<GroundedInferBody>,
) -> Json<Value> {
    let start = Instant::now();
    let base = "http://localhost:8420";
    let prompt = format!(
        "You are Jarvis, a system assistant. Answer the question using ONLY the context below.\n\
         If the answer is not in the context, reply: \"I don't have enough information.\"\n\n\
         Context:\n{}\n\n\
         Question: {}\n\n\
         Answer:",
        body.context, body.question
    );

    let answer = match call_inference(base, &prompt, 200).await {
        Ok(a) => a.trim().to_string(),
        Err(e) => format!("Inference error: {e}"),
    };
    let latency_ms = start.elapsed().as_millis() as u64;

    // Log to kernel_events (truncate to prevent oversized rows)
    if let Ok(conn) = s.pool.get() {
        let _ = conn.execute(
            "INSERT INTO kernel_events (severity, source, message, action_taken) \
             VALUES ('ok', 'grounded-infer', ?1, 'answered')",
            rusqlite::params![truncate_utf8(&body.question, 500)],
        );
    }

    Json(json!({
        "answer": answer,
        "agent": "jarvis-mlx",
        "latency_ms": latency_ms,
    }))
}

#[derive(Deserialize)]
pub struct AgentAskBody {
    pub agent: String,
    pub message: String,
}

/// POST /api/kernel/agent-ask
///
/// Routes a question to a named agent, using local inference to produce a
/// reply. Returns `{ ok, reply: { from, content } }` matching the CLI's
/// expected `parse_response` shape.
pub async fn handle_agent_ask(
    State(s): State<Arc<KernelState>>,
    Json(body): Json<AgentAskBody>,
) -> Json<Value> {
    let base = "http://localhost:8420";
    let prompt = format!(
        "You are {agent}, an AI assistant in the Convergio platform. \
         Answer the following question concisely and helpfully.\n\n\
         Question: {message}\n\nAnswer:",
        agent = body.agent,
        message = body.message,
    );

    let content = match call_inference(base, &prompt, 200).await {
        Ok(answer) => answer.trim().to_string(),
        Err(e) => format!("Inference error: {e}"),
    };

    if let Ok(conn) = s.pool.get() {
        let _ = conn.execute(
            "INSERT INTO kernel_events (severity, source, message, action_taken) \
             VALUES ('ok', 'agent-ask', ?1, ?2)",
            rusqlite::params![
                truncate_utf8(&body.message, 500),
                truncate_utf8(&body.agent, 100)
            ],
        );
    }

    Json(json!({
        "ok": true,
        "reply": {
            "from": body.agent,
            "content": content,
        }
    }))
}
