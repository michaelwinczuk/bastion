//! Semantic Eyes — gives Bastion agents the ability to understand their own
//! knowledge through typed graph traversal.
//!
//! Backed by memory-mapped binary graphs (scales to TB).
//! No JSON parsing at runtime. No RAM pressure.
//!
//! Integration points:
//! - `gate()` — query "has this action type caused failures before?"
//! - `verify()` — "what evidence supports/contradicts this result?"
//! - `heal()` — "what fixed this type of failure in the past?"
//! - `audit()` — reasoning paths attached to every logged decision
//!
//! Usage:
//! ```rust,ignore
//! let eyes = SemanticEyes::load("./knowledge_graphs")?;
//! let risks = eyes.query_risks("transfer $500 to unknown vendor");
//! let evidence = eyes.find_evidence("OFAC compliance check");
//! let precedent = eyes.find_precedent("transaction velocity limit exceeded");
//! ```

use std::collections::HashMap;
use std::path::Path;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};

// Edge type constants (matching graph_engine)
const SOLVES: u8 = 0;
const CAUSES: u8 = 1;
const REQUIRES: u8 = 2;
const ENABLES: u8 = 3;
const CONTRADICTS: u8 = 4;
const IMPROVES: u8 = 6;
const TRADEOFF_OF: u8 = 8;
const ALTERNATIVE_TO: u8 = 9;

/// A knowledge finding from the graph — typed, with provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeFinding {
    pub node_name: String,
    pub description: String,
    pub domain: String,
    pub relationship: String,
    pub source_cluster: String,
    pub relevance: f64,
}

/// Risk assessment from graph traversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub risk_level: String,
    pub factors: Vec<KnowledgeFinding>,
    pub mitigations: Vec<KnowledgeFinding>,
    pub contradictions: Vec<KnowledgeFinding>,
}

/// Reasoning path — how a decision was reached through the graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningPath {
    pub steps: Vec<ReasoningStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    pub node: String,
    pub edge_type: String,
    pub next_node: String,
}

/// Semantic Eyes — the knowledge layer for Bastion safety primitives.
pub struct SemanticEyes {
    /// Directory containing .graphbin + .bloom + index.jsonld
    graphs_dir: std::path::PathBuf,
    /// Bloom filter data per cluster (loaded once, tiny)
    bloom_cache: HashMap<String, Vec<u8>>,
    /// Index: term → cluster names
    term_index: HashMap<String, Vec<String>>,
}

impl SemanticEyes {
    /// Load from a directory of binary graph files.
    /// Only loads the index and bloom filters — graph data stays on disk (mmap).
    pub fn load(graphs_dir: &Path) -> Result<Self, String> {
        let index_path = graphs_dir.join("index.jsonld");
        let term_index: HashMap<String, Vec<String>> = if index_path.exists() {
            let content = std::fs::read_to_string(&index_path)
                .map_err(|e| format!("read index: {}", e))?;
            let index: Value = serde_json::from_str(&content)
                .map_err(|e| format!("parse index: {}", e))?;
            if let Some(obj) = index["term_to_clusters"].as_object() {
                obj.iter()
                    .filter_map(|(k, v)| {
                        v.as_array().map(|arr| {
                            (k.clone(), arr.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
                        })
                    })
                    .collect()
            } else {
                HashMap::new()
            }
        } else {
            HashMap::new()
        };

        // Cache bloom filters
        let mut bloom_cache = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(graphs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("bloom") {
                    if let Ok(data) = std::fs::read(&path) {
                        let cluster = path.file_stem().unwrap().to_string_lossy().to_string();
                        bloom_cache.insert(cluster, data);
                    }
                }
            }
        }

        Ok(Self {
            graphs_dir: graphs_dir.to_path_buf(),
            bloom_cache,
            term_index,
        })
    }

    /// Find relevant clusters for a query using the term index.
    fn relevant_clusters(&self, query: &str, max: usize) -> Vec<String> {
        let terms: Vec<String> = query.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| t.len() > 3)
            .map(|t| t.to_string())
            .collect();

        let mut hits: HashMap<String, usize> = HashMap::new();
        for term in &terms {
            if let Some(clusters) = self.term_index.get(term) {
                for c in clusters {
                    *hits.entry(c.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut sorted: Vec<(String, usize)> = hits.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.into_iter().take(max).map(|(c, _)| c).collect()
    }

    /// Open a binary graph for a cluster (mmap — instant, no RAM).
    fn open_graph(&self, cluster: &str) -> Option<BinaryGraphReader> {
        let path = self.graphs_dir.join(format!("{}.graphbin", cluster));
        if !path.exists() { return None; }
        let file = std::fs::File::open(&path).ok()?;
        let mmap = unsafe { memmap2::Mmap::map(&file).ok()? };
        if mmap.len() < 64 || &mmap[0..8] != b"SWRMGRPH" { return None; }

        let node_count = u32::from_le_bytes([mmap[12], mmap[13], mmap[14], mmap[15]]);
        let node_data_offset = u64::from_le_bytes([
            mmap[20], mmap[21], mmap[22], mmap[23], mmap[24], mmap[25], mmap[26], mmap[27]
        ]);
        let edge_csr_offset = u64::from_le_bytes([
            mmap[28], mmap[29], mmap[30], mmap[31], mmap[32], mmap[33], mmap[34], mmap[35]
        ]);
        let string_pool_offset = u64::from_le_bytes([
            mmap[36], mmap[37], mmap[38], mmap[39], mmap[40], mmap[41], mmap[42], mmap[43]
        ]);
        let index_offset = u64::from_le_bytes([
            mmap[44], mmap[45], mmap[46], mmap[47], mmap[48], mmap[49], mmap[50], mmap[51]
        ]);

        // Load term index for this cluster
        let index_data = &mmap[index_offset as usize..];
        let term_idx: HashMap<String, Vec<u32>> = serde_json::from_slice(index_data).unwrap_or_default();

        Some(BinaryGraphReader {
            mmap, cluster: cluster.to_string(),
            node_count, node_data_offset, edge_csr_offset, string_pool_offset, term_idx,
        })
    }

    // ── Public API: Safety Queries ──────────────────────────────────────

    /// Query risks for an action. Returns causes, contradictions, and tradeoffs
    /// found in the knowledge graph related to this action.
    pub fn query_risks(&self, action: &str) -> RiskAssessment {
        let clusters = self.relevant_clusters(action, 5);
        let mut factors = Vec::new();
        let mut mitigations = Vec::new();
        let mut contradictions = Vec::new();

        for cluster in &clusters {
            if let Some(graph) = self.open_graph(cluster) {
                let hits = graph.search(action);
                for (idx, _) in hits.iter().take(10) {
                    // What CAUSES problems related to this action?
                    for target in graph.follow(*idx, CAUSES) {
                        factors.push(graph.to_finding(target, cluster, "Causes"));
                    }
                    // What SOLVES/MITIGATES these problems?
                    for target in graph.follow(*idx, SOLVES) {
                        mitigations.push(graph.to_finding(target, cluster, "Solves"));
                    }
                    // What CONTRADICTS the assumptions?
                    for target in graph.follow(*idx, CONTRADICTS) {
                        contradictions.push(graph.to_finding(target, cluster, "Contradicts"));
                    }
                    // Tradeoffs
                    for target in graph.follow(*idx, TRADEOFF_OF) {
                        factors.push(graph.to_finding(target, cluster, "TradeoffOf"));
                    }
                }
            }
        }

        let risk_level = if !contradictions.is_empty() && contradictions.len() > mitigations.len() {
            "high"
        } else if !factors.is_empty() {
            "medium"
        } else {
            "low"
        }.to_string();

        RiskAssessment { risk_level, factors, mitigations, contradictions }
    }

    /// Find evidence supporting or contradicting a claim.
    pub fn find_evidence(&self, claim: &str) -> Vec<KnowledgeFinding> {
        let clusters = self.relevant_clusters(claim, 5);
        let mut evidence = Vec::new();

        for cluster in &clusters {
            if let Some(graph) = self.open_graph(cluster) {
                let hits = graph.search(claim);
                for (idx, hit_count) in hits.iter().take(15) {
                    let node = match graph.read_node(*idx) {
                        Some(n) => n,
                        None => continue,
                    };
                    if node.2.len() < 50 { continue; } // description too short

                    evidence.push(KnowledgeFinding {
                        node_name: node.1.clone(),
                        description: node.2.clone(),
                        domain: cluster.clone(),
                        relationship: "direct_match".into(),
                        source_cluster: cluster.clone(),
                        relevance: (*hit_count as f64 / 10.0).min(1.0),
                    });

                    // Also follow Enables — what does this evidence enable?
                    for target in graph.follow(*idx, ENABLES) {
                        evidence.push(graph.to_finding(target, cluster, "Enables"));
                    }
                }
            }
        }

        evidence.truncate(20);
        evidence
    }

    /// Find precedent — similar past knowledge about this type of situation.
    pub fn find_precedent(&self, situation: &str) -> Vec<KnowledgeFinding> {
        let clusters = self.relevant_clusters(situation, 5);
        let mut precedents = Vec::new();

        for cluster in &clusters {
            if let Some(graph) = self.open_graph(cluster) {
                let hits = graph.search(situation);
                for (idx, _) in hits.iter().take(10) {
                    // Follow AlternativeTo — what alternatives exist?
                    for target in graph.follow(*idx, ALTERNATIVE_TO) {
                        precedents.push(graph.to_finding(target, cluster, "AlternativeTo"));
                    }
                    // Follow Improves — what improves on this?
                    for target in graph.follow(*idx, IMPROVES) {
                        precedents.push(graph.to_finding(target, cluster, "Improves"));
                    }
                }
            }
        }

        precedents.truncate(15);
        precedents
    }

    /// Build a reasoning path — trace how knowledge connects from A to B.
    pub fn trace_reasoning(&self, from: &str, to: &str) -> Option<ReasoningPath> {
        let clusters = self.relevant_clusters(&format!("{} {}", from, to), 3);

        for cluster in &clusters {
            if let Some(graph) = self.open_graph(cluster) {
                let from_hits = graph.search(from);
                let to_hits = graph.search(to);

                if let (Some((from_idx, _)), Some((to_idx, _))) = (from_hits.first(), to_hits.first()) {
                    // BFS to find path
                    let path = graph.find_path(*from_idx, *to_idx, 5);
                    if !path.is_empty() {
                        let steps: Vec<ReasoningStep> = path.windows(2).map(|pair| {
                            let from_node = graph.read_node(pair[0]).map(|n| n.1).unwrap_or_default();
                            let to_node = graph.read_node(pair[1]).map(|n| n.1).unwrap_or_default();
                            ReasoningStep {
                                node: from_node,
                                edge_type: "connected".into(),
                                next_node: to_node,
                            }
                        }).collect();
                        return Some(ReasoningPath { steps });
                    }
                }
            }
        }

        None
    }

    /// Enrich an audit entry with graph-derived reasoning.
    /// Call this when logging a decision to attach knowledge provenance.
    pub fn enrich_audit(&self, action: &str) -> Value {
        let risks = self.query_risks(action);
        let evidence = self.find_evidence(action);

        json!({
            "@type": "SemanticContext",
            "risk_assessment": {
                "level": risks.risk_level,
                "factors": risks.factors.len(),
                "mitigations": risks.mitigations.len(),
                "contradictions": risks.contradictions.len(),
            },
            "evidence_found": evidence.len(),
            "top_evidence": evidence.iter().take(3).map(|e| json!({
                "name": e.node_name,
                "relationship": e.relationship,
                "domain": e.domain,
            })).collect::<Vec<_>>(),
        })
    }
}

// ─── Lightweight Binary Reader (self-contained, no swarm-core dependency) ──

struct BinaryGraphReader {
    mmap: memmap2::Mmap,
    cluster: String,
    node_count: u32,
    node_data_offset: u64,
    edge_csr_offset: u64,
    string_pool_offset: u64,
    term_idx: HashMap<String, Vec<u32>>,
}

impl BinaryGraphReader {
    /// Read node: returns (index, name, description)
    fn read_node(&self, index: u32) -> Option<(u32, String, String)> {
        if index >= self.node_count { return None; }
        let offset = self.node_data_offset as usize + (index as usize) * 32;
        if offset + 32 > self.mmap.len() { return None; }

        let entry = &self.mmap[offset..offset + 32];
        let name_off = u32::from_le_bytes([entry[3], entry[4], entry[5], entry[6]]);
        let name_len = u16::from_le_bytes([entry[7], entry[8]]);
        let desc_off = u32::from_le_bytes([entry[9], entry[10], entry[11], entry[12]]);
        let desc_len = u32::from_le_bytes([entry[13], entry[14], entry[15], entry[16]]);

        let name = self.read_string(name_off, name_len as u32);
        let desc = self.read_string(desc_off, desc_len);

        Some((index, name, desc))
    }

    fn read_string(&self, offset: u32, len: u32) -> String {
        let start = self.string_pool_offset as usize + offset as usize;
        let end = start + len as usize;
        if end > self.mmap.len() { return String::new(); }
        String::from_utf8_lossy(&self.mmap[start..end]).to_string()
    }

    /// Search by term — O(1) via inverted index.
    fn search(&self, query: &str) -> Vec<(u32, usize)> {
        let terms: Vec<String> = query.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|t| t.len() > 3)
            .map(|t| t.to_string())
            .collect();

        let mut hits: HashMap<u32, usize> = HashMap::new();
        for term in &terms {
            if let Some(indices) = self.term_idx.get(term) {
                for &idx in indices {
                    *hits.entry(idx).or_insert(0) += 1;
                }
            }
        }

        let mut sorted: Vec<(u32, usize)> = hits.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(30);
        sorted
    }

    /// Follow edges of a specific type from a node.
    fn follow(&self, node_index: u32, edge_type: u8) -> Vec<u32> {
        if node_index >= self.node_count { return Vec::new(); }
        let offset = self.node_data_offset as usize + (node_index as usize) * 32;
        if offset + 32 > self.mmap.len() { return Vec::new(); }

        let entry = &self.mmap[offset..offset + 32];
        let edge_start = u32::from_le_bytes([entry[17], entry[18], entry[19], entry[20]]);
        let edge_count = u16::from_le_bytes([entry[21], entry[22]]);

        let mut targets = Vec::new();
        for i in 0..edge_count as u32 {
            let e_offset = self.edge_csr_offset as usize + ((edge_start + i) as usize) * 12;
            if e_offset + 12 > self.mmap.len() { break; }
            let e_data = &self.mmap[e_offset..e_offset + 12];
            let target = u32::from_le_bytes([e_data[0], e_data[1], e_data[2], e_data[3]]);
            let etype = e_data[4];
            if etype == edge_type {
                targets.push(target);
            }
        }
        targets
    }

    /// BFS path finding between two nodes.
    fn find_path(&self, from: u32, to: u32, max_depth: usize) -> Vec<u32> {
        use std::collections::VecDeque;
        let mut queue = VecDeque::new();
        let mut visited: HashMap<u32, u32> = HashMap::new(); // node → parent
        queue.push_back(from);
        visited.insert(from, u32::MAX);

        while let Some(current) = queue.pop_front() {
            if current == to {
                // Reconstruct path
                let mut path = vec![to];
                let mut node = to;
                while let Some(&parent) = visited.get(&node) {
                    if parent == u32::MAX { break; }
                    path.push(parent);
                    node = parent;
                }
                path.reverse();
                return path;
            }

            // Follow all edge types
            let offset = self.node_data_offset as usize + (current as usize) * 32;
            if offset + 32 > self.mmap.len() { continue; }
            let entry = &self.mmap[offset..offset + 32];
            let edge_start = u32::from_le_bytes([entry[17], entry[18], entry[19], entry[20]]);
            let edge_count = u16::from_le_bytes([entry[21], entry[22]]);

            for i in 0..edge_count as u32 {
                let e_offset = self.edge_csr_offset as usize + ((edge_start + i) as usize) * 12;
                if e_offset + 12 > self.mmap.len() { break; }
                let target = u32::from_le_bytes([
                    self.mmap[e_offset], self.mmap[e_offset+1], self.mmap[e_offset+2], self.mmap[e_offset+3]
                ]);
                if !visited.contains_key(&target) {
                    visited.insert(target, current);
                    queue.push_back(target);
                    if visited.len() > 10000 { return Vec::new(); } // Safety cap
                }
            }

            if visited.len() > max_depth * 1000 { break; }
        }

        Vec::new() // No path found
    }

    fn to_finding(&self, node_index: u32, cluster: &str, relationship: &str) -> KnowledgeFinding {
        let (_, name, desc) = self.read_node(node_index).unwrap_or((0, String::new(), String::new()));
        KnowledgeFinding {
            node_name: name,
            description: if desc.len() > 300 { format!("{}...", &desc[..300]) } else { desc },
            domain: cluster.to_string(),
            relationship: relationship.to_string(),
            source_cluster: cluster.to_string(),
            relevance: 0.7,
        }
    }
}
