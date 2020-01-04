use futures::{pin_mut, select, sink::SinkExt, stream::StreamExt};
use linefeed::{Interface, ReadResult};
use std::sync::Arc;
use tokio::sync::mpsc::channel;

use chat::connection::{connect_framed_unix, Frame};

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
    let (mut frames_tx, frames_rx) = connect_framed_unix("chat.socket").await?;
    let mut frames_rx = frames_rx;
    pin_mut!(input_lines);
    loop {
        select! {
            line = input_lines.next() => match line {
                Some(line) => {
                    let rv = frames_tx.send(Frame::Msg(line)).await?;
                },
                None => break,
            },
            evt = frames_rx.next() => match evt {
                Some(Ok(Frame::Msg(line))) => {
                    writeln!(interface, "{}", line)?;
                },
                _ => break,
            },
        }
    }
    Ok(())
}
