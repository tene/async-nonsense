use futures::{pin_mut, select, SinkExt, StreamExt};
use std::{collections::HashMap, net::SocketAddr};
use tokio::{
    sync::mpsc::{channel, Receiver, Sender},
    task,
};

use chat::connection::{listen_framed, Frame, FrameReceiver, FrameSender};

enum Event {
    NewConnection((FrameSender, FrameReceiver), SocketAddr),
    Client(SocketAddr, Frame),
    ClosedConnection(SocketAddr),
}

impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Event::*;
        match self {
            NewConnection(_, addr) => write!(f, "NewConnection({:?})", addr),
            e => e.fmt(f),
        }
    }
}

async fn echo_thread(addr: SocketAddr, client_rx: FrameReceiver, mut events_tx: Sender<Event>) {
    pin_mut!(client_rx);
    loop {
        match client_rx.next().await {
            Some(Ok(frame)) => events_tx.send(Event::Client(addr, frame)).await.unwrap(),
            _ => break,
        }
    }
    events_tx.send(Event::ClosedConnection(addr)).await.unwrap();
}

async fn echo_server(events_tx: Sender<Event>, mut events_rx: Receiver<Event>) {
    let mut conns: HashMap<SocketAddr, FrameSender> = HashMap::new();
    loop {
        use Event::*;
        match events_rx.recv().await {
            Some(NewConnection((client_tx, client_rx), addr)) => {
                conns.insert(addr, client_tx);
                task::spawn(echo_thread(addr, client_rx, events_tx.clone()));
            }
            Some(Client(_addr, evt)) => match evt {
                Frame::Msg(line) => {
                    for client in conns.values_mut() {
                        client.send(Frame::Msg(line.clone())).await.unwrap();
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
    let incoming = listen_framed("127.0.0.1:8080").await?;
    let (mut events_tx, events_rx) = channel::<Event>(128);
    task::spawn(echo_server(events_tx.clone(), events_rx));
    pin_mut!(incoming);

    loop {
        select! {
            accepted = incoming.next() => match accepted {
                Some((socket, addr)) => events_tx.send(Event::NewConnection(socket, addr)).await?,
                _ => break,
            },
        }
    }
    Ok(())
}
