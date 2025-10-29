use super::LayeredLayoutError;
use petgraph::algo::toposort;
use petgraph::graphmap::DiGraphMap;
use petgraph::visit::{IntoNeighborsDirected, IntoNodeIdentifiers};
use petgraph::Direction;
use std::collections::HashMap;
use std::hash::Hash;

/// Assign layers to nodes based on topological order
///
/// Uses a two-pass approach to minimize edge lengths:
/// - First pass: assign each node to the layer after its predecessors
/// - Second pass: move nodes closer to their successors when possible
pub(crate) fn assign_layers<G>(graph: &G) -> Result<Vec<Vec<G::NodeId>>, LayeredLayoutError<G::NodeId>>
where
    G: IntoNodeIdentifiers + IntoNeighborsDirected,
    G::NodeId: Copy + Ord + Hash + std::fmt::Debug,
{
    // Convert to DiGraphMap for toposort
    let mut petgraph = DiGraphMap::new();
    for node in graph.node_identifiers() {
        petgraph.add_node(node);
    }
    for node in graph.node_identifiers() {
        for succ in graph.neighbors_directed(node, Direction::Outgoing) {
            petgraph.add_edge(node, succ, ());
        }
    }

    let topo_order = toposort(&petgraph, None)
        .map_err(|cycle| LayeredLayoutError::GraphHasCycle(cycle.node_id()))?;
    let mut layer_map: HashMap<_, usize> = HashMap::new();

    // First pass: forward, assign each node to layer after its predecessors
    for &node in &topo_order {
        let max_pred_layer = graph
            .neighbors_directed(node, Direction::Incoming)
            .map(|pred| layer_map.get(&pred).unwrap_or(&0) + 1)
            .max()
            .unwrap_or(0);
        layer_map.insert(node, max_pred_layer);
    }

    // Second pass: backward, move nodes closer to their successors
    for &node in topo_order.iter().rev() {
        let layer = *layer_map.get(&node).unwrap_or(&0);
        let min_succ_layer = graph
            .neighbors_directed(node, Direction::Outgoing)
            .map(|succ| *layer_map.get(&succ).unwrap_or(&0))
            .min()
            .unwrap_or(0);

        if min_succ_layer > layer + 1 {
            layer_map.insert(node, min_succ_layer.saturating_sub(1));
        }
    }

    // Group nodes by layer
    let max_layer = *layer_map.values().max().unwrap_or(&0);
    let mut layers = vec![Vec::new(); max_layer + 1];
    for (node, &layer) in &layer_map {
        layers[layer].push(*node);
    }

    Ok(layers)
}
