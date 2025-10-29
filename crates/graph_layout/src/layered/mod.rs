mod layers;
mod crossings;
mod positions;

use crate::{LayoutEngine, NodeSizes, Point, Vec2};
use petgraph::visit::{IntoNeighborsDirected, IntoNodeIdentifiers};
use petgraph::Direction;
use petgraph::graphmap::DiGraphMap;
use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use thiserror::Error;

use layers::assign_layers;
use crossings::minimize_crossings;
use positions::assign_coordinates;

/// Errors that can occur during layered layout computation
#[derive(Debug, Error)]
pub enum LayeredLayoutError<N>
where
    N: fmt::Debug,
{
    /// The graph contains a cycle at the given node
    #[error("graph contains a cycle at node {0:?}")]
    GraphHasCycle(N),
}

/// Configuration for the layered (Sugiyama-style) DAG layout
#[derive(Debug, Clone)]
pub struct LayeredLayout {
    /// Horizontal and vertical margins between nodes
    pub margin: Vec2,

    /// Maximum iterations for crossing minimization
    pub max_crossing_iterations: usize,

    /// Maximum iterations for vertical position optimization
    pub max_position_iterations: usize,
}

impl Default for LayeredLayout {
    fn default() -> Self {
        Self {
            margin: Vec2::new(20.0, 20.0),
            max_crossing_iterations: 10,
            max_position_iterations: 50,
        }
    }
}

impl LayeredLayout {
    /// Create a new layered layout with the given margin
    pub fn new(margin: Vec2) -> Self {
        Self {
            margin,
            ..Default::default()
        }
    }
}

/// Layer structure that can be cached and reused
#[derive(Debug, Clone)]
pub struct Layers<N>
where
    N: Copy + Ord + Hash + std::fmt::Debug,
{
    /// Internal graph representation for efficient edge lookups
    pub(crate) graph: DiGraphMap<N, ()>,

    /// Nodes organized into topological layers
    pub nodes: Vec<Vec<N>>,

    /// Number of edge crossings (quality metric)
    pub crossings: usize,
}

impl LayeredLayout {
    /// Compute layer structure (expensive, cache this)
    ///
    /// This phase assigns nodes to layers and minimizes edge crossings.
    /// It only depends on the graph structure, not on node sizes.
    ///
    /// # Errors
    /// Returns an error if the graph contains cycles
    pub fn compute_layers<G>(&self, graph: G) -> Result<Layers<G::NodeId>, LayeredLayoutError<G::NodeId>>
    where
        G: IntoNodeIdentifiers + IntoNeighborsDirected,
        G::NodeId: Copy + Ord + Hash + std::fmt::Debug,
    {
        let layers = assign_layers(&graph)?;
        let (layers, crossings) = minimize_crossings(&graph, layers, self.max_crossing_iterations);

        // Convert graph to DiGraphMap for efficient lookups during positioning
        let mut internal_graph = DiGraphMap::new();
        for node in graph.node_identifiers() {
            internal_graph.add_node(node);
        }
        for node in graph.node_identifiers() {
            for succ in graph.neighbors_directed(node, Direction::Outgoing) {
                internal_graph.add_edge(node, succ, ());
            }
        }

        Ok(Layers {
            graph: internal_graph,
            nodes: layers,
            crossings,
        })
    }

    /// Compute positions from cached layers (cheap, rerun when sizes change)
    ///
    /// This phase assigns coordinates to nodes based on their layer structure
    /// and current sizes. It can be called repeatedly as node sizes change.
    pub fn compute_positions<N, S>(
        &self,
        layers: &Layers<N>,
        sizes: &S,
    ) -> HashMap<N, Point>
    where
        N: Copy + Ord + Hash + std::fmt::Debug,
        S: NodeSizes<N>,
    {
        assign_coordinates(
            &layers.nodes,
            &layers.graph,
            sizes,
            self.margin,
            self.max_position_iterations,
        )
    }
}

// Implement LayoutEngine for any graph with the required capabilities
impl<G> LayoutEngine<G> for LayeredLayout
where
    G: IntoNodeIdentifiers + IntoNeighborsDirected,
    G::NodeId: Copy + Ord + Hash + std::fmt::Debug,
{
    type NodeId = G::NodeId;
    type Error = LayeredLayoutError<G::NodeId>;

    fn layout<S>(&self, graph: G, sizes: &S) -> Result<HashMap<Self::NodeId, Point>, Self::Error>
    where
        S: NodeSizes<Self::NodeId>,
    {
        let layers = self.compute_layers(graph)?;
        Ok(self.compute_positions(&layers, sizes))
    }
}
