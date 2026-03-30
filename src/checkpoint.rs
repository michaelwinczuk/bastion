//! Deterministic checkpointing — snapshot state before risky operations, rollback on failure.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::{BastionError, BastionResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub label: String,
    pub created_at: DateTime<Utc>,
    pub state: serde_json::Value,
    pub metadata: HashMap<String, String>,
}

impl Checkpoint {
    pub fn new(label: &str, state: serde_json::Value) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            label: label.to_string(),
            created_at: Utc::now(),
            state,
            metadata: HashMap::new(),
        }
    }
}

#[async_trait]
pub trait CheckpointStore: Send + Sync {
    async fn save(&self, checkpoint: &Checkpoint) -> BastionResult<()>;
    async fn load(&self, id: &str) -> BastionResult<Checkpoint>;
    async fn list(&self) -> BastionResult<Vec<String>>;
    async fn delete(&self, id: &str) -> BastionResult<()>;
}

/// In-memory checkpoint store (for testing and short-lived agents).
pub struct MemoryStore {
    data: RwLock<HashMap<String, Checkpoint>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl CheckpointStore for MemoryStore {
    async fn save(&self, checkpoint: &Checkpoint) -> BastionResult<()> {
        self.data
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(checkpoint.id.clone(), checkpoint.clone());
        Ok(())
    }

    async fn load(&self, id: &str) -> BastionResult<Checkpoint> {
        self.data
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()
            .ok_or_else(|| BastionError::NotFound(id.to_string()))
    }

    async fn list(&self) -> BastionResult<Vec<String>> {
        Ok(self
            .data
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect())
    }

    async fn delete(&self, id: &str) -> BastionResult<()> {
        self.data
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(id);
        Ok(())
    }
}

/// File-based checkpoint store (persistent across restarts).
pub struct FileStore {
    dir: PathBuf,
}

impl FileStore {
    pub async fn new(dir: PathBuf) -> BastionResult<Self> {
        tokio::fs::create_dir_all(&dir).await?;
        Ok(Self { dir })
    }
}

#[async_trait]
impl CheckpointStore for FileStore {
    async fn save(&self, checkpoint: &Checkpoint) -> BastionResult<()> {
        let path = self.dir.join(format!("{}.json", checkpoint.id));
        let json = serde_json::to_string_pretty(checkpoint)?;
        tokio::fs::write(&path, json).await?;
        Ok(())
    }

    async fn load(&self, id: &str) -> BastionResult<Checkpoint> {
        let path = self.dir.join(format!("{}.json", id));
        let data = tokio::fs::read_to_string(&path)
            .await
            .map_err(|_| BastionError::NotFound(id.to_string()))?;
        Ok(serde_json::from_str(&data)?)
    }

    async fn list(&self) -> BastionResult<Vec<String>> {
        let mut ids = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".json") {
                    ids.push(name.trim_end_matches(".json").to_string());
                }
            }
        }
        Ok(ids)
    }

    async fn delete(&self, id: &str) -> BastionResult<()> {
        let path = self.dir.join(format!("{}.json", id));
        tokio::fs::remove_file(&path).await?;
        Ok(())
    }
}
