use anyhow::{Context, Result};
use egui::Pos2;
use pipewerk_common::comps::*;
use graph_layout::{LayeredLayout, Vec2 as LayoutVec2};
use petgraph::graphmap::DiGraphMap;
use tracing::{debug, error};

#[derive(Default)]
pub struct Layout {
    engine: LayeredLayout,
}

impl Layout {
    pub fn new(margin: egui::Vec2) -> Self {
        Self {
            engine: LayeredLayout::new(LayoutVec2::new(margin.x, margin.y)),
        }
    }

    /// Layout all the nodes bellow the given root
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

        // Iterate over all the edges, and add the ones that are to and from
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

        let layers = self.engine.compute_layers(&graph)?;

        buffer.insert_one(parent, layers);

        Ok(())
    }

    pub fn update_positions(
        &self,
        world: &hecs::World,
        root: hecs::Entity,
        buffer: &mut hecs::CommandBuffer,
    ) -> Result<()> {
        let layers = world.get::<&Layers>(root)?;

        let size_provider = |node: hecs::Entity| {
            world
                .get::<&Size>(node)
                .map(|s| LayoutVec2::new(s.0.x, s.0.y))
                .unwrap_or(LayoutVec2::zero())
        };

        let positions = self.engine.compute_positions(&layers, &size_provider);

        // FIXME use BatchBuilder
        for (node, pos) in positions.into_iter() {
            buffer.insert_one(node, Pos2::new(pos.x, pos.y));
        }

        Ok(())
    }
}
