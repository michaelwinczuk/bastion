//! Multi-model consensus — multiple agents must agree before any action proceeds.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{BastionError, BastionResult};

/// Trait for pluggable agent backends.
#[async_trait]
pub trait Agent: Send + Sync {
    async fn evaluate(&self, prompt: &str) -> BastionResult<AgentResponse>;
    fn agent_id(&self) -> &str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub content: String,
    pub confidence: f64,
    pub model_id: String,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsensusStrategy {
    /// More than half must agree.
    Majority,
    /// All agents must agree.
    Unanimous,
    /// Weighted by confidence — highest weighted group wins.
    Weighted,
    /// Custom threshold (e.g., 0.67 for 2/3 supermajority).
    Supermajority(f64),
}

impl Default for ConsensusStrategy {
    fn default() -> Self {
        Self::Majority
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusConfig {
    pub strategy: ConsensusStrategy,
    pub min_confidence: f64,
    pub timeout_ms: u64,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        Self {
            strategy: ConsensusStrategy::Majority,
            min_confidence: 0.0,
            timeout_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    pub chosen: AgentResponse,
    pub agreement_ratio: f64,
    pub dissenting: Vec<AgentResponse>,
    pub confidence: f64,
    pub total_responses: usize,
    pub failed_agents: usize,
}

/// Run consensus across multiple agents.
pub async fn run_consensus(
    agents: &[Arc<dyn Agent>],
    prompt: &str,
    config: &ConsensusConfig,
) -> BastionResult<ConsensusResult> {
    if agents.is_empty() {
        return Err(BastionError::NoAgents);
    }

    let timeout = Duration::from_millis(config.timeout_ms);

    // Run all agents concurrently
    let mut handles = Vec::new();
    for agent in agents {
        let agent = agent.clone();
        let prompt = prompt.to_string();
        handles.push(tokio::spawn(async move {
            tokio::time::timeout(timeout, agent.evaluate(&prompt)).await
        }));
    }

    let mut responses = Vec::new();
    let mut failed = 0usize;
    for handle in handles {
        match handle.await {
            Ok(Ok(Ok(resp))) => {
                if resp.confidence >= config.min_confidence {
                    responses.push(resp);
                }
            }
            _ => failed += 1,
        }
    }

    if responses.is_empty() {
        return Err(BastionError::ConsensusFailure {
            reason: "no valid responses".into(),
        });
    }

    // Group by normalized content
    let mut groups: HashMap<String, Vec<AgentResponse>> = HashMap::new();
    for resp in &responses {
        let key = resp.content.trim().to_lowercase();
        groups.entry(key).or_default().push(resp.clone());
    }

    let mut sorted_groups: Vec<(String, Vec<AgentResponse>)> = groups.into_iter().collect();
    sorted_groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

    let total = responses.len();
    let (_, winner_group) = &sorted_groups[0];
    let agreement_ratio = winner_group.len() as f64 / total as f64;

    // Check strategy
    let required = match &config.strategy {
        ConsensusStrategy::Majority => 0.5,
        ConsensusStrategy::Unanimous => 1.0,
        ConsensusStrategy::Supermajority(threshold) => *threshold,
        ConsensusStrategy::Weighted => 0.0, // Weighted uses confidence, not count
    };

    if config.strategy == ConsensusStrategy::Weighted {
        // Sum confidence per group, pick highest
        let mut best_score = 0.0f64;
        let mut best_group = &sorted_groups[0].1;
        for (_, group) in &sorted_groups {
            let score: f64 = group.iter().map(|r| r.confidence).sum();
            if score > best_score {
                best_score = score;
                best_group = group;
            }
        }
        let chosen = best_group[0].clone();
        let dissenting: Vec<AgentResponse> = responses
            .iter()
            .filter(|r| r.content.trim().to_lowercase() != chosen.content.trim().to_lowercase())
            .cloned()
            .collect();
        let avg_conf =
            best_group.iter().map(|r| r.confidence).sum::<f64>() / best_group.len() as f64;

        return Ok(ConsensusResult {
            chosen,
            agreement_ratio: best_group.len() as f64 / total as f64,
            dissenting,
            confidence: avg_conf,
            total_responses: total,
            failed_agents: failed,
        });
    }

    if agreement_ratio < required {
        return Err(BastionError::ConsensusFailure {
            reason: format!(
                "agreement {:.0}% < required {:.0}%",
                agreement_ratio * 100.0,
                required * 100.0,
            ),
        });
    }

    let chosen = winner_group[0].clone();
    let dissenting: Vec<AgentResponse> = responses
        .iter()
        .filter(|r| r.content.trim().to_lowercase() != chosen.content.trim().to_lowercase())
        .cloned()
        .collect();
    let avg_conf =
        winner_group.iter().map(|r| r.confidence).sum::<f64>() / winner_group.len() as f64;

    Ok(ConsensusResult {
        chosen,
        agreement_ratio,
        dissenting,
        confidence: avg_conf,
        total_responses: total,
        failed_agents: failed,
    })
}
