//! convergio-kernel — Jarvis: local LLM + health monitoring + evidence gate.
//!
//! Extension: pluggable, feature-gated. Provides local inference (Qwen via MLX),
//! health monitoring (30s cycle), evidence gate (Article VI), voice routing,
//! Telegram notifications, and deterministic recovery.
//!
//! DB tables: kernel_events, kernel_verifications, kernel_config, knowledge_base.

pub mod engine;
pub mod ext;
pub mod jarvis_proactive;
pub mod kernel_ask_routes;
pub mod mcp_bridge;
pub mod mcp_defs;
pub mod monitor;
pub mod recover;
pub mod routes;
pub mod routes_watchdog;
pub mod telegram_poller;
pub mod types;
pub mod verify;
pub mod watchdog;
pub mod watchdog_handlers;

pub use ext::KernelExtension;
pub use types::{
    EvidenceCheck, EvidenceReport, InferenceLevel, KernelAction, KernelCheckResult, KernelConfig,
    KernelSeverity, KernelStatus, VoiceIntent,
};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_watchdog;
