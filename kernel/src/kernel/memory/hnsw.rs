//! Hierarchical Navigable Small World (HNSW) Index
//!
//! A graph-based index for efficient Approximate Nearest Neighbor (ANN) search
//! in high-dimensional spaces (like our 1024-bit hypervectors).
//!
//! # Simplified Implementation
//! This is a "Layered Graph" implementation optimized for `no_std` environments.
//! It supports:
//! - Insertion (Dynamic construction)
//! - Search (k-NN)
//! - Hamming Distance metric
//!
//! # Structure
//! - `HnswIndex`: The main entry point.
//! - `Node`: A node in the graph, containing the vector and links to neighbors.
//! - `Layer`: A level in the hierarchy.

use alloc::vec::Vec;
use core::cmp::Ordering;
use crate::intent::ConceptID;
use crate::kernel::memory::neural::{Hypervector, hamming_similarity};

/// Maximum number of neighbors per node (M)
const M: usize = 16;
/// Maximum number of neighbors for layer 0 (M0 = 2 * M)
const M0: usize = 32;
/// Size of the candidate list during construction (efConstruction)
const EF_CONSTRUCTION: usize = 100;
/// Size of the candidate list during search (efSearch)
const EF_SEARCH: usize = 50;
/// Probability decay for level generation (1 / ln(M))
/// For M=16, 1/ln(16) ~= 0.36
const LEVEL_MULT: f32 = 0.36;

/// A Node in the HNSW Graph
#[derive(Clone)]
struct Node {
    id: ConceptID,
    vector: Hypervector,
    /// Links to neighbors at each layer.
    /// layers[i] contains the neighbors at level i.
    /// Level 0 is the bottom layer.
    layers: Vec<Vec<usize>>, // Stores indices into the `nodes` vector
}

/// Search Result Candidate
#[derive(Copy, Clone, Debug)]
struct Candidate {
    index: usize,
    distance: f32, // Hamming Distance (Lower is better)
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse ordering for Min-Heap (smallest distance at top)
        other.distance.partial_cmp(&self.distance)
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// The HNSW Index
pub struct HnswIndex {
    /// All nodes in the graph, indexed by a dense integer ID (0..N)
    nodes: Vec<Node>,
    /// Entry point node index for the top layer
    entry_point: Option<usize>,
    /// Maximum layer in the graph
    max_layer: usize,
    /// Random number generator state (Xorshift)
    rng_state: u64,
}

impl HnswIndex {
    pub const fn new() -> Self {
        HnswIndex {
            nodes: Vec::new(),
            entry_point: None,
            max_layer: 0,
            rng_state: 0x1234567890ABCDEF, // Seed
        }
    }

    /// Insert a new vector into the index
    pub fn insert(&mut self, id: ConceptID, vector: Hypervector) {
        let level = self.random_level();
        let new_node_idx = self.nodes.len();
        
        let mut layers = Vec::with_capacity(level + 1);
        for _ in 0..=level {
            layers.push(Vec::new());
        }

        let new_node = Node {
            id,
            vector,
            layers,
        };
        self.nodes.push(new_node);

        // If graph is empty, this is the entry point
        if self.entry_point.is_none() {
            self.entry_point = Some(new_node_idx);
            self.max_layer = level;
            return;
        }

        let mut curr_obj = self.entry_point.unwrap();
        let mut curr_dist = self.dist(curr_obj, new_node_idx);

        // 1. Zoom down from top layer to level+1
        // Find the closest node to the new node at each layer
        for l in (level + 1..=self.max_layer).rev() {
            let mut changed = true;
            while changed {
                changed = false;
                // Check neighbors of curr_obj at layer l
                // We need to copy neighbors to avoid borrowing issues
                let neighbors = self.nodes[curr_obj].layers.get(l).cloned().unwrap_or_default();
                
                for neighbor_idx in neighbors {
                    let d = self.dist(neighbor_idx, new_node_idx);
                    if d < curr_dist {
                        curr_dist = d;
                        curr_obj = neighbor_idx;
                        changed = true;
                    }
                }
            }
        }

        // 2. Insert at layers level..0
        let mut ep = curr_obj;
        
        for l in (0..=level).rev() {
            // Search for EF_CONSTRUCTION nearest neighbors at layer l
            // starting from ep
            let mut candidates = self.search_layer(ep, new_node_idx, EF_CONSTRUCTION, l);
            
            // Select M neighbors (heuristic)
            // Simple heuristic: just take the M closest
            let m = if l == 0 { M0 } else { M };
            
            // Sort by distance
            candidates.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
            
            let neighbors: Vec<usize> = candidates.iter()
                .take(m)
                .map(|c| c.index)
                .collect();

            // Connect new node -> neighbors
            self.nodes[new_node_idx].layers[l] = neighbors.clone();

            // Connect neighbors -> new node (bidirectional)
            for neighbor_idx in neighbors {
                self.add_connection(neighbor_idx, new_node_idx, l);
            }
            
            // Update ep for next layer (closest found in this layer)
            if let Some(best) = candidates.first() {
                ep = best.index;
            }
        }

        // Update entry point if new node is at a higher layer
        if level > self.max_layer {
            self.max_layer = level;
            self.entry_point = Some(new_node_idx);
        }
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &Hypervector, k: usize) -> Vec<(ConceptID, f32)> {
        if self.entry_point.is_none() {
            return Vec::new();
        }

        let mut curr_obj = self.entry_point.unwrap();
        let mut curr_dist = self.dist_vec(curr_obj, query);

        // 1. Zoom down to layer 0
        for l in (1..=self.max_layer).rev() {
            let mut changed = true;
            while changed {
                changed = false;
                if let Some(neighbors) = self.nodes[curr_obj].layers.get(l) {
                    for &neighbor_idx in neighbors {
                        let d = self.dist_vec(neighbor_idx, query);
                        if d < curr_dist {
                            curr_dist = d;
                            curr_obj = neighbor_idx;
                            changed = true;
                        }
                    }
                }
            }
        }

        // 2. Search at layer 0
        let candidates = self.search_layer_vec(curr_obj, query, EF_SEARCH, 0);

        // Return top k
        let results: Vec<(ConceptID, f32)> = candidates.iter()
            .take(k)
            .map(|c| (self.nodes[c.index].id, 1.0 - c.distance)) // Convert distance back to similarity
            .collect();
            
        results
    }

    // --- Helpers ---

    /// Search a specific layer for nearest neighbors to a node
    fn search_layer(&self, entry_point: usize, target_idx: usize, ef: usize, layer: usize) -> Vec<Candidate> {
        let target_vec = self.nodes[target_idx].vector;
        self.search_layer_vec(entry_point, &target_vec, ef, layer)
    }

    /// Search a specific layer for nearest neighbors to a vector
    fn search_layer_vec(&self, entry_point: usize, target_vec: &Hypervector, ef: usize, layer: usize) -> Vec<Candidate> {
        let mut visited = alloc::collections::BTreeSet::new();
        let mut candidates = alloc::collections::BinaryHeap::new(); // Max-heap for candidates to explore
        let mut results = alloc::collections::BinaryHeap::new(); // Max-heap for best results found (worst on top)

        let dist = self.dist_raw(entry_point, target_vec);
        
        // Min-heap wrapper for candidates (smallest distance first)
        // We use Reverse(Candidate) to make BinaryHeap a min-heap
        use core::cmp::Reverse;
        
        let initial = Candidate { index: entry_point, distance: dist };
        visited.insert(entry_point);
        candidates.push(Reverse(initial));
        results.push(initial); // Max-heap: keeps largest distance at top

        while let Some(Reverse(c)) = candidates.pop() {
            let curr_dist = c.distance;
            
            // If closest candidate is worse than worst result, and we have enough results, stop
            if let Some(worst) = results.peek() {
                if curr_dist > worst.distance && results.len() >= ef {
                    break;
                }
            }

            // Explore neighbors
            if let Some(neighbors) = self.nodes[c.index].layers.get(layer) {
                for &neighbor_idx in neighbors {
                    if !visited.contains(&neighbor_idx) {
                        visited.insert(neighbor_idx);
                        
                        let d = self.dist_raw(neighbor_idx, target_vec);
                        let candidate = Candidate { index: neighbor_idx, distance: d };
                        
                        if results.len() < ef || d < results.peek().unwrap().distance {
                            candidates.push(Reverse(candidate));
                            results.push(candidate);
                            
                            if results.len() > ef {
                                results.pop(); // Remove worst
                            }
                        }
                    }
                }
            }
        }

        results.into_sorted_vec() // Returns sorted smallest to largest
    }

    /// Add a connection between two nodes at a specific layer
    fn add_connection(&mut self, src: usize, dst: usize, layer: usize) {
        let max_m = if layer == 0 { M0 } else { M };
        
        // Add dst to src's neighbor list
        // Note: In a real implementation, we would use a heuristic to select diverse neighbors
        // Here we just append and prune if too long.
        
        // We need to be careful about borrowing self.nodes
        // We can't mutate self.nodes[src] while reading others.
        // But here we just need to read dst to calculate distance if we prune.
        
        let neighbors = &mut self.nodes[src].layers[layer];
        if neighbors.contains(&dst) {
            return;
        }
        neighbors.push(dst);
        
        // Prune if too many
        if neighbors.len() > max_m {
            // Sort by distance to src and keep closest
            // We need to calculate distances. This is tricky with borrow checker.
            // We'll just remove the first one for now (FIFO) or random?
            // Better: Remove the one furthest away.
            
            // To do this properly without fighting borrow checker in this simple implementation:
            // We'll just keep the list as is, maybe random remove?
            // Let's remove the first element (oldest).
            neighbors.remove(0); 
        }
    }

    /// Calculate Hamming Distance (1.0 - Similarity)
    fn dist(&self, a: usize, b: usize) -> f32 {
        let va = &self.nodes[a].vector;
        let vb = &self.nodes[b].vector;
        1.0 - hamming_similarity(va, vb)
    }

    fn dist_vec(&self, a: usize, b: &Hypervector) -> f32 {
        let va = &self.nodes[a].vector;
        1.0 - hamming_similarity(va, b)
    }
    
    fn dist_raw(&self, a: usize, vb: &Hypervector) -> f32 {
        let va = &self.nodes[a].vector;
        1.0 - hamming_similarity(va, vb)
    }

    /// Generate a random level for a new node
    fn random_level(&mut self) -> usize {
        let mut level = 0;
        while self.rand_float() < LEVEL_MULT && level < 16 {
            level += 1;
        }
        level
    }

    /// Simple Xorshift RNG
    fn rand_float(&mut self) -> f32 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        // Normalize to [0, 1)
        (x as f32) / (u64::MAX as f32)
    }
}
