use bincode::Error as BincodeError;
use futures::{stream::FusedStream, Sink};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub use crate::AgentId;

pub(crate) enum Session {
    Ready(String, AgentId, Frames),
    Message(usize, Message),
    Closed(usize),
    Request(Request),
}

#[derive(Debug)]
pub(crate) enum Request {
    Broadcast(String),
    Observe(tokio::sync::mpsc::Sender<String>),
}

#[derive(Debug)]
pub(crate) enum Addr {
    Tcp(std::net::SocketAddr),
    Unix(std::os::unix::net::SocketAddr),
}

// TODO: Split into Handshake and Authenticated protocols???
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) enum Frame {
    Hello(AgentId),
    Message(Message),
    Error(String),
    Goodbye,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Message {
    Broadcast(String),
    Peers(Vec<(AgentId, usize)>),
}

pub(crate) type FrameSender = Pin<Box<dyn Sink<Frame, Error = FrameError> + Send + Sync>>;
pub(crate) type FrameReceiver =
    Pin<Box<dyn FusedStream<Item = Result<Frame, FrameError>> + Send + Sync>>;
pub(crate) type FrameListener = Pin<Box<dyn FusedStream<Item = (Frames, Addr)> + Send + Sync>>;
pub(crate) type Frames = (FrameSender, FrameReceiver);

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
