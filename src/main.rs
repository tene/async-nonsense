use async_stream::try_stream;
use futures::{
    pin_mut, select,
    stream::{Stream, StreamExt},
};
use std::net::SocketAddr;
use tokio::{
    io,
    net::{TcpListener, TcpStream, ToSocketAddrs},
    task,
};

fn bind_and_accept<A: ToSocketAddrs>(
    addr: A,
) -> impl Stream<Item = io::Result<(TcpStream, SocketAddr)>> {
    try_stream! {
        let mut listener = TcpListener::bind(addr).await?;

        loop {
            let (stream, addr) = listener.accept().await?;
            println!("received on {:?}", addr);
            yield (stream, addr);
        }
    }
}

async fn echo_server(s: TcpStream) -> io::Result<()> {
    let (mut recv, mut send) = io::split(s);
    io::copy(&mut recv, &mut send).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = bind_and_accept("127.0.0.1:8080").fuse();
    pin_mut!(listener);

    loop {
        select! {
        ms = listener.next() =>
            match ms {
                Some(Ok((socket, _))) => {
                    task::spawn(echo_server(socket));
                },
                _ => {}
            },
        }
    }
}
