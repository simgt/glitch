use crate::{Event, DEFAULT_PORT};
use remoc::prelude::*;
use std::net::Ipv4Addr;
use tokio::net::{TcpListener, TcpStream};
use tracing::*;

pub async fn serve(tx: tokio::sync::mpsc::Sender<Event>) {
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
                    rch::base::Receiver<Event>,
                ) = remoc::Connect::io(remoc::Cfg::default(), socket_rx, socket_tx)
                    .await
                    .unwrap();
                tokio::spawn(conn);
                debug!("Remoc connection established, waiting for events");

                let tx = tx.clone();
                tokio::spawn(async move {
                    while let Some(event) = remote_rx.recv().await.unwrap() {
                        debug!("Received event: {event:?}");
                        let _ = tx.send(event).await;
                    }
                });
            }
            Err(e) => {
                error!("Error accepting connection: {e}");
            }
        }
    }
}

pub async fn connect_client(mut rx: tokio::sync::mpsc::Receiver<Event>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use test_log::test;

    #[test(tokio::test)]
    async fn test_comm() {
        info!("Starting server and client");

        let (server_tx, mut server_rx) = tokio::sync::mpsc::channel(12);
        tokio::spawn(serve(server_tx));

        let (client_tx, client_rx) = tokio::sync::mpsc::channel(12);
        tokio::spawn(connect_client(client_rx));

        // Send a couple events on client_tx, and compare them with server_rx
        let event1 = Event::ChangeElementState {
            element: Element {
                id: RemoteId(0),
                name: "name".to_string().into(),
                type_name: "type".to_string().into(),
                properties: Default::default(),
                node: Node,
            },
            state: State::Playing,
        };
        let event2 = Event::LinkPad {
            src_pad: Pad {
                id: RemoteId(1),
                name: "src".to_string().into(),
                port: Port::Input,
            },
            sink_pad: Pad {
                id: RemoteId(2),
                name: "sink".to_string().into(),
                port: Port::Input,
            },
            state: State::Pending,
        };
        client_tx.send(event1.clone()).await.unwrap();
        client_tx.send(event2.clone()).await.unwrap();

        assert_eq!(server_rx.recv().await.unwrap(), event1);
        assert_eq!(server_rx.recv().await.unwrap(), event2);
    }
}
