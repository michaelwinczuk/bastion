//! Immutable audit trail with cryptographic hash chaining.
//! Every decision, action, and failure is logged with tamper evidence.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub severity: Severity,
    pub action: String,
    pub detail: String,
    pub prev_hash: String,
    pub hash: String,
    pub data: Option<serde_json::Value>,
}

pub struct AuditLog {
    entries: RwLock<Vec<AuditEntry>>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self { entries: RwLock::new(Vec::new()) }
    }

    pub fn log(&self, severity: Severity, action: &str, detail: &str, data: Option<serde_json::Value>) -> String {
        let mut entries = self.entries.write().unwrap_or_else(|e| e.into_inner());
        let prev_hash = entries.last().map(|e| e.hash.clone()).unwrap_or_else(|| "GENESIS".into());
        let id = uuid::Uuid::new_v4().to_string();
        let timestamp = Utc::now();

        let hash_input = format!("{}|{}|{}|{}|{}", id, timestamp, action, detail, prev_hash);
        let hash = sha256_hash(&hash_input);

        let entry = AuditEntry {
            id: id.clone(),
            timestamp,
            severity,
            action: action.to_string(),
            detail: detail.to_string(),
            prev_hash,
            hash,
            data,
        };
        entries.push(entry);
        id
    }

    pub fn entries(&self) -> Vec<AuditEntry> {
        self.entries.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn len(&self) -> usize {
        self.entries.read().unwrap_or_else(|e| e.into_inner()).len()
    }

    pub fn verify_chain(&self) -> (bool, Option<usize>) {
        let entries = self.entries.read().unwrap_or_else(|e| e.into_inner());
        for i in 1..entries.len() {
            if entries[i].prev_hash != entries[i - 1].hash {
                return (false, Some(i));
            }
        }
        for (i, entry) in entries.iter().enumerate() {
            let expected = sha256_hash(&format!(
                "{}|{}|{}|{}|{}",
                entry.id, entry.timestamp, entry.action, entry.detail, entry.prev_hash
            ));
            if entry.hash != expected {
                return (false, Some(i));
            }
        }
        (true, None)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&*self.entries.read().unwrap_or_else(|e| e.into_inner()))
    }
}

fn sha256_hash(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}
