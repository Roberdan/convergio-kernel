//! Jarvis watchdog — Telegram message handler with MLX classification.
//!
//! Polls Telegram, classifies intent via keyword matching + local inference,
//! dispatches to handlers (in watchdog_handlers), replies, and logs.
//! Supports confidence scoring and cloud escalation hints.

use std::sync::Arc;
use tokio::sync::RwLock;

use convergio_db::pool::ConnPool;

use crate::telegram_poller::{TelegramApi, TelegramMessage};
use crate::watchdog_handlers;

/// Classification result with intent and confidence scoring.
#[derive(Debug, Clone, PartialEq)]
pub struct IntentClassification {
    pub intent: String,
    /// "high" for keyword match, "medium" for MLX inference.
    pub confidence: String,
    /// Confidence score (0.0 — 1.0).
    pub score: f64,
    /// Set when score < 0.5 — suggests escalation to cloud model.
    pub cloud_escalation_hint: Option<String>,
}

/// Watchdog configuration.
#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    pub daemon_url: String,
    pub poll_interval_secs: u64,
    pub model: String,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            daemon_url: "http://localhost:8420".into(),
            poll_interval_secs: 3,
            model: "mlx-community/Qwen2.5-Coder-7B-Instruct-4bit".into(),
        }
    }
}

/// Runtime state shared between poller loop and routes.
pub struct WatchdogState {
    pub running: bool,
    pub messages_handled: u64,
    pub last_error: Option<String>,
}

/// Start the Telegram watchdog polling loop.
pub async fn run_watchdog(pool: ConnPool, telegram: TelegramApi, config: WatchdogConfig) {
    let state = Arc::new(RwLock::new(WatchdogState {
        running: true,
        messages_handled: 0,
        last_error: None,
    }));
    let mut offset: i64 = 0;

    tracing::info!(
        "Jarvis watchdog started — polling every {}s",
        config.poll_interval_secs
    );

    loop {
        match telegram.get_updates(offset).await {
            Ok(updates) => {
                for update in updates {
                    offset = update.update_id + 1;
                    if let Some(msg) = update.message {
                        if !telegram.is_authorized(msg.chat.id) {
                            tracing::warn!(chat_id = msg.chat.id, "unauthorized chat");
                            continue;
                        }
                        if let Some(text) = &msg.text {
                            let res = handle_message(&pool, &telegram, &config, &msg, text).await;
                            let mut s = state.write().await;
                            s.messages_handled += 1;
                            if let Err(e) = res {
                                tracing::warn!(error = %e, "handle failed");
                                s.last_error = Some(e);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "poll failed");
                state.write().await.last_error = Some(e);
            }
        }
        let dur = std::time::Duration::from_secs(config.poll_interval_secs);
        tokio::time::sleep(dur).await;
    }
}

/// Handle a single message: classify -> dispatch -> reply -> log.
async fn handle_message(
    pool: &ConnPool,
    telegram: &TelegramApi,
    config: &WatchdogConfig,
    msg: &TelegramMessage,
    text: &str,
) -> Result<(), String> {
    let classification = classify_intent(config, text).await?;
    let base = &config.daemon_url;
    let response = watchdog_handlers::dispatch_intent(base, &classification.intent, text).await?;
    telegram
        .reply(msg.chat.id, &response, Some(msg.message_id))
        .await?;
    log_event(pool, &classification.intent, text);
    Ok(())
}

/// All valid intent categories.
pub const VALID_INTENTS: &[&str] = &[
    "status",
    "plan",
    "cost",
    "help",
    "code",
    "escalate",
    "general",
    "deploy",
    "night-agents",
    "mesh",
    "security",
    "config",
    "org",
];

/// Keyword-first classification with confidence scoring.
///
/// Keyword matches yield "high" confidence (1.0).
/// MLX inference yields "medium" confidence (0.6).
/// When score < 0.5, adds a cloud escalation hint.
pub async fn classify_intent(
    config: &WatchdogConfig,
    text: &str,
) -> Result<IntentClassification, String> {
    let lower = text.to_lowercase();
    if let Some(kw) = keyword_intent(&lower) {
        return Ok(IntentClassification {
            intent: kw.to_string(),
            confidence: "high".into(),
            score: 1.0,
            cloud_escalation_hint: None,
        });
    }
    let categories = VALID_INTENTS.join(", ");
    let prompt = format!(
        "Classify this message into exactly one category.\n\
         Categories: {categories}\n\
         Reply with ONLY the category name, nothing else.\n\n\
         Message: {text}"
    );
    let raw = call_inference(&config.daemon_url, &prompt, 10)
        .await?
        .trim()
        .to_lowercase();
    let (intent, score) = if VALID_INTENTS.contains(&raw.as_str()) {
        (raw, 0.6)
    } else {
        ("general".into(), 0.3)
    };
    let hint = if score < 0.5 {
        Some("Low confidence — consider cloud model escalation".into())
    } else {
        None
    };
    Ok(IntentClassification {
        intent,
        confidence: "medium".into(),
        score,
        cloud_escalation_hint: hint,
    })
}

/// Deterministic keyword matching for common intents.
pub(crate) fn keyword_intent(lower: &str) -> Option<&'static str> {
    const STATUS_KW: &[&str] = &["stato", "status", "come va", "salute", "health", "alive"];
    const PLAN_KW: &[&str] = &["piani", "plans", "plan", "tasks", "task"];
    const COST_KW: &[&str] = &["costi", "cost", "costs", "spending", "spesa"];
    const HELP_KW: &[&str] = &["/help", "help", "aiuto", "comandi"];
    const DEPLOY_KW: &[&str] = &["deploy", "rilascio", "release", "ship"];
    const NIGHT_KW: &[&str] = &["night-agent", "notturno", "overnight", "night agent"];
    const MESH_KW: &[&str] = &["mesh", "nodi", "peers", "cluster"];
    const SECURITY_KW: &[&str] = &["security", "sicurezza", "audit", "vulnerability"];
    const CONFIG_KW: &[&str] = &["config", "configurazione", "settings", "impostazioni"];
    const ORG_KW: &[&str] = &["org", "organisation", "organization", "progetto", "project"];

    // Specific intents first — avoids "mesh status" matching generic "status".
    let table: &[(&[&str], &str)] = &[
        (NIGHT_KW, "night-agents"),
        (MESH_KW, "mesh"),
        (SECURITY_KW, "security"),
        (DEPLOY_KW, "deploy"),
        (CONFIG_KW, "config"),
        (ORG_KW, "org"),
        (STATUS_KW, "status"),
        (PLAN_KW, "plan"),
        (COST_KW, "cost"),
        (HELP_KW, "help"),
    ];
    for (keywords, intent) in table {
        for kw in *keywords {
            if lower.contains(kw) {
                return Some(intent);
            }
        }
    }
    None
}

/// Send inference request to the daemon, extract content string.
pub(crate) async fn call_inference(
    base_url: &str,
    prompt: &str,
    max_tokens: u32,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();
    let resp = client
        .post(format!("{base_url}/api/inference/complete"))
        .json(&serde_json::json!({
            "prompt": prompt,
            "max_tokens": max_tokens,
            "agent_id": "kernel-watchdog",
            "constraints": {}
        }))
        .send()
        .await
        .map_err(|e| format!("inference call: {e}"))?;
    let body: serde_json::Value = resp.json().await.map_err(|e| format!("parse: {e}"))?;
    let content = body["response"]["content"]
        .as_str()
        .or_else(|| body["content"].as_str())
        .unwrap_or("No response from model.");
    Ok(content.to_string())
}

pub(crate) fn log_event(pool: &ConnPool, intent: &str, text: &str) {
    if let Ok(conn) = pool.get() {
        let truncated = crate::routes::truncate_utf8(text, 500);
        let _ = conn.execute(
            "INSERT INTO kernel_events \
             (severity, source, message, action_taken) \
             VALUES ('ok', 'telegram-watchdog', ?1, ?2)",
            rusqlite::params![truncated, intent],
        );
    }
}

pub(crate) const HELP_TEXT: &str = "\
<b>Jarvis Commands</b>\n\
<code>status</code> — system health + agents + mesh\n\
<code>plans</code> — list active plans\n\
<code>costs</code> — inference spending\n\
Or ask anything — I'll answer based on real system data.";

// Tests in tests_watchdog.rs
