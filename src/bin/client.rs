use futures::{
    pin_mut, select,
    stream::{Stream, StreamExt},
};
use linefeed::{Interface, ReadResult};
use std::sync::Arc;
use tokio::{io, sync::mpsc::channel};

#[tokio::main]
async fn main() -> io::Result<()> {
    let interface = Arc::new(Interface::new("chat")?);
    interface.set_prompt("» ")?;
    let interface_r = interface.clone();
    let (mut lines_send, lines) = channel(128);
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
    let lines = lines.fuse();
    pin_mut!(lines);
    loop {
        select! {
            line = lines.next() => match line {
                Some(line) => writeln!(interface, "{}", line)?,
                None => break,
            }
        }
    }
    Ok(())
}
