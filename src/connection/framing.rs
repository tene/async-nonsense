use async_stream::stream;
use bincode::{deserialize, serialize};
use futures::{SinkExt, StreamExt};
use std::path::Path;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream, ToSocketAddrs, UnixListener, UnixStream},
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::types::*;

pub(crate) async fn listen_framed_tcp<S: ToSocketAddrs>(s: S) -> tokio::io::Result<FrameListener> {
    let mut listener = TcpListener::bind(s).await?;
    Ok(Box::pin(stream! {
        loop {
            match listener.accept().await {
                Ok((socket, addr)) => yield (frame_io(socket), Addr::Tcp(addr)),
                _ => break, // Is this right?
            }
        }
    }))
}
pub(crate) async fn listen_framed_unix<P: AsRef<Path>>(
    path: P,
) -> tokio::io::Result<FrameListener> {
    let mut listener = UnixListener::bind(path)?;
    Ok(Box::pin(stream! {
        loop {
            match listener.accept().await {
                Ok((socket, addr)) => yield (frame_io(socket), Addr::Unix(addr)),
                _ => break, // Is this right?
            }
        }
    }))
}

pub(crate) async fn connect_framed_tcp<S: ToSocketAddrs>(s: S) -> tokio::io::Result<Frames> {
    let socket = TcpStream::connect(s).await?;
    Ok(frame_io(socket))
}

pub(crate) async fn connect_framed_unix<P: AsRef<Path>>(path: P) -> tokio::io::Result<Frames> {
    let socket = UnixStream::connect(path).await?;
    Ok(frame_io(socket))
}

pub(crate) fn frame_io<IO>(io: IO) -> Frames
where
    IO: 'static + AsyncRead + AsyncWrite + Send,
{
    let framed = Framed::new(io, LengthDelimitedCodec::new());
    let (tx, rx) = framed.split();
    let frames_tx = tx
        .with(|f: Frame| async move { serialize(&f).map(Into::into) })
        .sink_err_into();
    let frames_rx = rx.then(|buf| async move { Ok(deserialize::<Frame>(&buf?)?) });
    (Box::pin(frames_tx), Box::pin(frames_rx.fuse()))
}
