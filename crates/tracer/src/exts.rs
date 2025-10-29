use glib::ParamFlags;
use pipewerk_common::{comps::*, RecordingStream};
use gst::glib::{gobject_ffi::g_strdup_value_contents, object::ObjectExt, translate::ToGlibPtr};
use gst::{prelude::*, Element, Pad};
use hecs::Entity;
use log::{error, warn};
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
        element
            .property::<Option<glib::GString>>("name")
            .inspect(|s| self.insert_one(id, Name::from(s.to_string())));

        // FIXME don't add a type_name component if it's not there
        element
            .factory()
            .and_then(|f| f.property::<Option<glib::GString>>("name"))
            .inspect(|s| {
                let type_name: TypeName = s.to_string().into();
                self.insert_one(id, type_name);
            });

        let properties: Properties = element
            .list_properties()
            .iter()
            .filter_map(|p| {
                let name = p.name().to_string();
                // Somehow `property_value` segfaults on rtph264pay when called on
                // non-writable properties, this may be the case on other elements.
                // Doc states that property_value will panic in that case.
                // Issue is this trick prevents from forwarding read-only properties
                // like `stats` of the identity element.
                // Related: https://discourse.gstreamer.org/t/ximagesink-segfault-on-element-name-custom-rust-tracer/1360
                if !p.flags().contains(ParamFlags::WRITABLE) {
                    warn!("Property {name} is not writable, skipping");
                    return None;
                }

                let value = format!("{:?}", element.property_value(&name));
                Some((name, value))
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

        pad.parent_element().inspect(|e| {
            self.insert_one(
                id,
                Child {
                    parent: Entity::from_hashable(e),
                },
            );
        });

        pad.property::<Option<glib::GString>>("name")
            .inspect(|s| self.insert_one(id, Name::from(s.to_string())));

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
