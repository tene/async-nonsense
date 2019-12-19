use futures::{pin_mut, select, FutureExt};
use std::{collections::HashMap, net::SocketAddr};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf},
    net::{TcpListener, TcpStream},
    sync::mpsc::{channel, Receiver, Sender},
    task,
};

#[derive(Debug)]
enum Event {
    NewConnection(SocketAddr, TcpStream),
    Msg(SocketAddr, String),
    ClosedConnection(SocketAddr),
}

async fn echo_thread(
    addr: SocketAddr,
    client_rx: ReadHalf<TcpStream>,
    mut events_tx: Sender<Event>,
) {
    let mut client_lines = BufReader::new(client_rx).lines();
    loop {
        match client_lines.next_line().await {
            Ok(Some(line)) => {
                events_tx.send(Event::Msg(addr, line)).await.unwrap();
            }
            _ => break,
        }
    }
    events_tx.send(Event::ClosedConnection(addr)).await.unwrap();
}

async fn echo_server(events_tx: Sender<Event>, mut events_rx: Receiver<Event>) {
    let mut conns: HashMap<SocketAddr, WriteHalf<TcpStream>> = HashMap::new();
    loop {
        use Event::*;
        match events_rx.recv().await {
            Some(NewConnection(addr, stream)) => {
                let (client_rx, client_tx) = io::split(stream);
                conns.insert(addr, client_tx);
                task::spawn(echo_thread(addr, client_rx, events_tx.clone()));
            }
            Some(Msg(addr, line)) => {
                let msg: String = format!("{}: {}\n", addr, line);
                for client in conns.values_mut() {
                    client.write_all(msg.as_bytes()).await.unwrap();
                }
            }
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
