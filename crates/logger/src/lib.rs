use chrono::Local;
use serde::Serialize;
use serde_json::Value;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum EventType {
    AccountAdded,
    InstrumentAdded,
    PositionAdded,
    TransactionAdded,
    RuleApplied,
    TransactionRemoved,
}

#[derive(Debug, Clone, Serialize)]
struct Event {
    timestamp: String,
    event_type: EventType,
    payload: Value,
}

pub fn log_account_added(account: &Value) {
    write_event(Event {
        timestamp: now_local_iso(),
        event_type: EventType::AccountAdded,
        payload: account.clone(),
    });
}

pub fn log_transaction_added(transaction: &Value) {
    write_event(Event {
        timestamp: now_local_iso(),
        event_type: EventType::TransactionAdded,
        payload: transaction.clone(),
    });
}

pub fn log_instrument_added(instrument: &Value) {
    write_event(Event {
        timestamp: now_local_iso(),
        event_type: EventType::InstrumentAdded,
        payload: instrument.clone(),
    });
}

pub fn log_position_added(position: &Value) {
    write_event(Event {
        timestamp: now_local_iso(),
        event_type: EventType::PositionAdded,
        payload: position.clone(),
    });
}

pub fn log_rule_applied(rule: &Value, before: &Value, after: &Value) {
    write_event(Event {
        timestamp: now_local_iso(),
        event_type: EventType::RuleApplied,
        payload: serde_json::json!({
            "rule": rule,
            "before": before,
            "after": after,
        }),
    });
}

pub fn log_transaction_removed(reason: &str, transaction: &Value) {
    write_event(Event {
        timestamp: now_local_iso(),
        event_type: EventType::TransactionRemoved,
        payload: serde_json::json!({
            "reason": reason,
            "transaction": transaction,
        }),
    });
}

fn write_event(event: Event) {
    let path = log_path();

    if let Some(parent) = path.parent() {
        let _ = create_dir_all(parent);
    }

    let payload = match serde_json::to_string(&event.payload) {
        Ok(value) => value,
        Err(_) => return,
    };

    let line = format!(
        "{} | {} | {}",
        event.timestamp,
        event.event_type.as_str(),
        payload
    );

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{line}");
    }
}

fn now_local_iso() -> String {
    Local::now().to_rfc3339()
}

fn log_path() -> PathBuf {
    if let Ok(raw) = std::env::var("MATAPAN_LOG_PATH") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    let mut workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    workspace_root.pop(); // crates
    workspace_root.pop(); // workspace root

    workspace_root
        .join("logs")
        .join(Local::now().format("%Y-%m-%d.log").to_string())
}

impl EventType {
    fn as_str(&self) -> &'static str {
        match self {
            EventType::AccountAdded => "account_added",
            EventType::InstrumentAdded => "instrument_added",
            EventType::PositionAdded => "position_added",
            EventType::TransactionAdded => "transaction_added",
            EventType::RuleApplied => "rule_applied",
            EventType::TransactionRemoved => "transaction_removed",
        }
    }
}
