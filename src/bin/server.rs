use futures::{pin_mut, select, FutureExt, Sink, SinkExt, Stream, StreamExt};
use std::{collections::HashMap, net::SocketAddr};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{channel, Receiver, Sender},
    task,
};

use chat::event::{self, wrap_client_conn, ClientEvent, ServerEvent};

#[derive(Debug)]
enum Event {
    NewConnection(SocketAddr, TcpStream),
    Client(SocketAddr, ClientEvent),
    ClosedConnection(SocketAddr),
}

async fn echo_thread(
    addr: SocketAddr,
    client_rx: impl Stream<Item = Result<ClientEvent, event::Error>>,
    mut events_tx: Sender<Event>,
) {
    pin_mut!(client_rx);
    loop {
        match client_rx.next().await {
            Some(Ok(evt)) => events_tx.send(Event::Client(addr, evt)).await.unwrap(),
            _ => break,
        }
    }
    events_tx.send(Event::ClosedConnection(addr)).await.unwrap();
}

async fn echo_server(events_tx: Sender<Event>, mut events_rx: Receiver<Event>) {
    let mut conns: HashMap<
        SocketAddr,
        std::pin::Pin<Box<dyn Sink<ServerEvent, Error = event::Error> + Send>>,
    > = HashMap::new();
    loop {
        use Event::*;
        match events_rx.recv().await {
            Some(NewConnection(addr, stream)) => {
                let (client_tx, client_rx) = wrap_client_conn(stream);
                conns.insert(addr, Box::pin(client_tx));
                task::spawn(echo_thread(addr, client_rx, events_tx.clone()));
            }
            Some(Client(addr, evt)) => match evt {
                ClientEvent::Msg(line) => {
                    for client in conns.values_mut() {
                        client
                            .send(ServerEvent::Msg(addr, line.clone()))
                            .await
                            .unwrap();
                    }
                }
            },
            Some(ClosedConnection(addr)) => {
                conns.remove(&addr);
            }
            None => break,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    let (mut events_tx, events_rx) = channel::<Event>(128);
    task::spawn(echo_server(events_tx.clone(), events_rx));
    pin_mut!(listener);

    loop {
        select! {
            accepted = listener.accept().fuse() => match accepted {
                Ok((socket, addr)) => events_tx.send(Event::NewConnection(addr, socket)).await?,
                _ => break,
            },
        }
    }
    Ok(())
}
