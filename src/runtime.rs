//! BastionRuntime — the unified agent safety runtime.
//! Ties together consensus, checkpoints, verification, audit, metrics, healing, and guardrails.

use std::sync::Arc;

use crate::audit::{AuditLog, Severity};
use crate::checkpoint::{Checkpoint, CheckpointStore, MemoryStore};
use crate::consensus::{self, Agent, ConsensusConfig, ConsensusResult, ConsensusStrategy};
use crate::error::{BastionError, BastionResult};
use crate::guardrails::{self, Guardrail, GuardrailVerdict};
use crate::heal;
use crate::observe::{Metrics, MetricsSnapshot, Timer};
use crate::verify::{self, Verification, VerifyResult};

/// The Bastion runtime — wrap any agent action in safety.
pub struct BastionRuntime<S: CheckpointStore = MemoryStore> {
    agents: Vec<Arc<dyn Agent>>,
    consensus_config: ConsensusConfig,
    store: Arc<S>,
    audit: AuditLog,
    metrics: Metrics,
    guardrails: Vec<Box<dyn Guardrail>>,
    verifications: Vec<Box<dyn Verification>>,
    max_retries: u32,
}

impl BastionRuntime<MemoryStore> {
    pub fn builder() -> BastionRuntimeBuilder<MemoryStore> {
        BastionRuntimeBuilder {
            agents: Vec::new(),
            consensus_config: ConsensusConfig::default(),
            store: Arc::new(MemoryStore::new()),
            guardrails: Vec::new(),
            verifications: Vec::new(),
            max_retries: 3,
        }
    }
}

impl<S: CheckpointStore> BastionRuntime<S> {
    pub fn builder_with_store(store: Arc<S>) -> BastionRuntimeBuilder<S> {
        BastionRuntimeBuilder {
            agents: Vec::new(),
            consensus_config: ConsensusConfig::default(),
            store,
            guardrails: Vec::new(),
            verifications: Vec::new(),
            max_retries: 3,
        }
    }

    // ── gate() — consensus before action ─────────────────

    /// Gate an action through multi-model consensus.
    /// Equivalent to `gate_with_context(action, &json!({}))`.
    pub async fn gate(&self, action: &str) -> BastionResult<GateOutcome> {
        self.gate_with_context(action, &serde_json::json!({})).await
    }

    /// Gate with context (for guardrails that need structured data).
    pub async fn gate_with_context(&self, action: &str, context: &serde_json::Value) -> BastionResult<GateOutcome> {
        let timer = Timer::start();

        let guardrail_results = guardrails::evaluate_all(&self.guardrails, action, context);
        if let Some(blocked) = guardrails::any_blocked(&guardrail_results) {
            let reason = match &blocked.verdict {
                GuardrailVerdict::Block { reason } => reason.clone(),
                _ => "blocked".into(),
            };
            self.audit.log(Severity::Critical, action, &format!("BLOCKED: {}", reason), Some(context.clone()));
            self.metrics.record_action(false, timer.elapsed_ms(), 0.0);
            return Ok(GateOutcome::Blocked { reason });
        }

        match consensus::run_consensus(&self.agents, action, &self.consensus_config).await {
            Ok(result) => {
                self.audit.log(Severity::Info, action, &format!("APPROVED: {:.0}%", result.agreement_ratio * 100.0), None);
                self.metrics.record_action(true, timer.elapsed_ms(), 0.0);
                self.metrics.record_consensus(true);
                Ok(GateOutcome::Approved { consensus: result })
            }
            Err(BastionError::ConsensusFailure { reason }) => {
                self.audit.log(Severity::Warning, action, &format!("REJECTED: {}", reason), None);
                self.metrics.record_action(false, timer.elapsed_ms(), 0.0);
                self.metrics.record_consensus(false);
                Ok(GateOutcome::Rejected { reason })
            }
            Err(e) => Err(e),
        }
    }

    // ── checkpoint() — snapshot state ─────────────────────

    pub async fn checkpoint(&self, label: &str, state: serde_json::Value) -> BastionResult<String> {
        let cp = Checkpoint::new(label, state);
        let id = cp.id.clone();
        self.store.save(&cp).await?;
        self.audit.log(Severity::Info, "checkpoint", &format!("saved: {}", label), None);
        Ok(id)
    }

    // ── verify() — deterministic validation ──────────────

    pub fn verify(&self, action: &str, result: &serde_json::Value) -> Vec<(String, VerifyResult)> {
        let results = verify::run_verifications(&self.verifications, action, result);
        for (name, r) in &results {
            match r {
                VerifyResult::Valid => {
                    self.metrics.record_verification(true);
                }
                VerifyResult::Invalid { reason } => {
                    self.audit.log(Severity::Critical, action, &format!("VERIFY FAILED ({}): {}", name, reason), None);
                    self.metrics.record_verification(false);
                }
                VerifyResult::Drift { score, detail } => {
                    self.audit.log(Severity::Warning, action, &format!("DRIFT ({}): {} (score={:.2})", name, detail, score), None);
                    self.metrics.record_drift();
                }
            }
        }
        results
    }

    // ── rollback() — restore to checkpoint ───────────────

    pub async fn rollback(&self, checkpoint_id: &str) -> BastionResult<Checkpoint> {
        let cp = self.store.load(checkpoint_id).await?;
        self.audit.log(Severity::Critical, "rollback", &format!("restored to: {}", cp.label), None);
        self.metrics.record_rollback();
        Ok(cp)
    }

    // ── heal() — self-healing decision ───────────────────

    pub fn heal(&self, failure_type: &str, error: &str, attempt: u32, prev_failures: &[String]) -> heal::HealAction {
        let ctx = heal::HealDecision {
            attempt,
            max_retries: self.max_retries,
            failure_type: failure_type.into(),
            error_detail: error.into(),
            previous_failures: prev_failures.to_vec(),
        };
        let action = heal::decide(&ctx);
        self.audit.log(
            Severity::Warning,
            &format!("heal:{}", failure_type),
            &format!("{:?}", action),
            None,
        );
        action
    }

    // ── observe() — metrics snapshot ─────────────────────

    pub fn observe(&self) -> MetricsSnapshot {
        self.metrics.snapshot()
    }

    // ── audit access ─────────────────────────────────────

    pub fn audit_log(&self) -> &AuditLog {
        &self.audit
    }
}

// ── Gate outcome ─────────────────────────────────────────

#[derive(Debug, Clone)]
#[must_use = "safety outcome must be checked"]
pub enum GateOutcome {
    Approved { consensus: ConsensusResult },
    Rejected { reason: String },
    Blocked { reason: String },
}

impl GateOutcome {
    pub fn is_approved(&self) -> bool {
        matches!(self, GateOutcome::Approved { .. })
    }
}

// ── Builder ──────────────────────────────────────────────

pub struct BastionRuntimeBuilder<S: CheckpointStore> {
    agents: Vec<Arc<dyn Agent>>,
    consensus_config: ConsensusConfig,
    store: Arc<S>,
    guardrails: Vec<Box<dyn Guardrail>>,
    verifications: Vec<Box<dyn Verification>>,
    max_retries: u32,
}

impl<S: CheckpointStore> BastionRuntimeBuilder<S> {
    pub fn add_agent<A: Agent + 'static>(mut self, agent: A) -> Self {
        self.agents.push(Arc::new(agent));
        self
    }

    pub fn consensus(mut self, strategy: ConsensusStrategy) -> Self {
        self.consensus_config.strategy = strategy;
        self
    }

    pub fn min_confidence(mut self, min: f64) -> Self {
        self.consensus_config.min_confidence = min;
        self
    }

    pub fn timeout_ms(mut self, ms: u64) -> Self {
        self.consensus_config.timeout_ms = ms;
        self
    }

    pub fn guardrail(mut self, g: Box<dyn Guardrail>) -> Self {
        self.guardrails.push(g);
        self
    }

    pub fn verification(mut self, v: Box<dyn Verification>) -> Self {
        self.verifications.push(v);
        self
    }

    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    pub fn build(self) -> BastionRuntime<S> {
        BastionRuntime {
            agents: self.agents,
            consensus_config: self.consensus_config,
            store: self.store,
            audit: AuditLog::new(),
            metrics: Metrics::new(),
            guardrails: self.guardrails,
            verifications: self.verifications,
            max_retries: self.max_retries,
        }
    }
}
