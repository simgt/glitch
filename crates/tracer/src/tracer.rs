use gst::glib;

glib::wrapper! {
    pub struct GlitchTracer(ObjectSubclass<imp::GlitchTracer>)
       @extends gst::Tracer, gst::Object;
}

mod imp {
    use crate::EntityExt;
    use glitch_common::{Child, RecordingStream, State};
    use gst::{glib, prelude::*, subclass::prelude::*};
    use hecs::Entity;
    use log::*;
    use once_cell::sync::Lazy;
    use std::{net::Ipv4Addr, str::FromStr};

    use crate::exts::RecordingStreamExt;

    static _CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
        gst::DebugCategory::new(
            "glitchtracing",
            gst::DebugColorFlags::all(),
            Some("Glitch client tracer"),
        )
    });

    pub struct GlitchTracer {
        pub stream: glitch_common::RecordingStream,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlitchTracer {
        const NAME: &'static str = "GlitchTracer";
        type Type = super::GlitchTracer;
        type ParentType = gst::Tracer;

        fn new() -> Self {
            Self {
                stream: RecordingStream::new(),
            }
        }
    }

    impl ObjectImpl for GlitchTracer {
        fn constructed(&self) {
            let mut ip = Ipv4Addr::LOCALHOST;
            let mut port = glitch_common::DEFAULT_PORT;
            if let Some(params) = self.obj().property::<Option<String>>("params") {
                let structure = {
                    let tmp = format!("params,{}", params);
                    info!("{:?} params: {:?}", self.obj(), tmp);
                    gst::Structure::from_str(&tmp).unwrap_or_else(|e| {
                        error!("Invalid params string: {:?}: {e:?}", tmp);
                        gst::Structure::new_empty("params")
                    })
                };

                debug!("params = {}", &structure);

                if let Ok(s) = structure.get::<String>("ip") {
                    ip = Ipv4Addr::from_str(&s).expect("Invalid IP address");
                }

                if let Ok(s) = structure.get::<String>("port") {
                    port = s.parse::<u16>().expect("Invalid port number");
                } else if let Ok(p) = structure.get::<i32>("port") {
                    port = p as u16;
                }
            };

            debug!("Connecting to {ip}:{port}");
            self.stream.connect(ip, port);

            self.parent_constructed();
            self.register_hook(TracerHook::BinAddPost);
            self.register_hook(TracerHook::ElementAddPad);
            self.register_hook(TracerHook::ElementChangeStatePost);
            self.register_hook(TracerHook::ElementNew);
            self.register_hook(TracerHook::PadLinkPost);
            self.register_hook(TracerHook::PadLinkPre);
        }
    }

    impl GstObjectImpl for GlitchTracer {}

    impl TracerImpl for GlitchTracer {
        fn element_add_pad(&self, _ts: u64, element: &gst::Element, pad: &gst::Pad) {
            // We're receiving events in a way that doesn't seem logical, for instance
            // in the case of decodebin pads are linked before being added, etc.
            // To account for that we always tentatively create related entities...
            self.stream.insert_element(element);
            self.stream.insert_pad(pad);
        }

        fn element_change_state_post(
            &self,
            ts: u64,
            element: &gst::Element,
            change: gst::StateChange,
            result: Result<gst::StateChangeSuccess, gst::StateChangeError>,
        ) {
            if result.is_ok() {
                let id = Entity::from_hashable(element);
                let new_state = match change {
                    gst::StateChange::NullToReady => State::Ready,
                    gst::StateChange::ReadyToPaused => State::Paused,
                    gst::StateChange::PausedToPlaying => State::Playing,
                    gst::StateChange::PlayingToPaused => State::Paused,
                    gst::StateChange::PausedToReady => State::Ready,
                    gst::StateChange::ReadyToNull => State::Null,
                    _ => return,
                };
                self.stream.insert_element(element);
                self.stream.insert_one(id, new_state);
            } else {
                error!(
                    "Element {:?} failed to change state to {:?} at ts {}",
                    element, change, ts
                );
            }
        }

        fn element_new(&self, _ts: u64, element: &gst::Element) {
            self.stream.insert_element(element);
        }

        fn bin_add_post(&self, _ts: u64, bin: &gst::Bin, element: &gst::Element, _success: bool) {
            self.stream.insert_element(bin.as_ref());
            self.stream.insert_element(element);
            self.stream.insert_one(
                Entity::from_hashable(element),
                Child {
                    parent: Entity::from_hashable(bin),
                },
            )
        }

        fn pad_link_pre(&self, _ts: u64, src: &gst::Pad, sink: &gst::Pad) {
            self.stream.insert_link(src, sink, State::Pending);
        }

        fn pad_link_post(
            &self,
            _ts: u64,
            src: &gst::Pad,
            sink: &gst::Pad,
            result: Result<gst::PadLinkSuccess, gst::PadLinkError>,
        ) {
            let state = match result {
                Ok(_) => State::Done,
                Err(_) => State::Failed,
            };

            self.stream.insert_link(src, sink, state);
        }
    }
}
