//! Self-healing decision tree — when something fails, decide: retry, escalate, or abort.

use serde::{Deserialize, Serialize};

/// What the healer decides to do.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[must_use = "safety outcome must be checked"]
pub enum HealAction {
    /// Retry the same action (possibly with a different model).
    Retry { reason: String },
    /// Retry with reduced scope or simpler approach.
    RetrySimplified { reason: String },
    /// Escalate to human review.
    Escalate { reason: String },
    /// Abort — too many failures, give up.
    Abort { reason: String },
    /// Rollback to checkpoint and try different approach.
    Rollback { checkpoint_id: String, reason: String },
}

/// Decision context for the healer.
#[derive(Debug, Clone)]
pub struct HealDecision {
    pub attempt: u32,
    pub max_retries: u32,
    pub failure_type: String,
    pub error_detail: String,
    pub previous_failures: Vec<String>,
}

impl Default for HealDecision {
    fn default() -> Self {
        Self {
            attempt: 1,
            max_retries: 3,
            failure_type: String::new(),
            error_detail: String::new(),
            previous_failures: Vec::new(),
        }
    }
}

/// Decide what to do when an action fails.
pub fn decide(ctx: &HealDecision) -> HealAction {
    // Too many attempts — abort
    if ctx.attempt > ctx.max_retries {
        return HealAction::Abort {
            reason: format!("exceeded max retries ({})", ctx.max_retries),
        };
    }

    // Check for oscillation — same error repeating
    if ctx.previous_failures.len() >= 2 {
        let last = ctx.previous_failures.last().unwrap();
        let prev = &ctx.previous_failures[ctx.previous_failures.len() - 2];
        if last == prev {
            return HealAction::Escalate {
                reason: format!("oscillation detected: same error '{}' repeated", last),
            };
        }
    }

    // Decision tree based on failure type
    match ctx.failure_type.as_str() {
        "consensus_failure" => {
            if ctx.attempt == 1 {
                HealAction::Retry {
                    reason: "consensus not reached — retrying with same agents".into(),
                }
            } else {
                HealAction::Escalate {
                    reason: "consensus failed multiple times — needs human review".into(),
                }
            }
        }

        "verification_failed" | "invalid_output" => {
            if ctx.attempt <= 2 {
                HealAction::Retry {
                    reason: "output verification failed — retrying".into(),
                }
            } else {
                HealAction::RetrySimplified {
                    reason: "output keeps failing verification — simplifying".into(),
                }
            }
        }

        "drift_detected" => {
            HealAction::Rollback {
                checkpoint_id: String::new(), // Caller fills this
                reason: format!("drift detected: {}", ctx.error_detail),
            }
        }

        "timeout" => {
            HealAction::Retry {
                reason: "operation timed out — retrying".into(),
            }
        }

        "guardrail_blocked" => {
            HealAction::Escalate {
                reason: format!("guardrail blocked action: {}", ctx.error_detail),
            }
        }

        _ => {
            if ctx.attempt == 1 {
                HealAction::Retry {
                    reason: format!("unknown failure: {} — retrying", ctx.failure_type),
                }
            } else {
                HealAction::Escalate {
                    reason: format!("repeated failure: {}", ctx.failure_type),
                }
            }
        }
    }
}
