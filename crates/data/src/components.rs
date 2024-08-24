use derive_more::{derive::Constructor, From};
use hecs::Bundle;
use petgraph::graphmap::DiGraphMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use tracing::error;

// It is necessary to put the component definitions in a separate crate
// to avoid changing the type identities when the draw crate is being
// recompiled. Otherwise we have no way to query the world.

/// Represents a remote identifier, used to match objects sent by a
/// tracer to the corresponding entity.
// FIXME we should instead use a hashmap from (peer id, object id) to entity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct RemoteId(pub u64);

impl RemoteId {
    pub fn new(t: impl Hash) -> Self {
        let mut s = std::hash::DefaultHasher::new();
        t.hash(&mut s);
        Self(s.finish())
    }
}

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

#[derive(Debug, Serialize, Deserialize, Clone)]
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

// FIXME the bundles bellow are only for gstreamer

#[derive(Bundle, Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Element {
    pub id: RemoteId,
    pub name: Name,
    pub node: Node,
    pub type_name: TypeName,
    pub properties: Properties,
}

#[derive(Bundle, Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Pad {
    pub id: RemoteId,
    pub name: Name,
    pub port: Port,
}

#[cfg(feature = "gstreamer")]
pub use gstreamer::*;

#[cfg(feature = "gstreamer")]
pub mod gstreamer {
    use super::*;
    use glib::{gobject_ffi::g_strdup_value_contents, translate::ToGlibPtr};
    use gst::prelude::*;

    pub trait ObjectExt {
        fn try_name(&self) -> Option<String>;
    }

    impl ObjectExt for gst::Object {
        fn try_name(&self) -> Option<String> {
            // FIXME somehow in the tracer it happens that the name is NULL, which leads to
            // a crash when using `element.name()` directly
            let name_ptr = unsafe { gst_sys::gst_object_get_name(self.as_ptr()) };
            if name_ptr.is_null() {
                None
            } else {
                Some(unsafe {
                    glib::translate::from_glib_full::<_, glib::GString>(name_ptr).to_string()
                })
            }
        }
    }

    pub trait ValueExt {
        fn to_string(&self) -> String;
    }

    impl ValueExt for glib::Value {
        fn to_string(&self) -> String {
            unsafe {
                glib::translate::from_glib_full::<_, glib::GString>(g_strdup_value_contents(
                    self.to_glib_none().0,
                ))
            }
            .to_string()
        }
    }

    impl From<&gst::Element> for Element {
        fn from(element: &gst::Element) -> Self {
            let name = element
                .upcast_ref::<gst::Object>()
                .try_name()
                .unwrap_or("unnamed".to_string())
                .into();

            let type_name = element
                .factory()
                .map(|f| f.name().to_string())
                .unwrap_or("unknown".to_string())
                .into();

            use glib::object::ObjectExt;
            let properties = element
                .list_properties()
                .iter()
                .map(|p| {
                    let name = p.name().to_string();
                    let value = element.property_value(&name).to_string();
                    (name, value)
                })
                .collect::<HashMap<String, String>>()
                .into();

            Self {
                id: RemoteId::new(element),
                name,
                type_name,
                properties,
                node: Node,
            }
        }
    }

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

    impl From<&gst::Pad> for Pad {
        fn from(pad: &gst::Pad) -> Self {
            Self {
                id: RemoteId::new(pad),
                name: pad
                    .upcast_ref::<gst::Object>()
                    .try_name()
                    .unwrap_or("unnamed".to_string())
                    .into(),
                port: match pad.direction() {
                    gst::PadDirection::Src => Port::Output,
                    gst::PadDirection::Sink => Port::Input,
                    _ => {
                        error!("Unhandled pad direction");
                        Port::Output
                    }
                },
            }
        }
    }
}
