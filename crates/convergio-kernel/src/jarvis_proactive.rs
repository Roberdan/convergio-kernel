//! Jarvis proactive capabilities — morning summaries, alert forwarding,
//! and runtime configuration for proactive features.

use std::sync::Arc;

use axum::extract::State;
use axum::response::Json;
use axum::routing::post;
use axum::Router;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;

/// Runtime configuration for Jarvis proactive features.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JarvisConfig {
    /// Enable daily morning summary generation.
    pub morning_summary_enabled: bool,
    /// Enable critical alert checking and forwarding.
    pub alerts_enabled: bool,
    /// Enable proactive suggestions based on system state.
    pub proactive_suggestions_enabled: bool,
}

impl Default for JarvisConfig {
    fn default() -> Self {
        Self {
            morning_summary_enabled: true,
            alerts_enabled: true,
            proactive_suggestions_enabled: false,
        }
    }
}

/// Shared state for Jarvis proactive features.
pub struct JarvisProactiveState {
    pub config: RwLock<JarvisConfig>,
    pub daemon_url: String,
}

/// Generate a daily morning summary: plan progress, agent health, overnight events.
pub async fn morning_summary(daemon_url: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let health = crate::watchdog_handlers::fetch_system_context(&client, daemon_url, None).await;

    let events = fetch_recent_events(&client, daemon_url).await;
    let mut summary = String::with_capacity(512);
    summary.push_str("Buongiorno! Ecco il riepilogo:\n\n");
    summary.push_str(&health);
    summary.push('\n');
    if events.is_empty() {
        summary.push_str("Nessun evento critico durante la notte.\n");
    } else {
        summary.push_str("Eventi recenti:\n");
        for ev in events.iter().take(10) {
            let sev = ev["severity"].as_str().unwrap_or("?");
            let msg = ev["message"].as_str().unwrap_or("—");
            summary.push_str(&format!("  [{sev}] {msg}\n"));
        }
    }
    Ok(summary)
}

/// Check for critical events that should be forwarded to the user.
pub async fn check_alerts(daemon_url: &str) -> Vec<AlertItem> {
    let client = reqwest::Client::new();
    let events = fetch_recent_events(&client, daemon_url).await;
    events
        .iter()
        .filter(|ev| {
            ev["severity"].as_str() == Some("critical") || ev["severity"].as_str() == Some("warn")
        })
        .map(|ev| AlertItem {
            severity: ev["severity"].as_str().unwrap_or("unknown").to_string(),
            message: ev["message"].as_str().unwrap_or("no message").to_string(),
            source: ev["source"].as_str().unwrap_or("unknown").to_string(),
        })
        .collect()
}

/// A single alert item for forwarding.
#[derive(Debug, Clone, Serialize)]
pub struct AlertItem {
    pub severity: String,
    pub message: String,
    pub source: String,
}

/// Fetch recent kernel events from the daemon API.
async fn fetch_recent_events(client: &reqwest::Client, base: &str) -> Vec<Value> {
    let url = format!("{base}/api/kernel/events?limit=20");
    let resp = client.get(&url).send().await;
    match resp {
        Ok(r) => {
            let body: Value = r.json().await.unwrap_or(Value::Null);
            body["events"].as_array().cloned().unwrap_or_default()
        }
        Err(_) => vec![],
    }
}

/// Build the Jarvis proactive config route.
pub fn jarvis_proactive_routes(state: Arc<JarvisProactiveState>) -> Router {
    Router::new()
        .route(
            "/api/kernel/jarvis/config",
            post(handle_update_config).get(handle_get_config),
        )
        .with_state(state)
}

async fn handle_update_config(
    State(s): State<Arc<JarvisProactiveState>>,
    Json(body): Json<JarvisConfig>,
) -> Json<Value> {
    let mut cfg = s.config.write().await;
    *cfg = body.clone();
    tracing::info!("Jarvis proactive config updated: {body:?}");
    Json(json!({"ok": true, "config": body}))
}

async fn handle_get_config(State(s): State<Arc<JarvisProactiveState>>) -> Json<Value> {
    let cfg = s.config.read().await;
    Json(json!({"config": *cfg}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_enables_summary_and_alerts() {
        let cfg = JarvisConfig::default();
        assert!(cfg.morning_summary_enabled);
        assert!(cfg.alerts_enabled);
        assert!(!cfg.proactive_suggestions_enabled);
    }

    #[test]
    fn config_roundtrip_json() {
        let cfg = JarvisConfig {
            morning_summary_enabled: false,
            alerts_enabled: true,
            proactive_suggestions_enabled: true,
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: JarvisConfig = serde_json::from_str(&json).unwrap();
        assert!(!back.morning_summary_enabled);
        assert!(back.proactive_suggestions_enabled);
    }

    #[test]
    fn alert_item_serializes() {
        let alert = AlertItem {
            severity: "critical".into(),
            message: "db down".into(),
            source: "monitor".into(),
        };
        let v = serde_json::to_value(&alert).unwrap();
        assert_eq!(v["severity"], "critical");
    }
}
