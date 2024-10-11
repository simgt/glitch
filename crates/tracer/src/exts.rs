use glitch_common::{comps::*, RecordingStream};
use gst::glib::{gobject_ffi::g_strdup_value_contents, object::ObjectExt, translate::ToGlibPtr};
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

pub trait ValueExt {
    fn to_string(&self) -> String;
}

impl ValueExt for glib::Value {
    fn to_string(&self) -> String {
        unsafe {
            let stash = self.to_glib_none();
            let ptr = stash.0;
            if ptr.is_null() {
                error!("Failed to get string from value");
                String::new()
            } else {
                println!("ptr: {:?}", ptr);
                println!("contents: {:?}", g_strdup_value_contents(ptr));
                println!("done");
                glib::translate::from_glib_full::<_, glib::GString>(g_strdup_value_contents(ptr))
                    .to_string()
            }
        }
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
            .filter_map(|pspec| {
                let name = pspec.name().to_string();
                // FIXME the call to property_value() is enough to crash when using rtsp
                // Solution might be here: file:///Users/simon/Dev/glitch/target/doc/src/glib/object.rs.html#2418
                unsafe {
                    use gst::glib::translate::ToGlibPtrMut;
                    let mut value = glib::Value::from_type_unchecked(pspec.value_type());
                    dbg!(element.as_object_ref().to_glib_none().0);
                    dbg!(pspec.name().as_ptr());
                    dbg!(value.to_glib_none_mut().0);
                    // glib::gobject_ffi::g_object_get_property(
                    //     element.as_object_ref().to_glib_none().0,
                    //     pspec.name().as_ptr() as *const _,
                    //     value.to_glib_none_mut().0,
                    // );
                }
                //let value = format!("{:?}", element.property_value(&name));
                let value = "test".to_owned();
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
