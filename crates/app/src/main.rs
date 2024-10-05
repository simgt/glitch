use std::collections::HashMap;

use anyhow::Result;
use clap::Parser;
use eframe::egui;
use glitch_data::*;
#[cfg(not(feature = "reload"))]
use glitch_draw::*;
use hecs::Entity;
#[cfg(feature = "reload")]
use hot_lib::*;
use tracing::debug;
use tracing_subscriber::{prelude::*, EnvFilter};

#[cfg(feature = "reload")]
#[hot_lib_reloader::hot_module(
    dylib = "glitch_draw",
    lib_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/debug")
)]
mod hot_lib {
    use eframe::egui;
    pub use glitch_draw::DrawState;

    hot_functions_from_file!("crates/draw/src/draw.rs");

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}
}

#[derive(Parser)]
#[command(author, version, about)]
struct Args {}

pub struct App {
    world: hecs::World,
    remote_entities: HashMap<Entity, Entity>,
    #[allow(dead_code)]
    rt: tokio::runtime::Runtime,
    rx: tokio::sync::mpsc::Receiver<Command>,
    draw_state: DrawState,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, _args: Args) -> Self {
        let ctx = cc.egui_ctx.clone();
        // FIXME set dark and light themes when this is in a release: https://github.com/emilk/egui/pull/4744
        ctx.set_visuals(egui::Visuals {
            dark_mode: true,
            selection: egui::style::Selection {
                stroke: egui::Stroke::new(2.0, egui::Color32::from_rgb(127, 33, 160)),
                bg_fill: egui::Color32::from_rgb(77, 27, 97),
            },
            ..egui::Visuals::dark()
        });

        let (tx, rx) = tokio::sync::mpsc::channel(32);

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.spawn(serve(tx));

        Self {
            world: hecs::World::new(),
            remote_entities: HashMap::default(),
            rt,
            rx,
            draw_state: DrawState::default(),
        }
    }

    pub fn recv_commands(&mut self, ctx: &egui::Context) {
        while let Ok(mut cmd) = self.rx.try_recv() {
            cmd.translate_entities(&mut self.remote_entities, &mut self.world);
            debug!("Received command: {cmd:?}");
            cmd.run_on(&mut self.world);
        }

        // FIXME this is a hack to make sure the update function is recalled
        ctx.request_repaint();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.recv_commands(ctx);
        draw(&mut self.draw_state, &mut self.world, ctx, frame);
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

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size((1024.0, 768.0))
            .with_title_shown(false)
            .with_titlebar_shown(false)
            .with_fullsize_content_view(true),
        ..Default::default()
    };
    eframe::run_native(
        "Glitch",
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
            Ok(Box::new(App::new(cc, args)))
        }),
    )
}
