pub mod framing;
pub use framing::*;

pub struct AgentId(String, u32);

pub enum Event {
    ConnectionEstablished,
    ConnectionTerminated,
}
