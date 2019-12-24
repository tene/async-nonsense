use futures::{Sink, SinkExt, Stream, StreamExt, TryStreamExt};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpStream, ToSocketAddrs},
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientEvent {
    Msg(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerEvent {
    Msg(SocketAddr, String),
}

pub async fn connect_to_server<S: ToSocketAddrs>(
    s: S,
) -> tokio::io::Result<(
    impl Sink<ClientEvent, Error = Error> + Send,
    impl Stream<Item = Result<ServerEvent, Error>> + Send,
)> {
    let socket = TcpStream::connect(s).await?;
    Ok(build_codec(socket))
}

pub fn wrap_client_conn<IO>(
    io: IO,
) -> (
    impl Sink<ServerEvent, Error = Error> + Send,
    impl Stream<Item = Result<ClientEvent, Error>> + Send,
)
where
    IO: AsyncRead + AsyncWrite + Send,
{
    build_codec(io)
}

pub fn build_codec<IO, TX, RX>(
    io: IO,
) -> (
    impl Sink<TX, Error = Error> + Send,
    impl Stream<Item = Result<RX, Error>> + Send,
)
where
    IO: AsyncRead + AsyncWrite + Send,
    TX: Serialize + Send,
    RX: DeserializeOwned + Send,
{
    let framed = Framed::new(io, LengthDelimitedCodec::new());
    let (tx, rx) = framed.split();
    let events_tx =
        tx.with(|event: TX| futures::future::ok(bincode::serialize(&event).unwrap().into()));
    let events_rx = rx
        .map_ok(|buf| bincode::deserialize::<RX>(&buf).unwrap())
        .map_err(|e| e.into());
    (events_tx, events_rx)
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(err)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Error::Io(ref err) => err.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Io(ref err) => err.description(),
        }
    }
}
