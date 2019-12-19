use futures::{Sink, SinkExt, Stream, StreamExt, TryStreamExt};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[derive(Serialize, Deserialize, Debug)]
enum ClientEvent {
    Msg(String),
}

#[derive(Serialize, Deserialize, Debug)]
enum ServerEvent {
    Msg(SocketAddr, String),
}

pub fn build_codec<IO, TX, RX>(
    io: IO,
) -> (
    impl Sink<TX, Error = Box<dyn std::error::Error>>,
    impl Stream<Item = Result<RX, std::io::Error>>,
)
where
    IO: AsyncRead + AsyncWrite,
    TX: Serialize,
    RX: DeserializeOwned,
{
    let framed = Framed::new(io, LengthDelimitedCodec::new());
    let (tx, rx) = framed.split();
    let events_tx =
        tx.with(|event: TX| futures::future::ok(bincode::serialize(&event).unwrap().into()));
    let events_rx = rx.map_ok(|buf| bincode::deserialize::<RX>(&buf).unwrap());
    (events_tx, events_rx)
}
