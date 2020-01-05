pub mod framing;
pub use framing::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentId {
    hostname: String,
    pid: u32,
}

fn guess_hostname() -> String {
    match hostname::get() {
        Ok(h) => match h.into_string() {
            Ok(h) => h,
            Err(_) => "Invalid Unicode?".into(),
        },
        Err(_) => "Unknown?".into(),
    }
}

impl AgentId {
    pub fn new<S: Into<String>>(hostname: S, pid: u32) -> Self {
        let hostname = hostname.into();
        Self { hostname, pid }
    }
    pub fn new_local() -> Self {
        let hostname = guess_hostname();
        let pid = std::process::id();
        Self { hostname, pid }
    }
}

pub enum Event {
    ConnectionEstablished,
    ConnectionTerminated,
}
