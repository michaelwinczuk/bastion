//! # Bastion — The Production Kernel for Agentic AI
//!
//! Every agent system needs these primitives to run safely in production:
//!
//! | Primitive | What it does |
//! |-----------|-------------|
//! | `gate()` | Multi-model consensus before any action |
//! | `checkpoint()` | Snapshot state before risky operations |
//! | `verify()` | Deterministic validation that an action produced correct results |
//! | `rollback()` | Restore to a known-good checkpoint |
//! | `audit()` | Immutable, tamper-evident logging of every decision |
//! | `observe()` | Real-time metrics — cost, latency, error rate, drift |
//! | `heal()` | Self-healing decision tree — retry, escalate, or abort |
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use bastion_core::prelude::*;
//!
//! // Create a runtime with 3 safety agents
//! let mut runtime = BastionRuntime::builder()
//!     .add_agent(MyAgent { model: "sonnet".into() })
//!     .add_agent(MyAgent { model: "gpt-4o".into() })
//!     .add_agent(MyAgent { model: "haiku".into() })
//!     .consensus(ConsensusStrategy::Majority)
//!     .build();
//!
//! // Gate an action through consensus
//! let outcome = runtime.gate("transfer $500 to vendor").await?;
//! // Checkpoint before execution
//! let cp = runtime.checkpoint("pre-transfer").await?;
//! // Verify after execution
//! let valid = runtime.verify(&outcome, &actual_result).await?;
//! // If invalid, rollback
//! if !valid { runtime.rollback(&cp).await?; }
//! ```

pub mod audit;
pub mod checkpoint;
pub mod consensus;
pub mod error;
pub mod guardrails;
pub mod heal;
pub mod observe;
pub mod prelude;
pub mod runtime;
pub mod semantic_eyes;
pub mod verify;

pub use audit::{AuditEntry, AuditLog, Severity};
pub use checkpoint::{Checkpoint, CheckpointStore, FileStore, MemoryStore};
pub use consensus::{Agent, AgentResponse, ConsensusConfig, ConsensusResult, ConsensusStrategy};
pub use error::{BastionError, BastionResult};
pub use guardrails::{Guardrail, GuardrailResult, GuardrailVerdict};
pub use heal::{HealAction, HealDecision};
pub use observe::{Metrics, MetricsSnapshot};
pub use runtime::BastionRuntime;
pub use semantic_eyes::SemanticEyes;
pub use verify::{Verification, VerifyResult};
