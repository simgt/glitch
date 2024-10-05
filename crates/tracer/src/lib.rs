mod exts;
mod tracer;

pub use exts::*;
pub use tracer::*;

mod gst_plugin {
    use once_cell::sync::Lazy;
    use std::mem::ManuallyDrop;

    static RT: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
    });

    fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
        let _ = ManuallyDrop::new(RT.enter());

        gst::Tracer::register(
            Some(plugin),
            "glitchtracing",
            <super::GlitchTracer as glib::types::StaticType>::static_type(),
        )?;

        Ok(())
    }

    gst::plugin_define!(
        glitch_tracer,
        env!("CARGO_PKG_DESCRIPTION"),
        plugin_init,
        env!("CARGO_PKG_VERSION"),
        "GPL",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_REPOSITORY"),
        env!("BUILD_REL_DATE")
    );
}
