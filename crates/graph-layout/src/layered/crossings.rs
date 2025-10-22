use petgraph::visit::IntoNeighborsDirected;
use petgraph::Direction;
use std::hash::Hash;

/// Minimize edge crossings by swapping adjacent nodes in layers
///
/// Uses a greedy local search approach with multiple iterations
pub(crate) fn minimize_crossings<G>(
    graph: &G,
    mut layers: Vec<Vec<G::NodeId>>,
    max_iterations: usize,
) -> (Vec<Vec<G::NodeId>>, usize)
where
    G: IntoNeighborsDirected,
    G::NodeId: Copy + Ord + Hash,
{
    for _ in 0..max_iterations {
        let mut improved = false;

        for layer_index in 0..layers.len() {
            let layer_len = layers[layer_index].len();
            for i in 0..layer_len.saturating_sub(1) {
                let crossings_before = count_crossings(graph, &layers);
                layers[layer_index].swap(i, i + 1);
                let crossings_after = count_crossings(graph, &layers);

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

    let crossings = count_crossings(graph, &layers);
    (layers, crossings)
}

/// Count the number of edge crossings in the current layout
fn count_crossings<G>(graph: &G, layers: &[Vec<G::NodeId>]) -> usize
where
    G: IntoNeighborsDirected,
    G::NodeId: Copy + Ord + Hash,
{
    let mut crossings = 0;

    for i in 0..layers.len().saturating_sub(1) {
        let upper_layer = &layers[i];
        let lower_layer = &layers[i + 1];

        for (idx1, &node1) in upper_layer.iter().enumerate() {
            for (idx2, &node2) in upper_layer.iter().enumerate().skip(idx1 + 1) {
                for target1 in graph.neighbors_directed(node1, Direction::Outgoing) {
                    for target2 in graph.neighbors_directed(node2, Direction::Outgoing) {
                        if lower_layer.contains(&target1) && lower_layer.contains(&target2) {
                            let pos1 = lower_layer
                                .iter()
                                .position(|&n| n == target1)
                                .unwrap();
                            let pos2 = lower_layer
                                .iter()
                                .position(|&n| n == target2)
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
