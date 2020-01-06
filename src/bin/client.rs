use futures::{pin_mut, select, stream::StreamExt};
use linefeed::{Interface, ReadResult};
use std::sync::Arc;
use tokio::sync::mpsc::channel;

use chat::web::Web;

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
    let mut web = Web::new().await;
    web.connect_unix("chat.socket").await?;
    let msg_rx = web.observe().await;
    let msg_rx = msg_rx.fuse();
    pin_mut!(input_lines, msg_rx);
    loop {
        select! {
            line = input_lines.next() => match line {
                Some(line) => {
                    web.broadcast(line).await?;
                },
                None => break,
            },
            msg = msg_rx.next() => match msg {
                Some(line) => {
                    writeln!(interface, "{}", line)?;
                },
                _ => break,
            },
        }
    }
    Ok(())
}
