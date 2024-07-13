use gst::glib;

glib::wrapper! {
    pub struct GlitchTracer(ObjectSubclass<imp::GlitchTracer>)
       @extends gst::Tracer, gst::Object;
}

mod imp {
    use glitch_data::{connect_client, ElementState, Event, LinkState};
    use gst::{glib, prelude::*, subclass::prelude::*};
    use log::*;
    use once_cell::sync::Lazy;

    static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
        gst::DebugCategory::new(
            "glitchtracing",
            gst::DebugColorFlags::all(),
            Some("Glitch client tracer"),
        )
    });

    pub struct GlitchTracer {
        pub tx: tokio::sync::mpsc::Sender<Event>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for GlitchTracer {
        const NAME: &'static str = "GlitchTracer";
        type Type = super::GlitchTracer;
        type ParentType = gst::Tracer;

        fn new() -> Self {
            // Spawn a tokio task that connects to the remoc server and forwards the events
            // from the crossbeam channel. This avoids the overhead of spawning a new task
            // for each hook.
            gst::debug!(CAT, "Creating tracer");

            let (tx, rx) = tokio::sync::mpsc::channel(32);

            tokio::spawn(connect_client(rx));

            Self { tx }
        }
    }

    impl ObjectImpl for GlitchTracer {
        fn constructed(&self) {
            self.parent_constructed();
            self.register_hook(TracerHook::BinAddPost);
            self.register_hook(TracerHook::ElementAddPad);
            self.register_hook(TracerHook::ElementRemovePad);
            self.register_hook(TracerHook::ElementChangeStatePost);
            self.register_hook(TracerHook::ElementNew);
            self.register_hook(TracerHook::PadLinkPost);
            self.register_hook(TracerHook::PadLinkPre);
        }
    }

    impl GstObjectImpl for GlitchTracer {}

    impl TracerImpl for GlitchTracer {
        fn element_add_pad(&self, _ts: u64, element: &gst::Element, pad: &gst::Pad) {
            assert_eq!(pad.parent_element().as_ref(), Some(element));
            let _ = self.tx.blocking_send(Event::AddPad {
                pad: pad.into(),
                element: element.into(),
            });
        }

        fn element_remove_pad(&self, _ts: u64, element: &gst::Element, pad: &gst::Pad) {
            assert_eq!(pad.parent_element().as_ref(), Some(element));
        }

        fn element_change_state_post(
            &self,
            ts: u64,
            element: &gst::Element,
            change: gst::StateChange,
            result: Result<gst::StateChangeSuccess, gst::StateChangeError>,
        ) {
            if result.is_ok() {
                let _ = self.tx.blocking_send(Event::ChangeElementState {
                    element: element.into(),
                    state: match change {
                        gst::StateChange::NullToReady => ElementState::Ready,
                        gst::StateChange::ReadyToPaused => ElementState::Paused,
                        gst::StateChange::PausedToPlaying => ElementState::Playing,
                        gst::StateChange::PlayingToPaused => ElementState::Paused,
                        gst::StateChange::PausedToReady => ElementState::Ready,
                        gst::StateChange::ReadyToNull => ElementState::Null,
                        _ => return,
                    },
                });
            } else {
                error!(
                    "Element {:?} failed to change state to {:?} at ts {}",
                    element, change, ts
                );
            }
        }

        fn element_new(&self, _ts: u64, element: &gst::Element) {
            let _ = self.tx.blocking_send(Event::NewElement(element.into()));
        }

        fn bin_add_post(&self, _ts: u64, bin: &gst::Bin, element: &gst::Element, _success: bool) {
            let _ = self.tx.blocking_send(Event::AddChildElement {
                child: element.into(),
                parent: bin.upcast_ref::<gst::Element>().into(),
            });
        }

        fn pad_link_pre(&self, _ts: u64, src: &gst::Pad, sink: &gst::Pad) {
            let _ = self.tx.blocking_send(Event::LinkPad {
                src_pad: src.into(),
                sink_pad: sink.into(),
                state: LinkState::Pending,
            });
        }

        fn pad_link_post(
            &self,
            _ts: u64,
            src_pad: &gst::Pad,
            sink_pad: &gst::Pad,
            result: Result<gst::PadLinkSuccess, gst::PadLinkError>,
        ) {
            let _ = self.tx.blocking_send(Event::LinkPad {
                src_pad: src_pad.into(),
                sink_pad: sink_pad.into(),
                state: match result {
                    Ok(_) => LinkState::Done,
                    Err(_) => LinkState::Failed,
                },
            });
        }
    }
}
