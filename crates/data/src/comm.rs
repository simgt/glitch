// Intended API of our communication layer
//
// Tracer can send any type that is serializable through serde.
// Server receives the data associated to an identifier, and can convert it
// to an hecs::CommandBuffer to be run on the world.

use crate::*;
use hecs::Entity;
use remoc::prelude::*;
use std::net::Ipv4Addr;
use tokio::net::{TcpListener, TcpStream};
use tracing::*;

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

pub async fn connect_client(mut rx: tokio::sync::mpsc::Receiver<Command>) {
    let ip = Ipv4Addr::LOCALHOST;
    let port = DEFAULT_PORT;
    println!("Connecting to {ip}:{port}");
    let socket = TcpStream::connect((ip, port)).await.unwrap();
    println!("Connected");
    let (socket_rx, socket_tx) = socket.into_split();
    let (conn, mut remote_tx, _): (_, _, rch::base::Receiver<()>) =
        remoc::Connect::io(remoc::Cfg::default(), socket_rx, socket_tx)
            .await
            .unwrap();
    tokio::spawn(conn);

    println!("Connected to server, waiting for events");

    while let Some(event) = rx.recv().await {
        println!("Sending event: {event:?}");
        remote_tx.send(event).await.unwrap();
    }
}

pub struct RecordingStream {
    pub tx: tokio::sync::mpsc::Sender<Command>,
}

impl RecordingStream {
    pub fn new() -> Self {
        // Spawn a tokio task that connects to the remoc server and forwards the events
        // from the crossbeam channel. This avoids the overhead of spawning a new task
        // for each call.
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        tokio::spawn(connect_client(rx));
        Self { tx }
    }

    pub fn insert_one(&self, id: Entity, component: impl Into<SpawnOrInsert>) {
        let _ = self
            .tx
            .blocking_send(Command::SpawnOrInsert(id, component.into()));
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
        let _ = self.tx.blocking_send(Command::Remove(id, component));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;
    use test_log::test;

    #[test(tokio::test)]
    async fn test_comm() {
        info!("Starting server and client");

        let (server_tx, mut server_rx) = tokio::sync::mpsc::channel(12);
        tokio::spawn(serve(server_tx));

        let (client_tx, client_rx) = tokio::sync::mpsc::channel(12);
        tokio::spawn(connect_client(client_rx));

        // Send a couple commands on client_tx, and compare them with server_rx
        let command1 = Command::SpawnOrInsert(Entity::DANGLING, Node {}.into());
        let command2 = Command::Remove(Entity::DANGLING, Remove::Edge);
        client_tx.send(command1.clone()).await.unwrap();
        client_tx.send(command2.clone()).await.unwrap();

        assert_eq!(server_rx.recv().await.unwrap(), command1);
        assert_eq!(server_rx.recv().await.unwrap(), command2);
    }
}
