use glib::{gobject_ffi::g_strdup_value_contents, translate::ToGlibPtr};
use glitch_common::{comps::*, RecordingStream};
use gst::{prelude::*, Element, Pad};
use hecs::Entity;
use log::error;
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

pub trait EntityExt {
    fn from_hashable(t: impl Hash) -> Self;
}

impl EntityExt for Entity {
    fn from_hashable(t: impl Hash) -> Self {
        let mut s = std::hash::DefaultHasher::new();
        t.hash(&mut s);
        let u = s.finish();
        // We could use Entity::from_bits here, but it could fail if
        // the hash value is too low and the generation parts ends
        // up being zero.
        // The resulting entity may be invalid but this is just for
        // passing to the app, which will assign another entity before
        // inserting it in the world.
        unsafe { std::mem::transmute(u) }
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

pub trait RecordingStreamExt {
    fn insert_element(&self, element: &Element) -> Entity;
    fn insert_pad(&self, pad: &Pad) -> Entity;
    fn insert_link(&self, src: &Pad, sink: &Pad, state: State) -> Entity;
}

impl RecordingStreamExt for RecordingStream {
    fn insert_element(&self, element: &Element) -> Entity {
        use glib::object::ObjectExt;

        // elements hashes seem stable so we use that as the id
        let id = Entity::from_hashable(element);

        self.insert_one(id, Node);

        // FIXME don't add a name component if it's not there
        let name: Name = element
            .upcast_ref::<gst::Object>()
            .try_name()
            .unwrap_or("unnamed".to_string())
            .into();
        self.insert_one(id, name);

        // FIXME don't add a type_name component if it's not there
        let type_name: TypeName = element
            .factory()
            .map(|f| f.name().to_string())
            .unwrap_or("unknown".to_string())
            .into();
        self.insert_one(id, type_name);

        let properties: Properties = element
            .list_properties()
            .iter()
            .map(|p| {
                let name = p.name().to_string();
                let value = element.property_value(&name).to_string();
                (name, value)
            })
            .collect::<HashMap<String, String>>()
            .into();
        self.insert_one(id, properties);

        id
    }

    fn insert_pad(&self, pad: &Pad) -> Entity {
        let id = Entity::from_hashable(pad);

        let port = match pad.direction() {
            gst::PadDirection::Src => Port::Output,
            gst::PadDirection::Sink => Port::Input,
            _ => {
                error!("Unhandled pad direction");
                Port::Output
            }
        };
        self.insert_one(id, port);

        if let Some(parent) = pad.parent_element().map(|e| Entity::from_hashable(&e)) {
            self.insert_one(id, Child { parent });
        }

        let name: Name = pad
            .upcast_ref::<gst::Object>()
            .try_name()
            .unwrap_or("unnamed".to_string())
            .into();
        self.insert_one(id, name);

        id
    }

    fn insert_link(&self, src: &Pad, sink: &Pad, state: State) -> Entity {
        let src_id = self.insert_pad(src);
        let sink_id = self.insert_pad(sink);
        let edge_id = Entity::from_hashable((src, sink));
        self.insert_one(
            edge_id,
            Edge {
                output_port: src_id,
                input_port: sink_id,
            },
        );
        self.insert_one(edge_id, state);
        edge_id
    }
}
