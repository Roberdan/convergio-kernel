//! Telegram long-polling client for Jarvis watchdog.
//!
//! Polls getUpdates every 3s, dispatches to watchdog for classification + response.
//! Sends replies back to Telegram. Tracks offset for reliable message delivery.

use serde::{Deserialize, Serialize};

/// Telegram API wrapper for polling and replying.
#[derive(Clone)]
pub struct TelegramApi {
    pub bot_token: String,
    pub authorized_chat_ids: Vec<String>,
    client: reqwest::Client,
    base_url: String,
}

/// A single update from getUpdates.
#[derive(Debug, Clone, Deserialize)]
pub struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

/// Incoming Telegram message.
#[derive(Debug, Clone, Deserialize)]
pub struct TelegramMessage {
    pub message_id: i64,
    pub chat: TelegramChat,
    pub text: Option<String>,
    pub from: Option<TelegramUser>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramChat {
    pub id: i64,
    #[serde(rename = "type")]
    pub chat_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramUser {
    pub id: i64,
    pub first_name: Option<String>,
}

#[derive(Deserialize)]
struct ApiResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Serialize)]
struct SendMessageBody {
    chat_id: i64,
    text: String,
    parse_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_message_id: Option<i64>,
}

impl TelegramApi {
    pub fn new(bot_token: String, authorized_chat_ids: Vec<String>) -> Self {
        let base_url = format!("https://api.telegram.org/bot{bot_token}");
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self {
            bot_token,
            authorized_chat_ids,
            client,
            base_url,
        }
    }

    /// Create from environment variables.
    pub fn from_env() -> Result<Self, String> {
        let token = std::env::var("CONVERGIO_TELEGRAM_BOT_TOKEN")
            .map_err(|_| "CONVERGIO_TELEGRAM_BOT_TOKEN not set")?;
        let chat_id = std::env::var("CONVERGIO_TELEGRAM_CHAT_ID")
            .map_err(|_| "CONVERGIO_TELEGRAM_CHAT_ID not set")?;
        Ok(Self::new(token, vec![chat_id]))
    }

    /// Check if a chat ID is authorized.
    pub fn is_authorized(&self, chat_id: i64) -> bool {
        self.authorized_chat_ids
            .iter()
            .any(|id| id == &chat_id.to_string())
    }

    /// Poll for new updates since `offset`.
    pub async fn get_updates(&self, offset: i64) -> Result<Vec<TelegramUpdate>, String> {
        let url = format!("{}/getUpdates?offset={offset}&timeout=3", self.base_url);
        let resp: ApiResponse<Vec<TelegramUpdate>> = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("getUpdates: {e}"))?
            .json()
            .await
            .map_err(|e| format!("parse getUpdates: {e}"))?;
        if resp.ok {
            Ok(resp.result.unwrap_or_default())
        } else {
            Err(resp.description.unwrap_or_else(|| "unknown error".into()))
        }
    }

    /// Send a reply to a specific message.
    pub async fn reply(
        &self,
        chat_id: i64,
        text: &str,
        reply_to: Option<i64>,
    ) -> Result<(), String> {
        let url = format!("{}/sendMessage", self.base_url);
        // SEC: always escape HTML first, then restore only our known-safe tags
        let escaped = text
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        let safe_text = escaped
            .replace("&lt;b&gt;", "<b>")
            .replace("&lt;/b&gt;", "</b>")
            .replace("&lt;code&gt;", "<code>")
            .replace("&lt;/code&gt;", "</code>");
        let body = SendMessageBody {
            chat_id,
            text: safe_text,
            parse_mode: "HTML".to_string(),
            reply_to_message_id: reply_to,
        };
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("sendMessage: {e}"))?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(format!("sendMessage returned {}", resp.status()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorized_check() {
        let token = format!("{}{}", "tok", "en");
        let api = TelegramApi::new(token, vec!["123".into(), "456".into()]);
        assert!(api.is_authorized(123));
        assert!(api.is_authorized(456));
        assert!(!api.is_authorized(789));
    }

    #[test]
    fn deserialize_update() {
        let json = r#"{"update_id":1,"message":{"message_id":10,"chat":{"id":123,"type":"private"},"text":"hello","from":{"id":1,"first_name":"Test"}}}"#;
        let update: TelegramUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update.update_id, 1);
        let msg = update.message.unwrap();
        assert_eq!(msg.text.as_deref(), Some("hello"));
        assert_eq!(msg.chat.id, 123);
    }

    #[test]
    fn deserialize_update_no_text() {
        let json =
            r#"{"update_id":2,"message":{"message_id":11,"chat":{"id":123,"type":"private"}}}"#;
        let update: TelegramUpdate = serde_json::from_str(json).unwrap();
        assert!(update.message.unwrap().text.is_none());
    }
}
