//! Extension trait implementation for convergio-kernel (Jarvis).

use std::sync::Arc;

use convergio_db::pool::ConnPool;
use convergio_types::extension::{
    AppContext, Extension, Health, McpToolDef, Metric, Migration, ScheduledTask,
};
use convergio_types::manifest::{Capability, Dependency, Manifest, ModuleKind};
use tokio::sync::RwLock;

use crate::engine::KernelEngine;
use crate::jarvis_proactive::{jarvis_proactive_routes, JarvisConfig, JarvisProactiveState};
use crate::routes::{kernel_routes, KernelState};
use crate::routes_watchdog::watchdog_routes;
use crate::telegram_poller::TelegramApi;
use crate::watchdog::{run_watchdog, WatchdogConfig};

/// Kernel extension — Jarvis: local LLM assistant with monitoring and recovery.
pub struct KernelExtension {
    pool: ConnPool,
    #[allow(dead_code)]
    engine: Arc<RwLock<KernelEngine>>,
}

impl KernelExtension {
    pub fn new(pool: ConnPool) -> Self {
        Self {
            pool,
            engine: Arc::new(RwLock::new(KernelEngine::default())),
        }
    }

    fn state(&self) -> Arc<KernelState> {
        Arc::new(KernelState {
            pool: self.pool.clone(),
            engine: RwLock::new(KernelEngine::default()),
        })
    }
}

impl Extension for KernelExtension {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "convergio-kernel".to_string(),
            description: "Jarvis — local LLM inference, health monitoring, evidence gate, \
                          voice routing, Telegram notifications"
                .to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            kind: ModuleKind::Extension,
            provides: vec![
                Capability {
                    name: "local-inference".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Local LLM inference via MLX (Qwen)".to_string(),
                },
                Capability {
                    name: "health-monitoring".to_string(),
                    version: "1.0.0".to_string(),
                    description: "30s health check cycle with recovery".to_string(),
                },
                Capability {
                    name: "evidence-gate".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Task completion verification (Article VI)".to_string(),
                },
                Capability {
                    name: "voice-routing".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Voice intent classification and routing".to_string(),
                },
                Capability {
                    name: "telegram-watchdog".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Telegram polling + MLX classification + auto-reply".to_string(),
                },
            ],
            requires: vec![
                Dependency {
                    capability: "db-pool".to_string(),
                    version_req: ">=1.0.0".to_string(),
                    required: true,
                },
                Dependency {
                    capability: "ipc-bus".to_string(),
                    version_req: ">=1.0.0".to_string(),
                    required: true,
                },
                Dependency {
                    capability: "tts".to_string(),
                    version_req: ">=1.0.0".to_string(),
                    required: false,
                },
            ],
            agent_tools: vec![],
            required_roles: vec!["kernel".into(), "all".into()],
        }
    }

    fn routes(&self, _ctx: &AppContext) -> Option<axum::Router> {
        let state = self.state();
        let kernel = kernel_routes(Arc::clone(&state));
        let wd = watchdog_routes(state);
        let jarvis_state = Arc::new(JarvisProactiveState {
            config: RwLock::new(JarvisConfig::default()),
            daemon_url: "http://localhost:8420".into(),
        });
        let jarvis = jarvis_proactive_routes(jarvis_state);
        Some(kernel.merge(wd).merge(jarvis))
    }

    fn on_start(&self, _ctx: &AppContext) -> convergio_types::extension::ExtResult<()> {
        let pool = self.pool.clone();
        tokio::spawn(async move {
            match TelegramApi::from_env() {
                Ok(api) => run_watchdog(pool, api, WatchdogConfig::default()).await,
                Err(e) => tracing::info!("Telegram not configured, watchdog disabled: {e}"),
            }
        });
        Ok(())
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration {
            version: 1,
            description: "kernel tables",
            up: "CREATE TABLE IF NOT EXISTS kernel_events (\
                    id INTEGER PRIMARY KEY,\
                    timestamp TEXT DEFAULT (datetime('now')),\
                    severity TEXT CHECK(severity IN ('ok','warn','critical')),\
                    source TEXT,\
                    message TEXT,\
                    action_taken TEXT\
                );\
                CREATE INDEX IF NOT EXISTS idx_ke_severity \
                    ON kernel_events(severity);\
                CREATE INDEX IF NOT EXISTS idx_ke_ts \
                    ON kernel_events(timestamp);\
                CREATE TABLE IF NOT EXISTS kernel_verifications (\
                    id INTEGER PRIMARY KEY,\
                    task_id INTEGER,\
                    timestamp TEXT DEFAULT (datetime('now')),\
                    checks_json TEXT,\
                    passed INTEGER,\
                    blocked_reason TEXT\
                );\
                CREATE INDEX IF NOT EXISTS idx_kv_task \
                    ON kernel_verifications(task_id);\
                CREATE TABLE IF NOT EXISTS kernel_config (\
                    key TEXT PRIMARY KEY,\
                    value TEXT,\
                    updated_at TEXT DEFAULT (datetime('now'))\
                );\
                CREATE TABLE IF NOT EXISTS knowledge_base (\
                    id INTEGER PRIMARY KEY,\
                    domain TEXT,\
                    title TEXT,\
                    content TEXT,\
                    created_at TEXT,\
                    hit_count INTEGER DEFAULT 0\
                );\
                CREATE TABLE IF NOT EXISTS telegram_messages (\
                    id INTEGER PRIMARY KEY,\
                    update_id INTEGER UNIQUE,\
                    chat_id INTEGER,\
                    from_user TEXT,\
                    text TEXT,\
                    intent TEXT,\
                    response TEXT,\
                    latency_ms INTEGER,\
                    created_at TEXT DEFAULT (datetime('now'))\
                );\
                CREATE INDEX IF NOT EXISTS idx_tg_chat \
                    ON telegram_messages(chat_id);\
                CREATE INDEX IF NOT EXISTS idx_tg_intent \
                    ON telegram_messages(intent);",
        }]
    }

    fn health(&self) -> Health {
        match self.pool.get() {
            Ok(_) => Health::Ok,
            Err(e) => Health::Degraded {
                reason: format!("db: {e}"),
            },
        }
    }

    fn metrics(&self) -> Vec<Metric> {
        vec![Metric {
            name: "kernel_active".to_string(),
            value: 1.0,
            labels: vec![],
        }]
    }

    fn scheduled_tasks(&self) -> Vec<ScheduledTask> {
        vec![
            ScheduledTask {
                name: "kernel-monitor",
                cron: "* * * * *",
            },
            ScheduledTask {
                name: "kernel-readiness",
                cron: "*/5 * * * *",
            },
        ]
    }

    fn on_scheduled_task(&self, task_name: &str) {
        match task_name {
            "kernel-monitor" => {
                let pool = self.pool.clone();
                tokio::spawn(async move {
                    let config = crate::monitor::MonitorConfig::default();
                    let results = crate::monitor::run_checks(&config);
                    let severity = crate::monitor::classify_results(&results);
                    if let Ok(conn) = pool.get() {
                        let _ = conn.execute(
                            "INSERT INTO kernel_events (severity, source, message, action_taken) \
                             VALUES (?1, 'monitor', 'scheduled check', 'none')",
                            rusqlite::params![severity.to_string()],
                        );
                    }
                });
            }
            "kernel-readiness" => {
                tracing::debug!("kernel readiness check");
            }
            _ => {}
        }
    }

    fn mcp_tools(&self) -> Vec<McpToolDef> {
        crate::mcp_defs::kernel_tools()
    }
}
