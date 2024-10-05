use anyhow::{anyhow, Context, Result};
use egui::{Pos2, Vec2};
use glitch_data::components::*;
use petgraph::algo::toposort;
use petgraph::graphmap::DiGraphMap;
use petgraph::prelude::*;
use std::{cmp::Ordering, collections::HashMap};
use tracing::{debug, error};

pub struct DAGLayout {
    margin: Vec2,
}

impl Default for DAGLayout {
    fn default() -> Self {
        Self {
            margin: Vec2::new(20.0, 20.0),
        }
    }
}

impl DAGLayout {
    pub fn new(margin: Vec2) -> Self {
        Self { margin }
    }

    /// Layout all the nodes bellow the given root
    /// FIXME this function doesn't need to be recursive, it can just iterate on
    /// entities that have a Parent component
    pub fn update_topology(
        &self,
        world: &hecs::World,
        parent: hecs::Entity,
        buffer: &mut hecs::CommandBuffer,
    ) -> Result<()> {
        // FIXME we should take the queries as params to avoid borrowing the entire world
        debug!("Layout from {parent:?}");

        // Create a graph of the children of the given root
        // For now we ignore the actual ports when deciding on the layout
        let mut graph = DiGraphMap::<hecs::Entity, ()>::new();

        for (entity, _) in world
            .query::<&Child>()
            .with::<(&Node, &Size)>()
            .iter()
            .filter(|(_, c)| c.parent == parent)
        {
            graph.add_node(entity);
        }

        // Iterate over all the wires, and add the edges that are to and from
        // nodes of this graph
        for (_, edge) in world.query::<&Edge>().iter() {
            let Ok(from_node) = world
                .parent(edge.output_port)
                .with_context(|| {
                    format!("Output port {:?} doesn't have a parent", edge.output_port)
                })
                .inspect_err(|e| error!("{e}"))
            else {
                continue;
            };

            let Ok(to_node) = world
                .parent(edge.input_port)
                .with_context(|| format!("Input port {:?} doesn't have a parent", edge.input_port))
                .inspect_err(|e| error!("{e}"))
            else {
                continue;
            };

            if graph.contains_node(from_node) && graph.contains_node(to_node) {
                graph.add_edge(from_node, to_node, ());
            }
        }

        debug!("Graph of {parent:?}: {graph:?}");

        let layers = self
            .assign_layers(&graph)
            .and_then(|layers| self.minimize_crossings(&graph, layers))?;

        buffer.insert_one(parent, TopologyLayout::new(graph, layers));

        Ok(())
    }

    pub fn update_positions(
        &self,
        world: &hecs::World,
        root: hecs::Entity,
        buffer: &mut hecs::CommandBuffer,
    ) -> Result<()> {
        let positions = {
            let TopologyLayout {
                ref graph,
                ref layers,
            } = *world.get::<&TopologyLayout>(root)?;

            self.assign_coordinates(&layers, &world.view::<&Size>(), &graph)
                .context("Failed to assign coordinates")?
        };

        // FIXME use BatchBuilder
        for (node, pos) in positions.into_iter() {
            buffer.insert_one(node, pos);
        }

        Ok(())
    }
}

impl DAGLayout {
    fn assign_layers(
        &self,
        graph: &DiGraphMap<hecs::Entity, ()>,
    ) -> Result<Vec<Vec<hecs::Entity>>> {
        let topo_order = toposort(graph, None).map_err(|_| anyhow!("Graph has cycles"))?;
        let mut layer_map = HashMap::new();

        for &node in &topo_order {
            let max_pred_layer = graph
                .edges_directed(node, petgraph::Direction::Incoming)
                .map(|e| layer_map.get(&e.source()).unwrap_or(&0) + 1)
                .max()
                .unwrap_or(0);
            layer_map.insert(node, max_pred_layer);
        }

        // Group nodes by layer
        let max_layer = *layer_map.values().max().unwrap_or(&0);
        let mut layers = vec![Vec::new(); max_layer + 1];
        for (node, &layer) in &layer_map {
            layers[layer].push(*node);
        }

        Ok(layers)
    }

    fn minimize_crossings(
        &self,
        graph: &DiGraphMap<hecs::Entity, ()>,
        mut layers: Vec<Vec<hecs::Entity>>,
    ) -> Result<Vec<Vec<hecs::Entity>>> {
        let max_iterations = 10;

        for _ in 0..max_iterations {
            let mut improved = false;

            for layer_index in 0..layers.len() {
                let layer_len = layers[layer_index].len();
                for i in 0..layer_len.max(1) - 1 {
                    let crossings_before = self.count_crossings(graph, &layers);
                    layers[layer_index].swap(i, i + 1);
                    let crossings_after = self.count_crossings(graph, &layers);

                    if crossings_after > crossings_before
                        || (crossings_after == crossings_before
                            && layers[layer_index][i] > layers[layer_index][i + 1])
                    {
                        // Swap back if no improvement
                        layers[layer_index].swap(i, i + 1);
                    } else {
                        improved = true;
                    }
                }
            }

            if !improved {
                break;
            }
        }

        Ok(layers)
    }

    fn count_crossings(
        &self,
        graph: &DiGraphMap<hecs::Entity, ()>,
        layers: &[Vec<hecs::Entity>],
    ) -> usize {
        // Implementation of crossing counting
        // This is a simplified version and may need to be optimized for large graphs
        let mut crossings = 0;

        for i in 0..layers.len() - 1 {
            let upper_layer = &layers[i];
            let lower_layer = &layers[i + 1];

            for (idx1, &node1) in upper_layer.iter().enumerate() {
                for (idx2, &node2) in upper_layer.iter().enumerate().skip(idx1 + 1) {
                    for edge1 in graph.edges(node1) {
                        for edge2 in graph.edges(node2) {
                            if lower_layer.contains(&edge1.target())
                                && lower_layer.contains(&edge2.target())
                            {
                                let pos1 = lower_layer
                                    .iter()
                                    .position(|&n| n == edge1.target())
                                    .unwrap();
                                let pos2 = lower_layer
                                    .iter()
                                    .position(|&n| n == edge2.target())
                                    .unwrap();
                                if (idx1 < idx2) != (pos1 < pos2) {
                                    crossings += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        crossings
    }

    fn assign_coordinates(
        &self,
        layers: &[Vec<hecs::Entity>],
        node_sizes: &hecs::ViewBorrow<&Size>,
        graph: &DiGraphMap<hecs::Entity, ()>,
    ) -> Result<HashMap<hecs::Entity, Pos2>> {
        let mut positions = HashMap::new();

        // First pass: Horizontal positioning
        self.assign_horizontal_positions(layers, node_sizes, &mut positions)?;

        // Second pass: Vertical positioning
        self.assign_vertical_positions(layers, graph, &mut positions, node_sizes)?;

        Ok(positions)
    }

    fn assign_horizontal_positions(
        &self,
        layers: &[Vec<hecs::Entity>],
        node_sizes: &hecs::ViewBorrow<&Size>,
        positions: &mut HashMap<hecs::Entity, Pos2>,
    ) -> Result<()> {
        let layer_dimensions: Vec<Vec2> = layers
            .iter()
            .map(|layer| {
                layer
                    .iter()
                    .map(|&node| {
                        node_sizes
                            .get(node)
                            .map(|size| size.0)
                            .unwrap_or(Vec2::ZERO)
                    })
                    .fold(Vec2::ZERO, Vec2::max)
            })
            .collect();

        let mut x = 0.0;
        for (layer_index, layer) in layers.iter().enumerate() {
            let layer_size = layer_dimensions[layer_index];
            for &node in layer {
                let node_size = node_sizes.get(node).copied().map_or(Vec2::ZERO, |s| s.0);
                positions.insert(node, Pos2::new(x + (layer_size.x - node_size.x) / 2.0, 0.0));
            }
            x += layer_size.x + self.margin.x;
        }

        Ok(())
    }

    fn assign_vertical_positions(
        &self,
        layers: &[Vec<hecs::Entity>],
        graph: &DiGraphMap<hecs::Entity, ()>,
        positions: &mut HashMap<hecs::Entity, Pos2>,
        node_sizes: &hecs::ViewBorrow<&Size>,
    ) -> Result<()> {
        // Initial positioning
        self.initial_vertical_positioning(layers, positions, node_sizes);

        // FIXME account for multiple nodes in the same layer
        let max_iterations = 50;
        for _ in 0..max_iterations {
            let mut changed = false;

            for layer_idx in (0..layers.len() - 1).rev() {
                let layer = &layers[layer_idx];
                for &node in layer {
                    let Some(new_y) = self.calculate_barycenter(
                        node,
                        &layers[layer_idx + 1],
                        graph,
                        positions,
                        node_sizes,
                    ) else {
                        continue;
                    };

                    let Some(pos) = positions.get_mut(&node) else {
                        continue;
                    };

                    if (new_y - pos.y).abs() > 0.1 {
                        pos.y = new_y;
                        changed = true;
                    }
                }

                // Enforce minimum vertical distance between nodes
                let mut sorted_nodes: Vec<_> = layer.iter().copied().collect();
                sorted_nodes.sort_by(|&a, &b| {
                    positions[&a]
                        .y
                        .partial_cmp(&positions[&b].y)
                        .unwrap_or(Ordering::Equal)
                });

                for i in 1..sorted_nodes.len() {
                    let prev_node = sorted_nodes[i - 1];
                    let curr_node = sorted_nodes[i];
                    let prev_bottom =
                        positions[&prev_node].y + node_sizes.get(prev_node).unwrap().0.y;
                    let curr_top = &mut positions.get_mut(&curr_node).unwrap().y;

                    if *curr_top < prev_bottom + self.margin.y {
                        *curr_top = prev_bottom + self.margin.y;
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }

        // Final adjustments (normalize positions, ensure minimum spacing)
        self.normalize_vertical_positions(positions);

        Ok(())
    }

    fn calculate_barycenter(
        &self,
        node: hecs::Entity,
        layer: &[hecs::Entity],
        graph: &DiGraphMap<hecs::Entity, ()>,
        positions: &HashMap<hecs::Entity, Pos2>,
        node_sizes: &hecs::ViewBorrow<&Size>,
    ) -> Option<f32> {
        let mut sum_y = 0.0;
        let mut count = 0;

        // Check next layer
        // FIXME avoid using petgraph here
        for &next_node in layer {
            if graph.contains_edge(node, next_node) {
                if let Some(pos) = positions.get(&next_node) {
                    let next_height = node_sizes.get(next_node).map_or(0.0, |size| size.0.y);
                    sum_y += pos.y + next_height / 2.0;
                    count += 1;
                }
            }
        }

        let node_height = node_sizes.get(node).map_or(0.0, |size| size.0.y);

        if count > 0 {
            Some((sum_y / count as f32) - node_height / 2.0)
        } else {
            None
        }
    }

    fn initial_vertical_positioning(
        &self,
        layers: &[Vec<hecs::Entity>],
        positions: &mut HashMap<hecs::Entity, Pos2>,
        node_sizes: &hecs::ViewBorrow<&Size>,
    ) {
        for layer in layers {
            let mut y = 0.0;
            for &node in layer {
                if let Some(pos) = positions.get_mut(&node) {
                    pos.y = y;
                    y += self.margin.y + node_sizes.get(node).map_or(0.0, |size| size.0.y);
                }
            }
        }
    }

    fn normalize_vertical_positions(&self, positions: &mut HashMap<hecs::Entity, Pos2>) {
        // Find the minimum vertical position
        let min_y = positions
            .values()
            .map(|pos| pos.y)
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0);

        // Add the minimum vertical position to every node
        for pos in positions.values_mut() {
            pos.y -= min_y;
        }
    }
}
