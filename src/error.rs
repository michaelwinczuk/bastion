use thiserror::Error;

#[derive(Debug, Error)]
pub enum BastionError {
    #[error("consensus failed: {reason}")]
    ConsensusFailure { reason: String },

    #[error("agent error ({agent_id}): {message}")]
    AgentError { agent_id: String, message: String },

    #[error("verification failed: {reason}")]
    VerificationFailed { reason: String },

    #[error("checkpoint error: {0}")]
    Checkpoint(String),

    #[error("checkpoint not found: {0}")]
    NotFound(String),

    #[error("guardrail blocked: {rule}: {reason}")]
    GuardrailBlocked { rule: String, reason: String },

    #[error("timeout after {ms}ms")]
    Timeout { ms: u64 },

    #[error("no agents configured")]
    NoAgents,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type BastionResult<T> = Result<T, BastionError>;
