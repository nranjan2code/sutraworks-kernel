//! Deadlock Detection
//!
//! Builds wait-for graphs and detects circular dependencies between tasks using Tarjan's algorithm.

use alloc::vec::Vec;
use alloc::collections::BTreeMap;

/// Detect circular wait (deadlock) among tasks
///
/// Returns Some(task_ids) if deadlock detected, None otherwise
pub fn detect_circular_wait() -> Option<Vec<u64>> {
    // Build wait-for graph from current system state
    let graph = build_wait_graph();
    
    // Find cycles using Tarjan's algorithm
    let cycles = graph.find_cycles();
    
    // Return first cycle found (if any)
    cycles.into_iter().next()
}

/// Build wait-for graph from current lock state
fn build_wait_graph() -> WaitGraph {
    // Get the real wait graph from the lock registry
    let raw_graph = crate::kernel::sync::spinlock::get_wait_graph();
    
    let mut graph = WaitGraph::new();
    for (waiter, holders) in raw_graph {
        for holder in holders {
            graph.add_edge(waiter, holder);
        }
    }
    
    graph
}

/// Wait-for graph structure using adjacency list
struct WaitGraph {
    /// Adjacency list: task_id -> list of tasks it's waiting for
    edges: BTreeMap<u64, Vec<u64>>,
}

impl WaitGraph {
    fn new() -> Self {
        WaitGraph {
            edges: BTreeMap::new(),
        }
    }
    
    /// Add edge: from_task is waiting for to_task
    #[allow(dead_code)]
    pub fn add_edge(&mut self, from_task: u64, to_task: u64) {
        self.edges.entry(from_task).or_default().push(to_task);
    }
    
    /// Find cycles using Tarjan's strongly connected components algorithm
    ///
    /// Returns all cycles found (each cycle is a Vec of task IDs)
    fn find_cycles(&self) -> Vec<Vec<u64>> {
        let mut index = 0;
        let mut stack = Vec::new();
        let mut indices = BTreeMap::new();
        let mut lowlinks = BTreeMap::new();
        let mut on_stack = BTreeMap::new();
        let mut sccs = Vec::new();
        
        // Run Tarjan's algorithm on each unvisited node
        for &node in self.edges.keys() {
            if !indices.contains_key(&node) {
                self.strongconnect(
                    node,
                    &mut index,
                    &mut stack,
                    &mut indices,
                    &mut lowlinks,
                    &mut on_stack,
                    &mut sccs,
                );
            }
        }
        
        // Filter out trivial SCCs (single nodes with no self-loop)
        // Only return actual cycles (size > 1 or self-loop)
        sccs.into_iter()
            .filter(|scc| {
                if scc.len() > 1 {
                    true // Multi-node cycle
                } else if scc.len() == 1 {
                    // Check for self-loop
                    let node = scc[0];
                    self.edges.get(&node)
                        .map(|neighbors| neighbors.contains(&node))
                        .unwrap_or(false)
                } else {
                    false
                }
            })
            .collect()
    }
    
    /// Tarjan's strongly connected components algorithm (recursive)
    #[allow(clippy::too_many_arguments)]
    fn strongconnect(
        &self,
        v: u64,
        index: &mut usize,
        stack: &mut Vec<u64>,
        indices: &mut BTreeMap<u64, usize>,
        lowlinks: &mut BTreeMap<u64, usize>,
        on_stack: &mut BTreeMap<u64, bool>,
        sccs: &mut Vec<Vec<u64>>,
    ) {
        // Set the depth index for v to the smallest unused index
        indices.insert(v, *index);
        lowlinks.insert(v, *index);
        *index += 1;
        stack.push(v);
        on_stack.insert(v, true);
        
        // Consider successors of v
        if let Some(neighbors) = self.edges.get(&v) {
            for &w in neighbors {
                if !indices.contains_key(&w) {
                    // Successor w has not yet been visited; recurse on it
                    self.strongconnect(w, index, stack, indices, lowlinks, on_stack, sccs);
                    let low_v = *lowlinks.get(&v).expect("v was inserted above");
                    let low_w = *lowlinks.get(&w).expect("w visited in recurse");
                    lowlinks.insert(v, low_v.min(low_w));
                } else if *on_stack.get(&w).unwrap_or(&false) {
                    // Successor w is in stack S and hence in the current SCC
                    let low_v = *lowlinks.get(&v).expect("v inserted above");
                    let index_w = *indices.get(&w).expect("w on stack so indexed");
                    lowlinks.insert(v, low_v.min(index_w));
                }
            }
        }
        
        // If v is a root node, pop the stack and generate an SCC
        if lowlinks.get(&v) == indices.get(&v) {
            let mut scc = Vec::new();
            loop {
                let w = stack.pop().expect("loop guards non-empty");
                on_stack.insert(w, false);
                scc.push(w);
                if w == v {
                    break;
                }
            }
            sccs.push(scc);
        }
    }
}

/// Check if task is blocked on a lock
pub fn is_task_blocked(_task_id: usize) -> bool {
    // This would query the scheduler for task state
    // Returns false as we don't have task state tracking yet
    // Infrastructure is ready for when scheduler exposes this
    false
}

#[cfg(test)]
mod tests {
    use alloc::vec;
    use super::{WaitGraph, detect_circular_wait};

    #[test]
    fn test_no_deadlock_on_empty_system() {
        let result = detect_circular_wait();
        assert!(result.is_none());
    }

    #[test]
    fn test_empty_graph_no_cycles() {
        let graph = WaitGraph::new();
        let cycles = graph.find_cycles();
        assert_eq!(cycles.len(), 0);
    }
    
    #[test]
    fn test_simple_cycle_detection() {
        let mut graph = WaitGraph::new();
        // Create cycle: 1 -> 2 -> 3 -> 1
        graph.add_edge(1, 2);
        graph.add_edge(2, 3);
        graph.add_edge(3, 1);
        
        let cycles = graph.find_cycles();
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0].len(), 3);
    }
    
    #[test]
    fn test_self_loop_detection() {
        let mut graph = WaitGraph::new();
        // Task 1 waiting for itself (self-loop)
        graph.add_edge(1, 1);
        
        let cycles = graph.find_cycles();
        assert_eq!(cycles.len(), 1);
        assert_eq!(cycles[0], vec![1]);
    }
    
    #[test]
    fn test_no_cycle_in_dag() {
        let mut graph = WaitGraph::new();
        // Create DAG: 1 -> 2 -> 3
        graph.add_edge(1, 2);
        graph.add_edge(2, 3);
        
        let cycles = graph.find_cycles();
        assert_eq!(cycles.len(), 0);
    }
}
