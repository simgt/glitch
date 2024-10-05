use derive_more::{Constructor, From};
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::error;

// It is necessary to put the component definitions in a separate crate
// to avoid changing the type identities when the draw crate is being
// recompiled. Otherwise we have no way to query the world.

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Node;

#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize, Default)]
pub enum State {
    // For gstreamer elements
    #[default]
    Null,
    Ready,
    Paused,
    Playing,
    // For gstreamer links
    Pending,
    Done,
    Failed,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, From)]
pub struct Name(pub String);

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, From)]
pub struct TypeName(pub String);

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Default, From)]
pub struct Properties(pub HashMap<String, String>);

// FIXME separate in InputPort and OutputPort types
#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum Port {
    Output,
    Input,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct Edge {
    pub output_port: hecs::Entity,
    pub input_port: hecs::Entity,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize, From)]
pub struct Size(pub egui::Vec2);

#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub struct Child {
    pub parent: hecs::Entity,
}

#[derive(Debug, Constructor)]
pub struct TopologyLayout {
    pub graph: DiGraphMap<hecs::Entity, ()>,
    pub layers: Vec<Vec<hecs::Entity>>, // FIXME use ndarray
}

pub trait WorldTreeExt {
    // FIXME add a query_children::<Q> method instead
    fn children(&self, parent: hecs::Entity) -> Vec<hecs::Entity>;
    fn parent(&self, child: hecs::Entity) -> Option<hecs::Entity>;
}

impl WorldTreeExt for hecs::World {
    fn children(&self, parent: hecs::Entity) -> Vec<hecs::Entity> {
        self.query::<&Child>()
            .iter()
            .filter_map(|(e, c)| if c.parent == parent { Some(e) } else { None })
            .collect()
    }

    fn parent(&self, entity: hecs::Entity) -> Option<hecs::Entity> {
        self.get::<&Child>(entity).ok().map(|c| c.parent)
    }
}

#[cfg(feature = "gstreamer")]
pub mod gstreamer {
    use super::*;

    impl From<gst::State> for State {
        fn from(state: gst::State) -> Self {
            match state {
                gst::State::Null => State::Null,
                gst::State::Ready => State::Ready,
                gst::State::Paused => State::Paused,
                gst::State::Playing => State::Playing,
                s => {
                    error!("Unhandled state: {s:?}");
                    State::Null
                }
            }
        }
    }
}
