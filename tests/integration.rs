use async_trait::async_trait;
use bastion_core::prelude::*;

struct ApproveAgent;
#[async_trait]
impl Agent for ApproveAgent {
    async fn evaluate(&self, _: &str) -> BastionResult<AgentResponse> {
        Ok(AgentResponse { content: "APPROVE".into(), confidence: 0.95, model_id: "test".into(), metadata: Default::default() })
    }
    fn agent_id(&self) -> &str { "approve" }
}

struct RejectAgent;
#[async_trait]
impl Agent for RejectAgent {
    async fn evaluate(&self, _: &str) -> BastionResult<AgentResponse> {
        Ok(AgentResponse { content: "REJECT".into(), confidence: 0.90, model_id: "test".into(), metadata: Default::default() })
    }
    fn agent_id(&self) -> &str { "reject" }
}

#[tokio::test]
async fn test_gate_approved() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).add_agent(ApproveAgent).add_agent(ApproveAgent).build();
    let outcome = rt.gate("safe action").await.unwrap();
    assert!(outcome.is_approved());
}

#[tokio::test]
async fn test_gate_blocked_by_guardrail() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).guardrail(Box::new(DangerousPatterns)).build();
    let outcome = rt.gate("DROP TABLE users").await.unwrap();
    assert!(matches!(outcome, GateOutcome::Blocked { .. }));
}

#[tokio::test]
async fn test_gate_spending_limit() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).guardrail(Box::new(SpendingLimit { max_usd: 100.0 })).build();
    let outcome = rt.gate_with_context("buy", &serde_json::json!({"amount_usd": 500.0})).await.unwrap();
    assert!(matches!(outcome, GateOutcome::Blocked { .. }));
}

#[tokio::test]
async fn test_checkpoint_and_rollback() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).build();
    let cp_id = rt.checkpoint("test", serde_json::json!({"state": "before"})).await.unwrap();
    let restored = rt.rollback(&cp_id).await.unwrap();
    assert_eq!(restored.label, "test");
    assert_eq!(restored.state["state"], "before");
}

#[tokio::test]
async fn test_verify_valid() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).verification(Box::new(NotEmpty)).build();
    let results = rt.verify("action", &serde_json::json!({"data": "exists"}));
    assert!(results.iter().all(|(_, r)| matches!(r, VerifyResult::Valid)));
}

#[tokio::test]
async fn test_verify_catches_empty() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).verification(Box::new(NotEmpty)).build();
    let results = rt.verify("action", &serde_json::json!(null));
    assert!(results.iter().any(|(_, r)| matches!(r, VerifyResult::Invalid { .. })));
}

#[tokio::test]
async fn test_verify_catches_drift() {
    let rt = BastionRuntime::builder()
        .add_agent(ApproveAgent)
        .verification(Box::new(ConfidenceThreshold { min_confidence: 0.8 }))
        .build();
    let results = rt.verify("action", &serde_json::json!({"confidence": 0.3}));
    assert!(results.iter().any(|(_, r)| matches!(r, VerifyResult::Drift { .. })));
}

#[tokio::test]
async fn test_verify_catches_hallucination() {
    let rt = BastionRuntime::builder()
        .add_agent(ApproveAgent)
        .verification(Box::new(HallucinationCheck))
        .build();
    let results = rt.verify("action", &serde_json::json!({"text": "I believe this might work hypothetically"}));
    assert!(results.iter().any(|(_, r)| matches!(r, VerifyResult::Drift { .. })));
}

#[tokio::test]
async fn test_heal_retry() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).build();
    let action = rt.heal("timeout", "request timed out", 1, &[]);
    assert!(matches!(action, HealAction::Retry { .. }));
}

#[tokio::test]
async fn test_heal_abort_after_max_retries() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).max_retries(2).build();
    let action = rt.heal("timeout", "request timed out", 3, &[]);
    assert!(matches!(action, HealAction::Abort { .. }));
}

#[tokio::test]
async fn test_heal_escalate_on_oscillation() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).build();
    let prev = vec!["error A".into(), "error A".into()];
    let action = rt.heal("verification_failed", "same error", 2, &prev);
    assert!(matches!(action, HealAction::Escalate { .. }));
}

#[tokio::test]
async fn test_audit_chain_integrity() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).add_agent(ApproveAgent).build();
    rt.gate("action 1").await.unwrap();
    rt.gate("action 2").await.unwrap();
    rt.gate("action 3").await.unwrap();
    let (valid, _) = rt.audit_log().verify_chain();
    assert!(valid);
}

#[tokio::test]
async fn test_metrics_tracking() {
    let rt = BastionRuntime::builder()
        .add_agent(ApproveAgent).add_agent(ApproveAgent)
        .guardrail(Box::new(DangerousPatterns))
        .build();
    rt.gate("safe action").await.unwrap();
    rt.gate("DROP TABLE x").await.unwrap();
    let m = rt.observe();
    assert_eq!(m.total_actions, 2);
    assert_eq!(m.approved, 1);
    assert_eq!(m.blocked, 1);
}

#[tokio::test]
async fn test_medical_guardrail_warns() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).guardrail(Box::new(MedicalDisclaimer)).build();
    // Medical guardrail warns but doesn't block (it's a Warn, not Block)
    let outcome = rt.gate("prescribe medication for patient").await.unwrap();
    // Should still be approved since MedicalDisclaimer returns Warn, not Block
    assert!(outcome.is_approved());
}

#[tokio::test]
async fn test_defense_human_in_loop() {
    let rt = BastionRuntime::builder().add_agent(ApproveAgent).guardrail(Box::new(HumanInLoop)).build();
    let outcome = rt.gate_with_context("launch drone", &serde_json::json!({"human_approved": false})).await.unwrap();
    assert!(matches!(outcome, GateOutcome::Blocked { .. }));

    let outcome2 = rt.gate_with_context("launch drone", &serde_json::json!({"human_approved": true})).await.unwrap();
    assert!(outcome2.is_approved());
}
