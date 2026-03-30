# Bastion

[![CI](https://github.com/michaelwinczuk/bastion/actions/workflows/ci.yml/badge.svg)](https://github.com/michaelwinczuk/bastion/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE)

**Safety primitives for agentic AI systems.**

A Rust + Tokio library that provides consensus gating, deterministic verification, checkpointing with rollback, and tamper-evident audit trails for AI agent workflows. Every safety check runs without LLM calls — sub-millisecond overhead on the critical path.

---

## What This Does

Bastion sits between your agents and their actions. Before any agent can execute, the action passes through a safety pipeline:

1. **Guardrails** — Fast domain-specific rules (spending limits, dangerous patterns, human-in-the-loop)
2. **Consensus** — Multiple models must agree before proceeding (Majority, Unanimous, Weighted, Supermajority)
3. **Checkpointing** — Snapshot state before risky operations, rollback if verification fails
4. **Verification** — Deterministic checks for empty responses, confidence drops, and hallucination markers
5. **Self-healing** — Retry/escalate/abort decision tree based on failure type and history
6. **Audit trail** — SHA-256 hash-chained log entries — tamper with any entry and the chain breaks

All safety checks are deterministic. No LLM calls in the critical path.

## Quick Start

```bash
cargo add bastion-core
```

```rust
use bastion_core::prelude::*;

let runtime = BastionRuntime::builder()
    .add_agent(my_agent_1)
    .add_agent(my_agent_2)
    .add_agent(my_agent_3)
    .consensus(ConsensusStrategy::Majority)
    .guardrail(Box::new(SpendingLimit { max_usd: 10_000.0 }))
    .verification(Box::new(HallucinationCheck))
    .build();

// Gate: guardrails first, then consensus
let outcome = runtime.gate("execute trade AAPL 100 shares").await?;

// Checkpoint before execution
let cp = runtime.checkpoint("pre-trade", state).await?;

// Verify result without LLM calls
let checks = runtime.verify("trade", &result);
if !bastion_core::verify::all_valid(&checks) {
    runtime.rollback(&cp).await?;
}
```

## Demo

```bash
cargo run --example bastion_demo
```

Output:

```
Gate: APPROVED (100% agreement, 3/3 agents)
Checkpoint: 1304b5fb
Verify: 3 checks, all passed: true

Gate: BLOCKED — dangerous pattern detected: drop table

Verify: DRIFT — hallucination marker: 'hypothetically' (confidence: 0.45)
Heal: Rollback — drift detected
Rollback: restored to 'pre-analysis' checkpoint

Gate: BLOCKED — $50000.00 exceeds limit $10000.00

Audit entries: 10 | Chain integrity: VERIFIED
Total actions: 4 | Approved: 2 | Blocked: 2 | Drift: 2 | Rollbacks: 1
```

## Core Primitives

| Primitive | What it does |
|-----------|-------------|
| `gate()` | Guardrails + multi-model consensus before any action |
| `checkpoint()` | Snapshot state before risky operations |
| `verify()` | Deterministic hallucination and drift detection |
| `rollback()` | Restore to a known-good checkpoint |
| `audit()` | SHA-256 hash-chained immutable logging |
| `observe()` | Metrics — cost, latency, approval/block rate |
| `heal()` | Decision tree — retry, escalate, rollback, or abort |

## Guardrails

Implement the `Guardrail` trait to add your own. Ships with:

| Guardrail | Domain | Behavior |
|-----------|--------|----------|
| `SpendingLimit` | Finance | Blocks transactions above threshold |
| `DangerousPatterns` | Coding | Catches `rm -rf /`, `DROP TABLE`, `eval()` |
| `MedicalDisclaimer` | Medical | Flags medical content for human review |
| `HumanInLoop` | Defense | Requires `human_approved: true` in context |

## Verification

Deterministic checks — no LLM call needed:

| Check | What it catches |
|-------|----------------|
| `NotEmpty` | Null/empty responses |
| `FileExists` | Agent claims a file exists but it doesn't |
| `ConfidenceThreshold` | Confidence dropped below threshold |
| `HallucinationCheck` | Hedging language ("hypothetically", "I would assume") |

## Self-Healing

When something fails, the healer follows a decision tree:

```
Attempt 1 timeout        → Retry
Attempt 2 verify fail    → Retry (simplified scope)
Same error twice         → Escalate (oscillation detected)
Drift detected           → Rollback to checkpoint
Guardrail blocked        → Escalate to human
Max retries exceeded     → Abort
```

## Audit Trail

Every decision is logged with SHA-256 hash chaining. Each entry includes the previous entry's hash — tamper with any entry and `verify_chain()` detects it.

```rust
let (valid, broken_at) = runtime.audit_log().verify_chain();
assert!(valid); // Full chain integrity verified
```

## Semantic Eyes

Knowledge graph integration layer. Memory-mapped binary graphs with typed edge traversal give safety primitives domain context for risk assessment and precedent lookup.

```rust
let eyes = SemanticEyes::load("./knowledge_graphs")?;
let risks = eyes.query_risks("transfer $50,000 to unknown vendor");
let evidence = eyes.find_evidence("OFAC sanctions compliance");
let path = eyes.trace_reasoning("transaction monitoring", "compliance");
```

Backed by mmap — scales to terabytes without loading into RAM. Bloom filters for sub-microsecond cluster relevance checks. CSR edge arrays for O(1) traversal.

```bash
cargo run --example semantic_demo
```

## Architecture

```
Your Agent Code
       │
       ▼
┌─────────────────────────────────────┐
│           BastionRuntime            │
│                                     │
│  gate() ──► Guardrails (fast)       │
│          ──► Consensus (parallel)   │
│          ──► Semantic Eyes (graph)   │
│                                     │
│  checkpoint() ──► MemoryStore       │
│  rollback()   ──► FileStore         │
│                                     │
│  verify() ──► Deterministic checks  │
│            ──► Graph evidence query  │
│                                     │
│  heal() ──► Decision tree           │
│          ──► Precedent lookup        │
│                                     │
│  audit() ──► SHA-256 hash chain     │
│  observe() ──► Live metrics         │
└─────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────┐
│  Binary Knowledge Graphs (mmap)     │
│  Bloom filters │ Term index │ CSR   │
│  Scales to TB  │ < 1GB RAM  │ O(1)  │
└─────────────────────────────────────┘
```

## Tests

```bash
cargo test
```

17 integration tests covering consensus strategies, guardrail enforcement, verification checks, checkpoint/rollback roundtrips, self-healing decisions, and audit chain integrity.

## License

MIT OR Apache-2.0
