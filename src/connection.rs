use async_stream::stream;
use bincode::{deserialize, serialize, Error as BincodeError};
use futures::{stream::FusedStream, Sink, SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, pin::Pin};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream, ToSocketAddrs},
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[derive(Serialize, Deserialize, Debug)]
pub enum Frame {
    Msg(String),
}

pub type FrameSender = Pin<Box<dyn Sink<Frame, Error = FrameError> + Send>>;
pub type FrameReceiver = Pin<Box<dyn FusedStream<Item = Result<Frame, FrameError>> + Send>>;

pub async fn listen_framed<S: ToSocketAddrs>(
    s: S,
) -> tokio::io::Result<impl FusedStream<Item = ((FrameSender, FrameReceiver), SocketAddr)>> {
    let mut listener = TcpListener::bind(s).await?;
    Ok(stream! {
        loop {
            match listener.accept().await {
                Ok((socket, addr)) => yield (frame_io(socket), addr),
                _ => break, // Is this right?
            }
        }
    })
}

pub async fn connect_framed<S: ToSocketAddrs>(
    s: S,
) -> tokio::io::Result<(FrameSender, FrameReceiver)> {
    let socket = TcpStream::connect(s).await?;
    Ok(frame_io(socket))
}

pub fn frame_io<IO>(io: IO) -> (FrameSender, FrameReceiver)
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

#[derive(Debug)]
pub enum FrameError {
    Io(std::io::Error),
    Bincode(BincodeError),
}

impl From<std::io::Error> for FrameError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}
impl From<BincodeError> for FrameError {
    fn from(err: BincodeError) -> Self {
        Self::Bincode(err)
    }
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Self::Io(ref err) => err.fmt(f),
            Self::Bincode(ref err) => err.fmt(f),
        }
    }
}

impl std::error::Error for FrameError {
    fn description(&self) -> &str {
        match *self {
            Self::Io(ref err) => err.description(),
            Self::Bincode(ref err) => err.description(),
        }
    }
}
