use crate::Vec2;
use std::collections::HashMap;
use std::hash::Hash;

/// Trait for providing node sizes during layout computation
pub trait NodeSizes<N> {
    /// Get the size of a node
    fn size(&self, node: N) -> Vec2;
}

// Blanket implementation for closures
impl<N, F> NodeSizes<N> for F
where
    F: Fn(N) -> Vec2,
{
    fn size(&self, node: N) -> Vec2 {
        self(node)
    }
}

// Implementation for HashMap
impl<N: Eq + Hash + Copy> NodeSizes<N> for HashMap<N, Vec2> {
    fn size(&self, node: N) -> Vec2 {
        self.get(&node).copied().unwrap_or(Vec2::zero())
    }
}
