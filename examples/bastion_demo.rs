//! Bastion Demo — 60 seconds that shows why every agent system needs this.
//!
//! Scenarios:
//! 1. Normal action → consensus approves → checkpoint → verify ✓
//! 2. Dangerous action → guardrail blocks it before consensus even runs
//! 3. Agent hallucinates → verification catches drift → self-healing rollback
//! 4. Spending limit → finance guardrail blocks overspend
//! 5. Audit trail → cryptographic chain proves everything that happened
//!
//! Run: cargo run --example bastion_demo

use async_trait::async_trait;
use bastion_core::prelude::*;

// ── Mock agents (in production, these call real LLMs) ────

struct SafetyAgent {
    name: String,
}

#[async_trait]
impl Agent for SafetyAgent {
    async fn evaluate(&self, prompt: &str) -> BastionResult<AgentResponse> {
        // Simple heuristic — approve safe actions, reject dangerous ones
        let lower = prompt.to_lowercase();
        let approve = !lower.contains("drop table")
            && !lower.contains("rm -rf")
            && !lower.contains("delete all")
            && !lower.contains("shutdown");

        Ok(AgentResponse {
            content: if approve { "APPROVE" } else { "REJECT" }.into(),
            confidence: if approve { 0.95 } else { 0.90 },
            model_id: self.name.clone(),
            metadata: Default::default(),
        })
    }

    fn agent_id(&self) -> &str {
        &self.name
    }
}

#[tokio::main]
async fn main() {
    println!();
    println!("  ╔══════════════════════════════════════════════════╗");
    println!("  ║  BASTION — Production Kernel for Agentic AI      ║");
    println!("  ║  Consensus + Checkpoints + Verification + Audit  ║");
    println!("  ╚══════════════════════════════════════════════════╝");
    println!();

    // Build the runtime with 3 safety agents + guardrails + verifications
    let runtime = BastionRuntime::builder()
        .add_agent(SafetyAgent {
            name: "claude-sonnet".into(),
        })
        .add_agent(SafetyAgent {
            name: "gpt-4o".into(),
        })
        .add_agent(SafetyAgent {
            name: "gemini-pro".into(),
        })
        .consensus(ConsensusStrategy::Majority)
        .min_confidence(0.8)
        .guardrail(Box::new(SpendingLimit { max_usd: 10_000.0 }))
        .guardrail(Box::new(DangerousPatterns))
        .verification(Box::new(NotEmpty))
        .verification(Box::new(HallucinationCheck))
        .verification(Box::new(ConfidenceThreshold {
            min_confidence: 0.7,
        }))
        .max_retries(3)
        .build();

    // ══════════════════════════════════════════════════════
    // Scenario 1: Safe action — full pipeline
    // ══════════════════════════════════════════════════════
    println!("  ── Scenario 1: Deploy API update ──");
    let outcome = runtime
        .gate("deploy api-server v2.1.0 to production")
        .await
        .unwrap();
    print!("     Gate: ");
    match &outcome {
        GateOutcome::Approved { consensus } => {
            println!(
                "APPROVED ({:.0}% agreement, {}/{} agents)",
                consensus.agreement_ratio * 100.0,
                consensus.total_responses - consensus.dissenting.len(),
                consensus.total_responses
            );
        }
        _ => println!("{:?}", outcome),
    }

    let cp_id = runtime
        .checkpoint("pre-deploy", serde_json::json!({"version": "2.0.0"}))
        .await
        .unwrap();
    println!("     Checkpoint: {}", &cp_id[..8]);

    let verify_results = runtime.verify(
        "deploy",
        &serde_json::json!({
            "status": "success",
            "confidence": 0.95,
        }),
    );
    let all_pass = verify_results
        .iter()
        .all(|(_, r)| matches!(r, VerifyResult::Valid));
    println!(
        "     Verify: {} checks, all passed: {}",
        verify_results.len(),
        all_pass
    );
    println!();

    // ══════════════════════════════════════════════════════
    // Scenario 2: Dangerous action — guardrail blocks
    // ══════════════════════════════════════════════════════
    println!("  ── Scenario 2: Dangerous database command ──");
    let outcome = runtime
        .gate("DROP TABLE users; -- cleanup old data")
        .await
        .unwrap();
    print!("     Gate: ");
    match &outcome {
        GateOutcome::Blocked { reason } => println!("BLOCKED — {}", reason),
        _ => println!("{:?}", outcome),
    }
    println!();

    // ══════════════════════════════════════════════════════
    // Scenario 3: Hallucination detected — drift + rollback
    // ══════════════════════════════════════════════════════
    println!("  ── Scenario 3: Agent hallucinates ──");
    let outcome = runtime.gate("analyze customer data trends").await.unwrap();
    print!("     Gate: ");
    match &outcome {
        GateOutcome::Approved { consensus } => {
            println!("APPROVED ({:.0}%)", consensus.agreement_ratio * 100.0);
        }
        _ => println!("{:?}", outcome),
    }

    let cp2 = runtime
        .checkpoint(
            "pre-analysis",
            serde_json::json!({"dataset": "customers_q4"}),
        )
        .await
        .unwrap();

    // Simulate hallucinated output
    let verify_results = runtime.verify(
        "analyze",
        &serde_json::json!({
            "result": "I believe this might show a trend, hypothetically speaking",
            "confidence": 0.45,
        }),
    );
    let has_issues = verify_results
        .iter()
        .any(|(_, r)| !matches!(r, VerifyResult::Valid));
    println!("     Verify: issues detected: {}", has_issues);
    for (name, result) in &verify_results {
        match result {
            VerifyResult::Drift { score, detail } => {
                println!("     DRIFT: {} — {} (score: {:.2})", name, detail, score);
            }
            VerifyResult::Invalid { reason } => {
                println!("     INVALID: {} — {}", name, reason);
            }
            _ => {}
        }
    }

    // Self-heal
    let heal_action = runtime.heal("drift_detected", "confidence dropped to 0.45", 1, &[]);
    println!("     Heal: {:?}", heal_action);

    let restored = runtime.rollback(&cp2).await.unwrap();
    println!("     Rollback: restored to '{}' checkpoint", restored.label);
    println!();

    // ══════════════════════════════════════════════════════
    // Scenario 4: Spending limit
    // ══════════════════════════════════════════════════════
    println!("  ── Scenario 4: Overspend attempt ──");
    let outcome = runtime
        .gate_with_context(
            "purchase enterprise license",
            &serde_json::json!({"amount_usd": 50_000.0}),
        )
        .await
        .unwrap();
    print!("     Gate: ");
    match &outcome {
        GateOutcome::Blocked { reason } => println!("BLOCKED — {}", reason),
        _ => println!("{:?}", outcome),
    }
    println!();

    // ══════════════════════════════════════════════════════
    // Scenario 5: Audit trail
    // ══════════════════════════════════════════════════════
    let log = runtime.audit_log();
    let (chain_valid, _) = log.verify_chain();
    let metrics = runtime.observe();

    println!("  ── Audit & Metrics ──");
    println!("     Audit entries: {}", log.len());
    println!(
        "     Chain integrity: {}",
        if chain_valid { "VERIFIED" } else { "BROKEN" }
    );
    println!("     Total actions: {}", metrics.total_actions);
    println!("     Approved: {}", metrics.approved);
    println!("     Blocked: {}", metrics.blocked);
    println!("     Drift detections: {}", metrics.drift_detections);
    println!("     Rollbacks: {}", metrics.rollbacks);
    println!("     Avg latency: {:.1}ms", metrics.avg_latency_ms);
    println!();

    println!("  ╔══════════════════════════════════════════════════╗");
    println!("  ║  Demo Complete — $0.00 safety overhead            ║");
    println!("  ║  4 scenarios, 3 agents, sub-millisecond latency  ║");
    println!("  ║  Every decision logged with cryptographic proof  ║");
    println!("  ╚══════════════════════════════════════════════════╝");
    println!();
}
