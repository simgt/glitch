use crate::{NodeSizes, Point};
use anyhow::Result;
use std::collections::HashMap;
use std::hash::Hash;

/// A layout engine that can compute positions for graph nodes
///
/// This trait is generic over the graph type `G`, allowing different layout
/// engines to work with different graph types:
/// - Layered layouts implement `LayoutEngine<G>` for `G: DirectedGraph`
/// - Force-directed layouts could implement it for undirected graphs
/// - Other layouts can specify their own graph requirements
pub trait LayoutEngine<G> {
    /// The type used to identify nodes in the graph
    type NodeId: Copy + Ord + Hash;

    /// Compute node positions for the given graph
    ///
    /// # Errors
    /// Returns an error if the layout computation fails (e.g., graph contains
    /// cycles for DAG layouts, or other layout-specific constraints are violated)
    fn layout<S>(&self, graph: G, sizes: &S) -> Result<HashMap<Self::NodeId, Point>>
    where
        S: NodeSizes<Self::NodeId>;
}
