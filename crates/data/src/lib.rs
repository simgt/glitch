mod comm;

pub use comm::*;

use gst::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use tracing::error;

pub const DEFAULT_PORT: u16 = 9870;

// FIXME Most event should only reference the element id, but
// they don't arrive in a meaningful order so it's easier for
// now to send all the needed data every time.
// Maybe a cleaner solution would simply be to send the whole
// topology every time there's a change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    NewElement(Element),
    ChangeElementState {
        element: Element,
        state: ElementState,
    },
    AddChildElement {
        child: Element,
        parent: Element,
    },
    AddPad {
        pad: Pad,
        element: Element,
    },
    LinkPad {
        src_pad: Pad,
        sink_pad: Pad,
        state: LinkState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ObjectId(u64);

impl ObjectId {
    pub fn new(t: impl Hash) -> Self {
        let mut s = std::hash::DefaultHasher::new();
        t.hash(&mut s);
        Self(s.finish())
    }
}

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

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Element {
    pub id: ObjectId,
    pub name: String,
}

impl From<&gst::Element> for Element {
    fn from(element: &gst::Element) -> Self {
        let name = element
            .upcast_ref::<gst::Object>()
            .try_name()
            .unwrap_or("unnamed".to_string());
        Self {
            id: ObjectId::new(element),
            name,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Pad {
    pub id: ObjectId,
}

impl From<&gst::Pad> for Pad {
    fn from(pad: &gst::Pad) -> Self {
        Self {
            id: ObjectId::new(pad),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize, Default)]
pub enum ElementState {
    #[default]
    Null,
    Ready,
    Paused,
    Playing,
}

impl From<gst::State> for ElementState {
    fn from(state: gst::State) -> Self {
        match state {
            gst::State::Null => ElementState::Null,
            gst::State::Ready => ElementState::Ready,
            gst::State::Paused => ElementState::Paused,
            gst::State::Playing => ElementState::Playing,
            s => {
                error!("Unhandled state: {s:?}");
                ElementState::Null
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Serialize, Deserialize)]
pub enum LinkState {
    Pending,
    Done,
    Failed,
}

pub mod components {
    // It is necessary to put the component definitions in a separate crate
    // to avoid changing the type identities when the render crate is being
    // recompiled. Otherwise we have no way to query the world.

    #[derive(Debug, PartialEq, Eq, Copy, Clone)]
    pub struct Size(pub egui::Vec2);

    #[derive(Debug)]
    pub struct Link {
        pub from_pad: hecs::Entity,
        pub to_pad: hecs::Entity,
    }

    #[derive(Debug)]
    pub struct ElementTree;
    pub type ChildElement = hecs_hierarchy::Child<ElementTree>;
    pub type ParentElement = hecs_hierarchy::Parent<ElementTree>;

    #[derive(Debug)]
    pub struct PadTree;
    pub type ChildPad = hecs_hierarchy::Child<PadTree>;
}
