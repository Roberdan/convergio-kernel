//! Watchdog HTTP routes for Jarvis Telegram integration.
//!
//! - GET  /api/kernel/watchdog       — watchdog status
//! - POST /api/kernel/telegram-test  — test Telegram connectivity
//! - POST /api/kernel/register-node  — register local node capabilities

use std::sync::Arc;

use axum::extract::State;
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use serde_json::{json, Value};

use crate::routes::KernelState;
use crate::telegram_poller::TelegramApi;

/// Build watchdog routes (merged into kernel router).
pub fn watchdog_routes(state: Arc<KernelState>) -> Router {
    Router::new()
        .route("/api/kernel/watchdog", get(handle_watchdog_status))
        .route("/api/kernel/telegram-test", post(handle_telegram_test))
        .route("/api/kernel/register-node", post(handle_register_node))
        .with_state(state)
}

async fn handle_watchdog_status(State(s): State<Arc<KernelState>>) -> Json<Value> {
    let telegram_configured = TelegramApi::from_env().is_ok();
    let conn = s.pool.get();
    let msg_count: i64 = conn
        .as_ref()
        .ok()
        .and_then(|c| {
            c.query_row(
                "SELECT COUNT(*) FROM kernel_events WHERE source = 'telegram-watchdog'",
                [],
                |r| r.get(0),
            )
            .ok()
        })
        .unwrap_or(0);
    Json(json!({
        "telegram_configured": telegram_configured,
        "messages_handled": msg_count,
        "model": "mlx-community/Qwen2.5-Coder-7B-Instruct-4bit",
    }))
}

async fn handle_telegram_test(State(_s): State<Arc<KernelState>>) -> Json<Value> {
    match TelegramApi::from_env() {
        Err(e) => Json(json!({"ok": false, "error": e})),
        Ok(api) => match api
            .reply(
                api.authorized_chat_ids[0].parse().unwrap_or(0),
                "<b>Jarvis</b> Telegram test OK",
                None,
            )
            .await
        {
            Ok(()) => Json(json!({"ok": true})),
            Err(e) => Json(json!({"ok": false, "error": e})),
        },
    }
}

async fn handle_register_node(State(_s): State<Arc<KernelState>>) -> Json<Value> {
    let daemon_url =
        std::env::var("CONVERGIO_DAEMON_URL").unwrap_or_else(|_| "http://localhost:8420".into());
    let node_name = std::env::var("CONVERGIO_NODE_NAME")
        .unwrap_or_else(|_| hostname().unwrap_or_else(|| "m1-pro".into()));
    let caps = serde_json::json!({
        "capabilities": [
            {"name": "llm-inference", "version": "1.0.0", "tags": ["inference", "gpu"],
             "metadata": {"provider": "mlx", "model": "Qwen2.5-Coder-7B-Instruct-4bit"}},
            {"name": "voice-pipeline", "version": "1.0.0", "tags": ["voice"],
             "metadata": {"tts": true, "stt": true}},
            {"name": "telegram-watchdog", "version": "1.0.0", "tags": ["voice", "low_latency"],
             "metadata": {"classify_latency_ms": 580, "code_latency_ms": 1300}},
        ]
    });
    let client = reqwest::Client::new();
    match client
        .post(format!("{daemon_url}/api/mesh/capabilities/register"))
        .query(&[("peer_name", &node_name)])
        .json(&caps)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            Json(json!({"ok": true, "node": node_name, "capabilities": 3}))
        }
        Ok(resp) => Json(json!({"ok": false, "error": format!("status {}", resp.status())})),
        Err(e) => Json(json!({"ok": false, "error": e.to_string()})),
    }
}

fn hostname() -> Option<String> {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostname_returns_something() {
        assert!(hostname().is_some());
    }
}
