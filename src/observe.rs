//! Real-time observability — track cost, latency, error rate, and drift across agents.

use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub total_actions: u64,
    pub approved: u64,
    pub blocked: u64,
    pub failed: u64,
    pub total_latency_ms: u64,
    pub avg_latency_ms: f64,
    pub total_cost_usd: f64,
    pub consensus_agreements: u64,
    pub consensus_failures: u64,
    pub drift_detections: u64,
    pub rollbacks: u64,
    pub verifications_passed: u64,
    pub verifications_failed: u64,
}

pub struct Metrics {
    inner: RwLock<MetricsInner>,
}

struct MetricsInner {
    total_actions: u64,
    approved: u64,
    blocked: u64,
    failed: u64,
    total_latency_ms: u64,
    total_cost_usd: f64,
    consensus_agreements: u64,
    consensus_failures: u64,
    drift_detections: u64,
    rollbacks: u64,
    verifications_passed: u64,
    verifications_failed: u64,
}

impl Metrics {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(MetricsInner {
                total_actions: 0,
                approved: 0,
                blocked: 0,
                failed: 0,
                total_latency_ms: 0,
                total_cost_usd: 0.0,
                consensus_agreements: 0,
                consensus_failures: 0,
                drift_detections: 0,
                rollbacks: 0,
                verifications_passed: 0,
                verifications_failed: 0,
            }),
        }
    }

    pub fn record_action(&self, approved: bool, latency_ms: u64, cost_usd: f64) {
        let mut m = self.inner.write().unwrap_or_else(|e| e.into_inner());
        m.total_actions += 1;
        m.total_latency_ms += latency_ms;
        m.total_cost_usd += cost_usd;
        if approved {
            m.approved += 1;
        } else {
            m.blocked += 1;
        }
    }

    pub fn record_consensus(&self, reached: bool) {
        let mut m = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if reached {
            m.consensus_agreements += 1;
        } else {
            m.consensus_failures += 1;
        }
    }

    pub fn record_drift(&self) {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .drift_detections += 1;
    }

    pub fn record_rollback(&self) {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .rollbacks += 1;
    }

    pub fn record_failure(&self) {
        self.inner.write().unwrap_or_else(|e| e.into_inner()).failed += 1;
    }

    pub fn record_verification(&self, passed: bool) {
        let mut m = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if passed {
            m.verifications_passed += 1;
        } else {
            m.verifications_failed += 1;
        }
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let m = self.inner.read().unwrap_or_else(|e| e.into_inner());
        MetricsSnapshot {
            total_actions: m.total_actions,
            approved: m.approved,
            blocked: m.blocked,
            failed: m.failed,
            total_latency_ms: m.total_latency_ms,
            avg_latency_ms: if m.total_actions > 0 {
                m.total_latency_ms as f64 / m.total_actions as f64
            } else {
                0.0
            },
            total_cost_usd: m.total_cost_usd,
            consensus_agreements: m.consensus_agreements,
            consensus_failures: m.consensus_failures,
            drift_detections: m.drift_detections,
            rollbacks: m.rollbacks,
            verifications_passed: m.verifications_passed,
            verifications_failed: m.verifications_failed,
        }
    }
}

/// Timer helper for measuring operation latency.
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}
