#![recursion_limit = "256"]
use futures::{pin_mut, select, SinkExt, StreamExt};
use slab::Slab;
use tokio::{
    sync::mpsc::{channel, Sender},
    task,
};

use chat::connection::{listen_framed_unix, Addr, Frame, FrameReceiver, FrameSender};

#[derive(Debug)]
enum Event {
    Client(usize, Frame),
    ClosedConnection(usize),
}

async fn echo_thread(id: usize, client_rx: FrameReceiver, mut events_tx: Sender<Event>) {
    pin_mut!(client_rx);
    loop {
        match client_rx.next().await {
            Some(Ok(frame)) => events_tx.send(Event::Client(id, frame)).await.unwrap(),
            _ => break,
        }
    }
    events_tx.send(Event::ClosedConnection(id)).await.unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //let incoming = listen_framed_tcp("127.0.0.1:8080").await?;
    let incoming = listen_framed_unix("chat.socket").await?;
    let (events_tx, events_rx) = channel::<Event>(128);
    let mut events_rx = events_rx.fuse();
    pin_mut!(incoming);

    let mut conns: Slab<(Addr, FrameSender)> = Slab::new();

    loop {
        select! {
            accepted = incoming.next() => match accepted {
                Some(((client_tx, client_rx), addr)) => {
                    let id = conns.insert((addr, client_tx));
                    task::spawn(echo_thread(id, client_rx, events_tx.clone()));
                },
                _ => break,
            },
            event = events_rx.next() => match event {
                Some(Event::Client(id, frame)) => {
                    for (_id, (_addr, client)) in conns.iter_mut() {
                        client.send(frame.clone()).await.unwrap();
                    };
                },
                Some(Event::ClosedConnection(id)) => {
                        conns.remove(id);
                },
                None => unreachable!(),
            }
        }
    }
    Ok(())
}
