use anyhow::{anyhow, Result};
use egui::{Pos2, Vec2};
use petgraph::algo::toposort;
use petgraph::graphmap::{DiGraphMap, NodeTrait};
use petgraph::prelude::*;
use std::{collections::HashMap, fmt::Debug};

pub struct DAGLayout {
    padding: Vec2,
}

impl Default for DAGLayout {
    fn default() -> Self {
        Self {
            padding: Vec2::new(20.0, 20.0), // Default value
        }
    }
}

impl DAGLayout {
    pub fn new(padding: Vec2) -> Self {
        Self { padding }
    }

    pub fn layout<N, E>(
        &self,
        graph: &DiGraphMap<N, E>,
        node_sizes: &HashMap<N, Vec2>,
    ) -> Result<HashMap<N, Pos2>>
    where
        N: NodeTrait + Debug,
        E: Copy,
    {
        let layers = self.assign_layers(graph)?;
        let ordered_layers = self.minimize_crossings(graph, &layers);
        self.assign_coordinates(&ordered_layers, node_sizes)
    }

    fn assign_layers<N, E>(&self, graph: &DiGraphMap<N, E>) -> Result<Vec<Vec<N>>>
    where
        N: NodeTrait + Debug,
        E: Copy,
    {
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

    fn minimize_crossings<N, E>(&self, graph: &DiGraphMap<N, E>, layers: &[Vec<N>]) -> Vec<Vec<N>>
    where
        N: NodeTrait,
        E: Copy,
    {
        let mut ordered_layers = layers.to_vec();
        let max_iterations = 100;

        for _ in 0..max_iterations {
            let mut improved = false;

            for layer_index in 0..ordered_layers.len() {
                let layer_len = ordered_layers[layer_index].len();
                for i in 0..layer_len.max(1) - 1 {
                    let crossings_before = self.count_crossings(graph, &ordered_layers);
                    ordered_layers[layer_index].swap(i, i + 1);
                    let crossings_after = self.count_crossings(graph, &ordered_layers);

                    if crossings_after >= crossings_before {
                        // Swap back if no improvement
                        ordered_layers[layer_index].swap(i, i + 1);
                    } else {
                        improved = true;
                    }
                }
            }

            if !improved {
                break;
            }
        }

        ordered_layers
    }

    fn count_crossings<N, E>(&self, graph: &DiGraphMap<N, E>, layers: &[Vec<N>]) -> usize
    where
        N: NodeTrait,
        E: Copy,
    {
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

    fn assign_coordinates<N>(
        &self,
        layers: &[Vec<N>],
        node_sizes: &HashMap<N, Vec2>,
    ) -> Result<HashMap<N, Pos2>>
    where
        N: NodeTrait,
    {
        let mut positions = HashMap::new();

        let layer_dimensions: Vec<Vec2> = layers
            .iter()
            .map(|layer| {
                layer
                    .iter()
                    .map(|&node| node_sizes.get(&node).copied().unwrap_or(Vec2::ZERO))
                    .fold(Vec2::ZERO, Vec2::max)
            })
            .collect();

        let mut bb = Vec2::ZERO;
        let mut x = 0.0;
        for (layer_index, layer) in layers.iter().enumerate() {
            let layer_size = layer_dimensions[layer_index];
            let mut y = 0.0;

            for &node in layer {
                let node_size = node_sizes.get(&node).copied().unwrap_or(Vec2::ZERO); // FIXME return an error, all nodes need to be sized
                let node_pos = (Vec2::new(x, y)
                    + ((layer_size - node_size) * Vec2::RIGHT + node_size * Vec2::DOWN) / 2.0
                        * Vec2::RIGHT)
                    .to_pos2();
                positions.insert(node, node_pos);
                y += node_size.y + self.padding.y;
            }
            bb.y = bb.y.max(y - self.padding.y);
            x += layer_size.x + self.padding.x;
        }
        bb.x = x - self.padding.x;

        Ok(positions)
    }
}
