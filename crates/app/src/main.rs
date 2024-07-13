use anyhow::Result;
use clap::Parser;
use eframe::egui;
#[cfg(not(feature = "reload"))]
use glitch_render::*;
#[cfg(feature = "reload")]
use hot_lib::*;
use tracing_subscriber::{prelude::*, EnvFilter};

#[cfg(feature = "reload")]
#[hot_lib_reloader::hot_module(
    dylib = "glitch_render",
    lib_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/debug")
)]
mod hot_lib {
    use eframe::egui;
    pub use glitch_render::AppState;

    hot_functions_from_file!("crates/render/src/render.rs");

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}
}

#[derive(Parser)]
#[command(author, version, about)]
struct Args {}

pub struct App {
    main_loop: glib::MainLoop,
    state: AppState,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, main_loop: glib::MainLoop, _args: Args) -> Self {
        let ctx = cc.egui_ctx.clone();
        let state = AppState::new(&ctx).expect("Failed to create state");

        Self { main_loop, state }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        render(&mut self.state, ctx, frame);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // FIXME set all pipelines to null
        self.main_loop.quit();
    }
}

fn main() -> Result<(), eframe::Error> {
    let args = Args::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    std::env::set_var(
        "GST_DEBUG_DUMP_DOT_DIR",
        std::env::current_dir().unwrap().canonicalize().unwrap(),
    );

    let main_loop = glib::MainLoop::new(None, false);
    std::thread::spawn({
        let main_loop = main_loop.clone();
        move || {
            main_loop.run();
        }
    });

    let options = eframe::NativeOptions {
        ..Default::default()
    };
    eframe::run_native(
        env!("CARGO_PKG_NAME"),
        options,
        Box::new(|cc| {
            // When hot reload is enabled, repaint after every lib change
            #[cfg(feature = "reload")]
            {
                let ctx = cc.egui_ctx.clone();
                std::thread::spawn(move || loop {
                    hot_lib::subscribe().wait_for_reload();
                    ctx.request_repaint();
                });
            }
            Ok(Box::new(App::new(cc, main_loop, args)))
        }),
    )
}
