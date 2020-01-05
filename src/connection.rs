pub mod framing;
pub use framing::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AgentId {
    name: String,
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
    pub fn new<S: Into<String>>(name: S) -> Self {
        let name = name.into();
        Self { name }
    }
    pub fn new_local() -> Self {
        let hostname = guess_hostname();
        let pid = std::process::id();
        let name = format!("{}+{}", hostname, pid).into();
        Self { name }
    }
}

pub enum Event {
    ConnectionEstablished,
    ConnectionTerminated,
}
