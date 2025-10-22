use crate::{NodeSizes, Point, Vec2};
use petgraph::graphmap::DiGraphMap;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::Hash;

/// Assign coordinates to nodes based on their layer structure and sizes
pub(crate) fn assign_coordinates<N, S>(
    layers: &[Vec<N>],
    graph: &DiGraphMap<N, ()>,
    sizes: &S,
    margin: Vec2,
    max_position_iterations: usize,
) -> HashMap<N, Point>
where
    N: Copy + Ord + Hash,
    S: NodeSizes<N>,
{
    let mut positions = HashMap::new();

    // First pass: Horizontal positioning
    assign_horizontal_positions(layers, sizes, &mut positions, margin);

    // Second pass: Vertical positioning
    assign_vertical_positions(
        layers,
        graph,
        &mut positions,
        sizes,
        margin,
        max_position_iterations,
    );

    positions
}

/// Assign horizontal positions based on layers
fn assign_horizontal_positions<N, S>(
    layers: &[Vec<N>],
    sizes: &S,
    positions: &mut HashMap<N, Point>,
    margin: Vec2,
) where
    N: Copy + Ord + Hash,
    S: NodeSizes<N>,
{
    // Calculate the maximum width for each layer
    let layer_dimensions: Vec<Vec2> = layers
        .iter()
        .map(|layer| {
            layer
                .iter()
                .map(|&node| sizes.size(node))
                .fold(Vec2::zero(), Vec2::max)
        })
        .collect();

    let mut x = 0.0;
    for (layer_index, layer) in layers.iter().enumerate() {
        let layer_size = layer_dimensions[layer_index];
        for &node in layer {
            let node_size = sizes.size(node);
            positions.insert(
                node,
                Point::new(x + (layer_size.x - node_size.x) / 2.0, 0.0),
            );
        }
        x += layer_size.x + margin.x;
    }
}

/// Assign vertical positions with barycenter optimization
fn assign_vertical_positions<N, S>(
    layers: &[Vec<N>],
    graph: &DiGraphMap<N, ()>,
    positions: &mut HashMap<N, Point>,
    sizes: &S,
    margin: Vec2,
    max_iterations: usize,
) where
    N: Copy + Ord + Hash,
    S: NodeSizes<N>,
{
    // Initial positioning
    initial_vertical_positioning(layers, positions, sizes, margin);

    // Iterative optimization
    for _ in 0..max_iterations {
        let mut changed = false;

        for layer_idx in (0..layers.len().saturating_sub(1)).rev() {
            let layer = &layers[layer_idx];
            for &node in layer {
                let Some(new_y) =
                    calculate_barycenter(node, &layers[layer_idx + 1], graph, positions, sizes)
                else {
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
            let mut sorted_nodes: Vec<_> = layer.to_vec();
            sorted_nodes.sort_by(|&a, &b| {
                positions[&a]
                    .y
                    .partial_cmp(&positions[&b].y)
                    .unwrap_or(Ordering::Equal)
            });

            for i in 1..sorted_nodes.len() {
                let prev_node = sorted_nodes[i - 1];
                let curr_node = sorted_nodes[i];
                let prev_bottom = positions[&prev_node].y + sizes.size(prev_node).y;
                let curr_top = &mut positions.get_mut(&curr_node).unwrap().y;

                if *curr_top < prev_bottom + margin.y {
                    *curr_top = prev_bottom + margin.y;
                    changed = true;
                }
            }
        }

        if !changed {
            break;
        }
    }

    // Final adjustments
    normalize_vertical_positions(positions);
}

/// Calculate the barycenter (average position) of connected nodes
fn calculate_barycenter<N, S>(
    node: N,
    next_layer: &[N],
    graph: &DiGraphMap<N, ()>,
    positions: &HashMap<N, Point>,
    sizes: &S,
) -> Option<f32>
where
    N: Copy + Ord + Hash,
    S: NodeSizes<N>,
{
    let mut sum_y = 0.0;
    let mut count = 0;

    for &next_node in next_layer {
        if graph.contains_edge(node, next_node) {
            if let Some(pos) = positions.get(&next_node) {
                let next_height = sizes.size(next_node).y;
                sum_y += pos.y + next_height / 2.0;
                count += 1;
            }
        }
    }

    let node_height = sizes.size(node).y;

    if count > 0 {
        Some((sum_y / count as f32) - node_height / 2.0)
    } else {
        None
    }
}

/// Initial vertical positioning with uniform spacing
fn initial_vertical_positioning<N, S>(
    layers: &[Vec<N>],
    positions: &mut HashMap<N, Point>,
    sizes: &S,
    margin: Vec2,
) where
    N: Copy + Ord + Hash,
    S: NodeSizes<N>,
{
    for layer in layers {
        let mut y = 0.0;
        for &node in layer {
            if let Some(pos) = positions.get_mut(&node) {
                pos.y = y;
                y += margin.y + sizes.size(node).y;
            }
        }
    }
}

/// Normalize vertical positions to start from y=0
fn normalize_vertical_positions<N>(positions: &mut HashMap<N, Point>)
where
    N: Copy + Ord + Hash,
{
    let min_y = positions
        .values()
        .map(|pos| pos.y)
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0);

    for pos in positions.values_mut() {
        pos.y -= min_y;
    }
}
