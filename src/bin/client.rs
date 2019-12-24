use futures::{pin_mut, select, sink::SinkExt, stream::StreamExt};
use linefeed::{Interface, ReadResult};
use std::sync::Arc;
use tokio::sync::mpsc::channel;

use chat::event::{connect_to_server, ClientEvent, ServerEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let interface = Arc::new(Interface::new("chat")?);
    interface.set_prompt("» ")?;
    let interface_r = interface.clone();
    let (mut lines_send, input_lines) = channel(128);
    std::thread::spawn(move || loop {
        match interface_r.read_line() {
            Ok(ReadResult::Input(line)) => {
                lines_send.try_send(line).expect("lines buffer full?");
            }
            Ok(ReadResult::Eof) => {
                break;
            }
            _ => {}
        }
    });
    let input_lines = input_lines.fuse();
    let (mut events_tx, events_rx) = connect_to_server("127.0.0.1:8080").await?;
    let mut events_rx = events_rx.fuse();
    pin_mut!(input_lines);
    loop {
        select! {
            line = input_lines.next() => match line {
                Some(line) => {
                    let rv = events_tx.send(ClientEvent::Msg(line)).await?;
                },
                None => break,
            },
            evt = events_rx.next() => match evt {
                Some(Ok(ServerEvent::Msg(addr, line))) => {
                    writeln!(interface, "{}: {}", addr, line)?;
                },
                _ => break,
            },
        }
    }
    Ok(())
}
