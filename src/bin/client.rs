use futures::{pin_mut, select, stream::StreamExt};
use linefeed::{Interface, ReadResult};
use std::sync::Arc;
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::mpsc::channel,
};

#[tokio::main]
async fn main() -> io::Result<()> {
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
    let mut socket = TcpStream::connect("127.0.0.1:8080").await?;
    let (recv, send) = socket.split();
    let mut server_lines = BufReader::new(recv).lines().fuse();
    pin_mut!(input_lines, send);
    loop {
        select! {
            line = input_lines.next() => match line {
                Some(line) => {
                    let rv = send.write_all(format!("{}\n", line).as_bytes()).await?;
                },
                None => break,
            },
            line = server_lines.next() => match line {
                Some(Ok(line)) => {
                    writeln!(interface, "{}", line)?;
                },
                _ => break,
            },
        }
    }
    Ok(())
}
