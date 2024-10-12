use crate::*;
use hecs::Entity;
use remoc::prelude::*;
use std::net::Ipv4Addr;
use tokio::net::TcpStream;
use tracing::*;

pub struct RecordingStream {
    pub tx: tokio::sync::broadcast::Sender<Command>,
}

impl RecordingStream {
    pub fn new() -> Self {
        // Spawn a tokio task that connects to the remoc server and forwards the events
        // from the broadcast channel. This avoids the overhead of spawning a new task
        // for each call.
        let (tx, _) = tokio::sync::broadcast::channel(2048);
        Self { tx }
    }

    pub fn connect(&self, ip: Ipv4Addr, port: u16) {
        info!("Connecting to {ip}:{port}");
        let rx = self.tx.subscribe();
        tokio::spawn(connect_client(ip, port, rx));
    }

    pub fn insert_one(&self, id: Entity, component: impl Into<SpawnOrInsert>) {
        debug!(
            "Inserting component, number of recievers = {}, buffer size = {}",
            self.tx.receiver_count(),
            self.tx.len()
        );
        let _ = self.tx.send(Command::SpawnOrInsert(id, component.into()));
    }

    pub fn remove_one<T>(&self, id: Entity) {
        let component = match std::any::type_name::<T>() {
            "Node" => Remove::Node,
            "Edge" => Remove::Edge,
            "State" => Remove::State,
            "Name" => Remove::Name,
            "TypeName" => Remove::TypeName,
            "Properties" => Remove::Properties,
            "Port" => Remove::Port,
            "Child" => Remove::Child,
            _ => panic!("Unsupported component type"),
        };
        let _ = self.tx.send(Command::Remove(id, component));
    }
}

pub async fn connect_client(
    ip: Ipv4Addr,
    port: u16,
    mut rx: tokio::sync::broadcast::Receiver<Command>,
) {
    let socket = TcpStream::connect((ip, port)).await.unwrap();
    info!("Connected to {ip}:{port}");
    let (socket_rx, socket_tx) = socket.into_split();
    let (conn, mut remote_tx, _): (_, _, rch::base::Receiver<()>) =
        remoc::Connect::io(remoc::Cfg::default(), socket_rx, socket_tx)
            .await
            .unwrap();
    tokio::spawn(conn);

    info!("Connected to server, waiting for commands");

    while let Ok(cmd) = rx.recv().await {
        debug!("Forwarding: {cmd:?}");
        remote_tx.send(cmd).await.unwrap();
    }
}
