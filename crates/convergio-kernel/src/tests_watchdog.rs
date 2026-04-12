//! Tests for watchdog: keyword classification, handlers, Telegram, ext.

mod watchdog_tests {
    use crate::watchdog::{keyword_intent, IntentClassification, WatchdogConfig, HELP_TEXT};

    #[test]
    fn watchdog_config_default() {
        let cfg = WatchdogConfig::default();
        assert_eq!(cfg.daemon_url, "http://localhost:8420");
        assert_eq!(cfg.poll_interval_secs, 3);
        assert!(cfg.model.contains("Qwen"));
    }

    #[test]
    fn keyword_status_variants() {
        assert_eq!(keyword_intent("stato"), Some("status"));
        assert_eq!(keyword_intent("come va?"), Some("status"));
        assert_eq!(keyword_intent("system status"), Some("status"));
        assert_eq!(keyword_intent("salute"), Some("status"));
        assert_eq!(keyword_intent("health check"), Some("status"));
    }

    #[test]
    fn keyword_plan() {
        assert_eq!(keyword_intent("mostra i piani"), Some("plan"));
        assert_eq!(keyword_intent("plans"), Some("plan"));
    }

    #[test]
    fn keyword_cost() {
        assert_eq!(keyword_intent("quanto costa?"), Some("cost"));
        assert_eq!(keyword_intent("costs"), Some("cost"));
    }

    #[test]
    fn keyword_help() {
        assert_eq!(keyword_intent("/help"), Some("help"));
        assert_eq!(keyword_intent("aiuto"), Some("help"));
    }

    #[test]
    fn keyword_deploy() {
        assert_eq!(keyword_intent("deploy now"), Some("deploy"));
        assert_eq!(keyword_intent("rilascio versione"), Some("deploy"));
    }

    #[test]
    fn keyword_night_agents() {
        assert_eq!(keyword_intent("night-agent status"), Some("night-agents"));
        assert_eq!(keyword_intent("overnight jobs"), Some("night-agents"));
    }

    #[test]
    fn keyword_mesh() {
        assert_eq!(keyword_intent("mesh status"), Some("mesh"));
        assert_eq!(keyword_intent("quanti nodi?"), Some("mesh"));
    }

    #[test]
    fn keyword_security() {
        assert_eq!(keyword_intent("security audit"), Some("security"));
        assert_eq!(keyword_intent("sicurezza"), Some("security"));
    }

    #[test]
    fn keyword_config() {
        assert_eq!(keyword_intent("config update"), Some("config"));
        assert_eq!(keyword_intent("configurazione"), Some("config"));
    }

    #[test]
    fn keyword_none_for_generic() {
        assert_eq!(keyword_intent("hello world"), None);
        assert_eq!(keyword_intent("random text"), None);
    }

    #[test]
    fn intent_classification_struct() {
        let ic = IntentClassification {
            intent: "status".into(),
            confidence: "high".into(),
            score: 1.0,
            cloud_escalation_hint: None,
        };
        assert_eq!(ic.confidence, "high");
        assert!(ic.cloud_escalation_hint.is_none());
    }

    #[test]
    fn low_confidence_triggers_escalation_hint() {
        let ic = IntentClassification {
            intent: "general".into(),
            confidence: "medium".into(),
            score: 0.3,
            cloud_escalation_hint: Some("Low confidence".into()),
        };
        assert!(ic.score < 0.5);
        assert!(ic.cloud_escalation_hint.is_some());
    }

    #[test]
    fn valid_intents_includes_new_categories() {
        use crate::watchdog::VALID_INTENTS;
        for cat in &["deploy", "night-agents", "mesh", "security", "config"] {
            assert!(VALID_INTENTS.contains(cat), "missing category: {cat}");
        }
    }

    #[test]
    fn help_text_mentions_real_data() {
        assert!(HELP_TEXT.contains("real system data"));
        assert!(HELP_TEXT.contains("Jarvis"));
    }

    #[test]
    fn log_event_no_panic() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        crate::watchdog::log_event(&pool, "test", "hello");
    }
}

mod handler_tests {
    use crate::watchdog_handlers::*;
    use serde_json::Value;

    #[test]
    fn format_health_unreachable() {
        let out = format_health(&Value::Null);
        assert!(out.contains("0/0"), "null → 0/0: {out}");
    }

    #[test]
    fn format_health_ok() {
        let v = serde_json::json!({"status": "ok", "components": [
            {"name":"db","status":"ok"},
            {"name":"ipc","status":"ok"}
        ]});
        let out = format_health(&v);
        assert!(out.contains("2/2"), "all ok → 2/2: {out}");
    }

    #[test]
    fn format_plans_empty() {
        let v = serde_json::json!({"plans": []});
        assert!(format_plans(&v).contains("0"));
    }

    #[test]
    fn format_agents_missing() {
        assert!(format_agents(&Value::Null).contains("0"));
    }

    #[test]
    fn format_peers_with_data() {
        let v = serde_json::json!({"peers": ["a", "b", "c"]});
        assert!(format_peers(&v).contains("3"));
    }
}

mod telegram_poller_tests {
    use crate::telegram_poller::{TelegramApi, TelegramUpdate};

    #[test]
    fn api_authorization() {
        let api = TelegramApi::new("t".into(), vec!["100".into()]);
        assert!(api.is_authorized(100));
        assert!(!api.is_authorized(200));
    }

    #[test]
    fn deserialize_empty_result() {
        let json = r#"[]"#;
        let updates: Vec<TelegramUpdate> = serde_json::from_str(json).unwrap();
        assert!(updates.is_empty());
    }
}

mod ext_migration_v5_tests {
    use convergio_types::extension::Extension;

    #[test]
    fn has_one_migration() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let ext = crate::ext::KernelExtension::new(pool);
        let migs = ext.migrations();
        assert_eq!(migs.len(), 1);
    }

    #[test]
    fn migration_v5_sql_valid() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let conn = pool.get().unwrap();
        let ext = crate::ext::KernelExtension::new(pool.clone());
        for mig in ext.migrations() {
            conn.execute_batch(mig.up).unwrap();
        }
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM telegram_messages", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn manifest_has_telegram_watchdog_capability() {
        let pool = convergio_db::pool::create_memory_pool().unwrap();
        let ext = crate::ext::KernelExtension::new(pool);
        let m = ext.manifest();
        assert!(m.provides.iter().any(|c| c.name == "telegram-watchdog"));
    }
}
