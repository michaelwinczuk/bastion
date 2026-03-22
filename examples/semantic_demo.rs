//! Semantic Eyes Demo — Agents that understand their own knowledge.
//!
//! Shows the difference between "blind" safety checks (pattern matching)
//! and "semantic" safety checks (knowledge graph traversal).
//!
//! Run: cargo run --example semantic_demo
//!
//! This demo:
//! 1. Builds a sample knowledge graph with typed relationships
//! 2. Converts it to binary format (mmap-ready)
//! 3. Loads SemanticEyes from the binary graph
//! 4. Demonstrates: risk queries, evidence finding, precedent lookup, reasoning traces
//! 5. Shows how audit entries get enriched with graph provenance

use bastion_core::semantic_eyes::SemanticEyes;
use std::collections::HashMap;
use std::path::Path;

fn main() {
    println!();
    println!("  ╔═══════════════════════════════════════════════════════════╗");
    println!("  ║  SEMANTIC EYES — Agents That Understand Their Knowledge  ║");
    println!("  ╚═══════════════════════════════════════════════════════════╝");
    println!();

    // ── Step 1: Build a sample knowledge graph ──
    println!("  ── Step 1: Building sample knowledge graph ──");
    let tmp_dir = std::env::temp_dir().join("bastion_semantic_demo");
    std::fs::create_dir_all(&tmp_dir).unwrap();

    build_sample_graph(&tmp_dir);
    println!("     Graph built at: {}", tmp_dir.display());
    println!();

    // ── Step 2: Load Semantic Eyes ──
    println!("  ── Step 2: Loading Semantic Eyes (mmap — instant, no RAM) ──");
    let eyes = match SemanticEyes::load(&tmp_dir) {
        Ok(e) => {
            println!("     Loaded successfully");
            e
        }
        Err(e) => {
            println!("     Could not load graph: {}", e);
            println!("     (This is expected if no binary graphs exist yet)");
            println!();
            println!("  To use with real data, run the binary graph builder:");
            println!("    cargo run --release --bin build_binary_graphs");
            println!();
            demo_without_graph();
            return;
        }
    };
    println!();

    // ── Step 3: Risk Assessment ──
    println!("  ── Step 3: Risk Assessment ──");
    println!("  Query: 'transfer $50,000 to unknown vendor'");
    let risks = eyes.query_risks("transfer $50,000 to unknown vendor");
    println!("     Risk level: {}", risks.risk_level);
    println!("     Factors found: {}", risks.factors.len());
    println!("     Mitigations found: {}", risks.mitigations.len());
    println!("     Contradictions found: {}", risks.contradictions.len());
    for factor in risks.factors.iter().take(3) {
        println!("     ├─ [{}] {} → {}", factor.relationship, factor.node_name,
            &factor.description[..factor.description.len().min(80)]);
    }
    for mitigation in risks.mitigations.iter().take(2) {
        println!("     ├─ [{}] {}", mitigation.relationship, mitigation.node_name);
    }
    println!();

    // ── Step 4: Evidence Finding ──
    println!("  ── Step 4: Evidence Finding ──");
    println!("  Query: 'OFAC sanctions compliance'");
    let evidence = eyes.find_evidence("OFAC sanctions compliance");
    println!("     Evidence found: {}", evidence.len());
    for e in evidence.iter().take(3) {
        println!("     ├─ [{}] {} ({})",
            e.relationship, e.node_name, e.domain);
    }
    println!();

    // ── Step 5: Precedent Lookup ──
    println!("  ── Step 5: Precedent Lookup ──");
    println!("  Query: 'transaction velocity limit exceeded'");
    let precedent = eyes.find_precedent("transaction velocity limit exceeded");
    println!("     Precedents found: {}", precedent.len());
    for p in precedent.iter().take(3) {
        println!("     ├─ [{}] {} → {}",
            p.relationship, p.node_name,
            &p.description[..p.description.len().min(60)]);
    }
    println!();

    // ── Step 6: Reasoning Trace ──
    println!("  ── Step 6: Reasoning Trace ──");
    println!("  Query: path from 'transaction monitoring' to 'compliance'");
    match eyes.trace_reasoning("transaction monitoring", "compliance") {
        Some(path) => {
            println!("     Path found ({} steps):", path.steps.len());
            for step in &path.steps {
                println!("     {} ──[{}]──► {}", step.node, step.edge_type, step.next_node);
            }
        }
        None => println!("     No direct path found (graph may be too sparse)"),
    }
    println!();

    // ── Step 7: Audit Enrichment ──
    println!("  ── Step 7: Audit Enrichment ──");
    println!("  Action: 'execute high-value trade AAPL 10000 shares'");
    let context = eyes.enrich_audit("execute high-value trade AAPL 10000 shares");
    println!("     Semantic context attached to audit entry:");
    println!("     {}", serde_json::to_string_pretty(&context).unwrap_or_default());
    println!();

    // ── Summary ──
    println!("  ╔═══════════════════════════════════════════════════════════╗");
    println!("  ║  WITHOUT Semantic Eyes:                                  ║");
    println!("  ║    gate() → pattern match on action string              ║");
    println!("  ║    verify() → check if output is empty/hallucinated     ║");
    println!("  ║    heal() → retry blindly, hope for the best            ║");
    println!("  ║    audit() → log what happened, nothing about WHY       ║");
    println!("  ║                                                         ║");
    println!("  ║  WITH Semantic Eyes:                                    ║");
    println!("  ║    gate() → traverse Causes/Contradicts before acting   ║");
    println!("  ║    verify() → query evidence that supports/refutes      ║");
    println!("  ║    heal() → look up precedent fixes via AlternativeTo   ║");
    println!("  ║    audit() → attach full reasoning provenance           ║");
    println!("  ║                                                         ║");
    println!("  ║  Same deterministic core. Same speed. Now with eyes.    ║");
    println!("  ╚═══════════════════════════════════════════════════════════╝");
    println!();

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

/// Demo output when no graph files are available.
fn demo_without_graph() {
    println!("  ── Demo (no graph data — showing API surface) ──");
    println!();
    println!("  SemanticEyes provides these queries for every safety primitive:");
    println!();
    println!("  eyes.query_risks(action)");
    println!("    → Traverses Causes/Contradicts/TradeoffOf edges");
    println!("    → Returns: risk_level, factors, mitigations, contradictions");
    println!();
    println!("  eyes.find_evidence(claim)");
    println!("    → Searches graph + follows Enables edges");
    println!("    → Returns: supporting evidence with provenance");
    println!();
    println!("  eyes.find_precedent(situation)");
    println!("    → Follows AlternativeTo/Improves edges");
    println!("    → Returns: what was tried before, what worked");
    println!();
    println!("  eyes.trace_reasoning(from, to)");
    println!("    → BFS path through typed edges");
    println!("    → Returns: step-by-step reasoning chain");
    println!();
    println!("  eyes.enrich_audit(action)");
    println!("    → Combines risk + evidence into audit context");
    println!("    → Returns: JSON-LD semantic context for audit entry");
    println!();
    println!("  All backed by memory-mapped binary graphs.");
    println!("  Scales to terabytes. Under 1GB RAM. Sub-millisecond queries.");
}

/// Build a small sample knowledge graph for the demo.
/// Creates index.jsonld + sample.graphbin + sample.bloom files.
fn build_sample_graph(dir: &Path) {
    // Build a minimal index
    let mut term_to_clusters: HashMap<String, Vec<String>> = HashMap::new();
    let terms = vec![
        "transaction", "transfer", "compliance", "sanctions", "ofac",
        "velocity", "limit", "monitoring", "trade", "risk",
        "fraud", "aml", "kyc", "wallet", "audit",
    ];
    for term in &terms {
        term_to_clusters.insert(term.to_string(), vec!["sample".to_string()]);
    }

    let index = serde_json::json!({
        "term_to_clusters": term_to_clusters,
        "cluster_stats": {
            "sample": {
                "node_count": 20,
                "edge_count": 25,
                "semantic_edge_count": 20,
                "graph_file": "sample.jsonld"
            }
        },
        "cross_cluster_edges": {}
    });
    std::fs::write(dir.join("index.jsonld"), serde_json::to_string_pretty(&index).unwrap()).unwrap();

    // Build a small binary graph with typed edges
    // Nodes: compliance concepts with real relationships
    let nodes = vec![
        ("Transaction Monitoring", "System that watches for suspicious transaction patterns in real-time, flagging velocity spikes, unusual amounts, and new counterparty interactions for compliance review"),
        ("OFAC Sanctions Screening", "Checks counterparty addresses and names against the Office of Foreign Assets Control sanctions list before allowing any transfer or trade execution"),
        ("Velocity Limits", "Rate limiting mechanism that caps the number and total value of transactions within a time window to prevent rapid fund extraction or wash trading"),
        ("AML Compliance", "Anti-money laundering framework combining transaction monitoring, sanctions screening, KYC verification, and suspicious activity reporting into a unified compliance pipeline"),
        ("KYC Verification", "Know Your Customer identity verification process that validates counterparty identity before establishing business relationships or executing high-value transactions"),
        ("Fraud Detection", "Machine learning and rule-based system that identifies potentially fraudulent transactions by analyzing patterns, anomalies, and behavioral deviations from established baselines"),
        ("Risk Scoring", "Quantitative risk assessment that assigns numerical scores to transactions and counterparties based on multiple factors including geography, amount, velocity, and historical behavior"),
        ("Wallet Security", "Cryptographic key management and transaction signing infrastructure that protects wallet operations from unauthorized access and ensures non-repudiation of signed transactions"),
        ("Trade Surveillance", "Post-trade monitoring system that detects market manipulation patterns including wash trading, spoofing, and layering across multiple venues and time horizons"),
        ("Regulatory Reporting", "Automated generation and submission of regulatory filings including SARs, CTRs, and periodic compliance reports to relevant authorities"),
        ("High Value Transfer Risk", "Elevated risk category triggered when transaction amounts exceed configurable thresholds, requiring additional verification steps and senior approval before execution"),
        ("Unknown Counterparty Risk", "Risk factor activated when the receiving party has no established transaction history, triggering enhanced due diligence and reduced transaction limits"),
        ("Sanctions Violation", "Critical compliance breach that occurs when a transaction involves a sanctioned entity, requiring immediate blocking, reporting, and investigation"),
        ("Transaction Rollback", "Recovery mechanism that reverses a completed transaction when post-execution checks reveal compliance violations, fraud indicators, or counterparty risk issues"),
        ("Rate Limit Exceeded", "System state triggered when transaction frequency or cumulative value exceeds configured velocity thresholds within the monitoring window"),
        ("Enhanced Due Diligence", "Intensified verification process applied to high-risk transactions, new counterparties, or PEP-associated accounts requiring additional documentation and senior sign-off"),
        ("Compliance Audit Trail", "Immutable cryptographic log of all compliance decisions, screening results, and override actions that provides regulators with complete transaction provenance"),
        ("Manual Review Queue", "Escalation pathway for transactions that automated systems cannot decisively approve or reject, routing to human compliance officers for judgment"),
        ("Counterparty Risk Assessment", "Evaluation of counterparty creditworthiness, regulatory status, and historical behavior used to set transaction limits and approval requirements"),
        ("Real-time Alert System", "Notification infrastructure that immediately alerts compliance officers when high-risk transactions, sanctions hits, or anomalous patterns are detected"),
    ];

    // Edges: typed relationships between compliance concepts
    let edges: Vec<(usize, usize, u8)> = vec![
        // Causes
        (10, 14, 1), // High Value Transfer Risk causes Rate Limit Exceeded
        (11, 15, 1), // Unknown Counterparty Risk causes Enhanced Due Diligence
        (12, 13, 1), // Sanctions Violation causes Transaction Rollback
        (14, 17, 1), // Rate Limit Exceeded causes Manual Review Queue
        // Solves
        (0, 5, 0),   // Transaction Monitoring solves Fraud Detection
        (1, 12, 0),  // OFAC Screening solves Sanctions Violation
        (2, 14, 0),  // Velocity Limits solves Rate Limit Exceeded
        (4, 11, 0),  // KYC Verification solves Unknown Counterparty Risk
        (6, 10, 0),  // Risk Scoring solves High Value Transfer Risk
        // Enables
        (3, 9, 3),   // AML Compliance enables Regulatory Reporting
        (0, 6, 3),   // Transaction Monitoring enables Risk Scoring
        (4, 18, 3),  // KYC enables Counterparty Risk Assessment
        (16, 9, 3),  // Audit Trail enables Regulatory Reporting
        // Contradicts
        (10, 2, 4),  // High Value Transfer Risk contradicts Velocity Limits (tension)
        (11, 7, 4),  // Unknown Counterparty Risk contradicts Wallet Security
        // Requires
        (1, 4, 2),   // OFAC Screening requires KYC
        (3, 0, 2),   // AML Compliance requires Transaction Monitoring
        (9, 16, 2),  // Regulatory Reporting requires Audit Trail
        // Improves
        (5, 0, 6),   // Fraud Detection improves Transaction Monitoring
        (19, 17, 6), // Alert System improves Manual Review Queue
        // AlternativeTo
        (15, 17, 9), // Enhanced Due Diligence alternative to Manual Review
        (8, 0, 9),   // Trade Surveillance alternative to Transaction Monitoring
        // TradeoffOf
        (2, 10, 8),  // Velocity Limits tradeoff of High Value Transfer Risk
    ];

    // Write binary graph
    write_binary_graph(dir, "sample", &nodes, &edges);
    println!("     {} nodes, {} edges ({} semantic)",
        nodes.len(), edges.len(), edges.iter().filter(|(_, _, t)| *t != 5).count());
}

fn write_binary_graph(dir: &Path, cluster: &str, nodes: &[(&str, &str)], edges: &[(usize, usize, u8)]) {
    let mut string_pool: Vec<u8> = Vec::new();
    let mut node_entries: Vec<[u8; 32]> = Vec::new();
    let mut edge_entries: Vec<u8> = Vec::new();
    let mut term_index: HashMap<String, Vec<u32>> = HashMap::new();

    // Build bloom filter
    let mut bloom_bits = vec![0u64; 128]; // 1KB bloom
    let bloom_hashes = 7usize;

    // Build node entries
    let mut edge_offset = 0u32;
    for (i, (name, desc)) in nodes.iter().enumerate() {
        let name_off = string_pool.len() as u32;
        string_pool.extend_from_slice(name.as_bytes());
        let name_len = name.len() as u16;

        let desc_off = string_pool.len() as u32;
        string_pool.extend_from_slice(desc.as_bytes());
        let desc_len = desc.len() as u32;

        // Count edges for this node
        let node_edges: Vec<&(usize, usize, u8)> = edges.iter().filter(|(from, _, _)| *from == i).collect();
        let edge_count = node_edges.len() as u16;

        let mut entry = [0u8; 32];
        entry[0] = 2; // Concept
        entry[3..7].copy_from_slice(&name_off.to_le_bytes());
        entry[7..9].copy_from_slice(&name_len.to_le_bytes());
        entry[9..13].copy_from_slice(&desc_off.to_le_bytes());
        entry[13..17].copy_from_slice(&desc_len.to_le_bytes());
        entry[17..21].copy_from_slice(&edge_offset.to_le_bytes());
        entry[21..23].copy_from_slice(&edge_count.to_le_bytes());
        let conf: f32 = 0.9;
        entry[23..27].copy_from_slice(&conf.to_le_bytes());
        node_entries.push(entry);

        // Build edges for this node
        for (_, to, etype) in &node_edges {
            edge_entries.extend_from_slice(&(*to as u32).to_le_bytes());
            edge_entries.push(*etype);
            let w: f32 = 0.8;
            edge_entries.extend_from_slice(&w.to_le_bytes());
            edge_entries.extend_from_slice(&[0u8; 3]); // detail placeholder
        }
        edge_offset += edge_count as u32;

        // Term index
        let text = format!("{} {}", name.to_lowercase(), desc.to_lowercase());
        for word in text.split(|c: char| !c.is_alphanumeric()) {
            if word.len() > 3 {
                term_index.entry(word.to_string()).or_default().push(i as u32);
                // Bloom insert
                for seed in 0..bloom_hashes {
                    let mut h: u64 = 14695981039346656037u64.wrapping_add(seed as u64 * 2654435761);
                    for b in word.as_bytes() { h ^= *b as u64; h = h.wrapping_mul(1099511628211); }
                    let idx = h as usize % (bloom_bits.len() * 64);
                    bloom_bits[idx / 64] |= 1 << (idx % 64);
                }
            }
        }
    }

    let index_bytes = serde_json::to_vec(&term_index).unwrap();

    // Calculate offsets
    let node_data_offset = 64u64;
    let edge_csr_offset = node_data_offset + (node_entries.len() * 32) as u64;
    let string_pool_offset = edge_csr_offset + edge_entries.len() as u64;
    let index_offset = string_pool_offset + string_pool.len() as u64;

    // Write binary file
    let mut file = std::fs::File::create(dir.join(format!("{}.graphbin", cluster))).unwrap();
    use std::io::Write;

    file.write_all(b"SWRMGRPH").unwrap();
    file.write_all(&1u32.to_le_bytes()).unwrap(); // version
    file.write_all(&(nodes.len() as u32).to_le_bytes()).unwrap();
    file.write_all(&(edges.len() as u32).to_le_bytes()).unwrap();
    file.write_all(&node_data_offset.to_le_bytes()).unwrap();
    file.write_all(&edge_csr_offset.to_le_bytes()).unwrap();
    file.write_all(&string_pool_offset.to_le_bytes()).unwrap();
    file.write_all(&index_offset.to_le_bytes()).unwrap();
    file.write_all(&[0u8; 12]).unwrap(); // pad to 64

    for entry in &node_entries { file.write_all(entry).unwrap(); }
    file.write_all(&edge_entries).unwrap();
    file.write_all(&string_pool).unwrap();
    file.write_all(&index_bytes).unwrap();

    // Write bloom
    let mut bloom_bytes = Vec::new();
    bloom_bytes.extend_from_slice(&(bloom_bits.len() as u32).to_le_bytes());
    bloom_bytes.extend_from_slice(&(bloom_hashes as u32).to_le_bytes());
    for word in &bloom_bits { bloom_bytes.extend_from_slice(&word.to_le_bytes()); }
    std::fs::write(dir.join(format!("{}.bloom", cluster)), bloom_bytes).unwrap();
}
