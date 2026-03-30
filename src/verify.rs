//! Deterministic verification — validate agent outputs without an LLM call.
//! Catches hallucinations, silent failures, and drift.

use serde::{Deserialize, Serialize};

/// Result of a verification check.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[must_use = "safety outcome must be checked"]
pub enum VerifyResult {
    /// Output matches expectations.
    Valid,
    /// Output is wrong or unexpected.
    Invalid { reason: String },
    /// Output is technically valid but shows drift from expected behavior.
    Drift { score: f64, detail: String },
}

/// A verification check that runs deterministically (no LLM call).
pub trait Verification: Send + Sync {
    fn name(&self) -> &str;
    fn check(&self, action: &str, result: &serde_json::Value) -> VerifyResult;
}

// ── Built-in verifications ───────────────────────────────

/// Verify that a file exists on disk.
pub struct FileExists;

impl Verification for FileExists {
    fn name(&self) -> &str {
        "file_exists"
    }

    fn check(&self, _action: &str, result: &serde_json::Value) -> VerifyResult {
        if let Some(path) = result.get("path").and_then(|p| p.as_str()) {
            if std::path::Path::new(path).exists() {
                VerifyResult::Valid
            } else {
                VerifyResult::Invalid {
                    reason: format!("file not found: {}", path),
                }
            }
        } else {
            VerifyResult::Valid // No path to check
        }
    }
}

/// Verify that a result is not empty.
pub struct NotEmpty;

impl Verification for NotEmpty {
    fn name(&self) -> &str {
        "not_empty"
    }

    fn check(&self, _action: &str, result: &serde_json::Value) -> VerifyResult {
        match result {
            serde_json::Value::Null => VerifyResult::Invalid {
                reason: "result is null".into(),
            },
            serde_json::Value::String(s) if s.is_empty() => VerifyResult::Invalid {
                reason: "result is empty string".into(),
            },
            serde_json::Value::Array(a) if a.is_empty() => VerifyResult::Invalid {
                reason: "result is empty array".into(),
            },
            serde_json::Value::Object(o) if o.is_empty() => VerifyResult::Invalid {
                reason: "result is empty object".into(),
            },
            _ => VerifyResult::Valid,
        }
    }
}

/// Verify that confidence hasn't dropped below a threshold (drift detection).
pub struct ConfidenceThreshold {
    pub min_confidence: f64,
}

impl Verification for ConfidenceThreshold {
    fn name(&self) -> &str {
        "confidence_threshold"
    }

    fn check(&self, _action: &str, result: &serde_json::Value) -> VerifyResult {
        if let Some(confidence) = result.get("confidence").and_then(|c| c.as_f64()) {
            if confidence < self.min_confidence {
                VerifyResult::Drift {
                    score: confidence,
                    detail: format!(
                        "confidence {:.2} below threshold {:.2}",
                        confidence, self.min_confidence
                    ),
                }
            } else {
                VerifyResult::Valid
            }
        } else {
            VerifyResult::Valid // No confidence field
        }
    }
}

/// Verify that output doesn't contain hallucination markers.
pub struct HallucinationCheck;

impl Verification for HallucinationCheck {
    fn name(&self) -> &str {
        "hallucination_check"
    }

    fn check(&self, _action: &str, result: &serde_json::Value) -> VerifyResult {
        let text = result.to_string().to_lowercase();

        // Common hallucination patterns
        let markers = [
            "i cannot",
            "i don't have access",
            "as an ai",
            "i'm not able to",
            "hypothetically",
            "i would assume",
            "i believe this might",
        ];

        for marker in &markers {
            if text.contains(marker) {
                return VerifyResult::Drift {
                    score: 0.3,
                    detail: format!("possible hallucination marker: '{}'", marker),
                };
            }
        }

        VerifyResult::Valid
    }
}

/// Run all verifications and return combined result.
pub fn run_verifications(
    checks: &[Box<dyn Verification>],
    action: &str,
    result: &serde_json::Value,
) -> Vec<(String, VerifyResult)> {
    checks
        .iter()
        .map(|check| (check.name().to_string(), check.check(action, result)))
        .collect()
}

/// Quick check: did ALL verifications pass?
pub fn all_valid(results: &[(String, VerifyResult)]) -> bool {
    results
        .iter()
        .all(|(_, r)| matches!(r, VerifyResult::Valid))
}

/// Quick check: any drift detected?
pub fn has_drift(results: &[(String, VerifyResult)]) -> bool {
    results
        .iter()
        .any(|(_, r)| matches!(r, VerifyResult::Drift { .. }))
}
