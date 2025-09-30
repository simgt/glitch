use anyhow::Result;
use clap::Parser;
use eframe::egui;
use glitch_common::*;
#[cfg(not(feature = "reload"))]
use glitch_ui::*;

#[cfg(feature = "reload")]
use hot_lib::*;
use remoc::prelude::*;
use ser::load_datastore;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use tokio::net::TcpListener;
use tracing::debug;
use tracing::{error, info};
use tracing_subscriber::{prelude::*, EnvFilter};

#[cfg(feature = "reload")]
#[hot_lib_reloader::hot_module(
    dylib = "glitch_ui",
    lib_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/debug")
)]
mod hot_lib {
    use eframe::egui;
    pub use glitch_common::DataStore;
    pub use glitch_ui::UiState;

    hot_functions_from_file!("crates/ui/src/ui.rs");

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}
}

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// Load content from a file
    #[clap(short, long)]
    load: Option<PathBuf>,
}

pub struct App {
    data_store: DataStore,
    #[allow(dead_code)]
    rt: tokio::runtime::Runtime,
    rx: tokio::sync::mpsc::Receiver<Command>,
    ui_state: UiState,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
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

        let data_store = if let Some(path) = args.load {
            load_datastore(&path).unwrap()
        } else {
            DataStore::default()
        };

        Self {
            data_store,
            rt,
            rx,
            ui_state: UiState::default(),
        }
    }

    pub fn recv_commands(&mut self, ctx: &egui::Context) {
        while let Ok(cmd) = self.rx.try_recv() {
            debug!("Received command: {cmd:?}");
            self.data_store.record_command(cmd);
        }

        // FIXME this is a hack to make sure the update function is recalled
        ctx.request_repaint();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.recv_commands(ctx);
        show_ui(&mut self.ui_state, &mut self.data_store, ctx, frame);
    }
}

pub async fn serve(tx: tokio::sync::mpsc::Sender<Command>) {
    // Going through tokio's mpsc because remoc's channel doesn't provide
    // sync methods, which is needed for the UI code
    info!(
        "Binding server on {ip}:{port}",
        ip = Ipv4Addr::LOCALHOST,
        port = DEFAULT_PORT
    );
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, DEFAULT_PORT))
        .await
        .unwrap();
    debug!("Socket bound, waiting for connection");

    loop {
        match listener.accept().await {
            Ok((socket, _)) => {
                let (socket_rx, socket_tx) = socket.into_split();
                let (conn, _, mut remote_rx): (
                    _,
                    rch::base::Sender<()>,
                    rch::base::Receiver<Command>,
                ) = remoc::Connect::io(remoc::Cfg::default(), socket_rx, socket_tx)
                    .await
                    .unwrap();
                tokio::spawn(conn);
                debug!("Remoc connection established, waiting for events");

                let tx = tx.clone();
                tokio::spawn(async move {
                    while let Some(cmd) = remote_rx.recv().await.unwrap() {
                        debug!("Received command: {cmd:?}");
                        let _ = tx.send(cmd).await;
                    }
                });
            }
            Err(e) => {
                error!("Error accepting connection: {e}");
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;
    use glitch_common::client::connect_client;
    use hecs::Entity;
    use test_log::test;

    #[test(tokio::test)]
    async fn test_comm() {
        info!("Starting server and client");

        let ip = Ipv4Addr::LOCALHOST;
        let port = DEFAULT_PORT;

        let (server_tx, mut server_rx) = tokio::sync::mpsc::channel(12);
        tokio::spawn(serve(server_tx));

        let (client_tx, _) = tokio::sync::broadcast::channel(12);
        let client_rx = client_tx.subscribe();
        tokio::spawn(connect_client(ip, port, client_rx));

        // Send a couple commands on client_tx, and compare them with server_rx
        let command1 = Command::SpawnOrInsert(Entity::DANGLING, Node {}.into());
        let command2 = Command::Remove(Entity::DANGLING, Remove::Edge);
        client_tx.send(command1.clone()).unwrap();
        client_tx.send(command2.clone()).unwrap();

        assert_eq!(server_rx.recv().await.unwrap(), command1);
        assert_eq!(server_rx.recv().await.unwrap(), command2);
    }
}
