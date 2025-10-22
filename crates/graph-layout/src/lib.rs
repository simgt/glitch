//! Generic graph layout algorithms
//!
//! This crate provides generic layout algorithms for graphs that work with
//! any graph data structure through petgraph's visitor traits. It works
//! with any graph implementing petgraph's visitor traits.
//!
//! # Layout Engines
//!
//! - [`LayeredLayout`]: Sugiyama-style layered layout for DAGs
//!
//! # Example
//!
//! ```
//! use graph_layout::{LayeredLayout, LayoutEngine, Vec2};
//! use petgraph::graphmap::DiGraphMap;
//!
//! // Create a graph
//! let mut graph = DiGraphMap::new();
//! graph.add_edge(1, 2, ());
//! graph.add_edge(2, 3, ());
//!
//! // Create a layout engine
//! let engine = LayeredLayout::new(Vec2::new(20.0, 20.0));
//!
//! // Provide node sizes
//! let sizes = |_node| Vec2::new(100.0, 50.0);
//!
//! // Use the LayoutEngine trait (simple, single-phase):
//! let positions = engine.layout(&graph, &sizes).unwrap();
//!
//! // Or directly by calling each step for better control
//! let layers = engine.compute_layers(&graph).unwrap();
//! let positions = engine.compute_positions(&layers, &sizes);
//! ```

mod engine;
mod geometry;
mod sizes;

pub mod layered;

// Re-export core types and traits
pub use engine::LayoutEngine;
pub use geometry::{Point, Vec2};
pub use sizes::NodeSizes;

// Re-export petgraph visitor traits for graph abstraction
pub use petgraph::visit::{GraphBase, IntoNeighborsDirected, IntoNodeIdentifiers};
pub use petgraph::Direction;

// Re-export layered layout types
pub use layered::{LayeredLayout, Layers};
