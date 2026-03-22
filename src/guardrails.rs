//! Pluggable domain guardrails — different rules for finance, medical, coding, defense.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GuardrailVerdict {
    Allow,
    Block { reason: String },
    Warn { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailResult {
    pub rule: String,
    pub verdict: GuardrailVerdict,
    pub risk_score: f64,
}

/// A guardrail that evaluates whether an action should proceed.
pub trait Guardrail: Send + Sync {
    fn name(&self) -> &str;
    fn domain(&self) -> &str;
    fn evaluate(&self, action: &str, context: &serde_json::Value) -> GuardrailResult;
}

// ── Finance guardrails ───────────────────────────────────

pub struct SpendingLimit {
    pub max_usd: f64,
}

impl Guardrail for SpendingLimit {
    fn name(&self) -> &str { "spending_limit" }
    fn domain(&self) -> &str { "finance" }

    fn evaluate(&self, _action: &str, context: &serde_json::Value) -> GuardrailResult {
        let amount = context.get("amount_usd").and_then(|a| a.as_f64()).unwrap_or(0.0);
        if amount > self.max_usd {
            GuardrailResult {
                rule: self.name().into(),
                verdict: GuardrailVerdict::Block {
                    reason: format!("${:.2} exceeds limit ${:.2}", amount, self.max_usd),
                },
                risk_score: (amount / self.max_usd).min(1.0),
            }
        } else {
            GuardrailResult {
                rule: self.name().into(),
                verdict: GuardrailVerdict::Allow,
                risk_score: amount / self.max_usd,
            }
        }
    }
}

// ── Code guardrails ──────────────────────────────────────

pub struct DangerousPatterns;

impl Guardrail for DangerousPatterns {
    fn name(&self) -> &str { "dangerous_patterns" }
    fn domain(&self) -> &str { "coding" }

    fn evaluate(&self, action: &str, _context: &serde_json::Value) -> GuardrailResult {
        let lower = action.to_lowercase();
        let dangerous = [
            "rm -rf /", "drop table", "delete from", "format c:",
            "shutdown", "exec(", "eval(", "system(",
        ];

        for pattern in &dangerous {
            if lower.contains(pattern) {
                return GuardrailResult {
                    rule: self.name().into(),
                    verdict: GuardrailVerdict::Block {
                        reason: format!("dangerous pattern detected: {}", pattern),
                    },
                    risk_score: 1.0,
                };
            }
        }

        GuardrailResult {
            rule: self.name().into(),
            verdict: GuardrailVerdict::Allow,
            risk_score: 0.0,
        }
    }
}

// ── Medical guardrails ───────────────────────────────────

pub struct MedicalDisclaimer;

impl Guardrail for MedicalDisclaimer {
    fn name(&self) -> &str { "medical_disclaimer" }
    fn domain(&self) -> &str { "medical" }

    fn evaluate(&self, action: &str, _context: &serde_json::Value) -> GuardrailResult {
        let lower = action.to_lowercase();
        let medical_terms = ["prescribe", "diagnose", "dosage", "treatment plan", "medication"];

        for term in &medical_terms {
            if lower.contains(term) {
                return GuardrailResult {
                    rule: self.name().into(),
                    verdict: GuardrailVerdict::Warn {
                        reason: format!("medical action '{}' requires human oversight", term),
                    },
                    risk_score: 0.8,
                };
            }
        }

        GuardrailResult {
            rule: self.name().into(),
            verdict: GuardrailVerdict::Allow,
            risk_score: 0.0,
        }
    }
}

// ── Defense guardrails ───────────────────────────────────

pub struct HumanInLoop;

impl Guardrail for HumanInLoop {
    fn name(&self) -> &str { "human_in_loop" }
    fn domain(&self) -> &str { "defense" }

    fn evaluate(&self, _action: &str, context: &serde_json::Value) -> GuardrailResult {
        let has_human_approval = context.get("human_approved").and_then(|v| v.as_bool()).unwrap_or(false);
        if !has_human_approval {
            GuardrailResult {
                rule: self.name().into(),
                verdict: GuardrailVerdict::Block {
                    reason: "defense actions require explicit human approval".into(),
                },
                risk_score: 1.0,
            }
        } else {
            GuardrailResult {
                rule: self.name().into(),
                verdict: GuardrailVerdict::Allow,
                risk_score: 0.0,
            }
        }
    }
}

/// Run all guardrails and return results.
pub fn evaluate_all(
    guardrails: &[Box<dyn Guardrail>],
    action: &str,
    context: &serde_json::Value,
) -> Vec<GuardrailResult> {
    guardrails.iter().map(|g| g.evaluate(action, context)).collect()
}

/// Quick check: any guardrail blocked?
pub fn any_blocked(results: &[GuardrailResult]) -> Option<&GuardrailResult> {
    results.iter().find(|r| matches!(r.verdict, GuardrailVerdict::Block { .. }))
}
