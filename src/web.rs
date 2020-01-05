use futures::{select, SinkExt, StreamExt};
use slab::Slab;
use std::{path::Path, sync::Arc};
use tokio::{
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
    task,
};

use crate::connection::{
    connect_framed_unix, listen_framed_unix, AgentId, Frame, FrameListener, FrameSender, Frames,
};

#[derive(Clone)]
pub struct Web {
    pub id: AgentId,
    control: Sender<Control>,
}

enum Control {
    Connect(String, Frames),
    Listen(String, FrameListener),
    Broadcast(String),
}

impl std::fmt::Debug for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Control::*;
        match self {
            Connect(name, _) => write!(f, "Connect({})", name),
            Listen(name, _) => write!(f, "Connect({})", name),
            Broadcast(s) => s.fmt(f),
        }
    }
}

impl Web {
    pub async fn new() -> Self {
        let id = AgentId::new_local();
        let (control, rx) = channel(256);
        task::spawn(run_web(id.clone(), control.clone(), rx));
        Self { id, control }
    }
    pub async fn connect_unix<P: AsRef<Path>>(
        &mut self,
        p: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let name: String = p.as_ref().to_string_lossy().into_owned();
        let frames = connect_framed_unix(p).await?;
        self.control.send(Control::Connect(name, frames)).await?;
        Ok(())
    }
    pub async fn listen_unix<P: AsRef<Path>>(
        &mut self,
        p: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let name: String = p.as_ref().to_string_lossy().into_owned();
        let listen: FrameListener = listen_framed_unix(p).await?;
        self.control.send(Control::Listen(name, listen)).await?;
        Ok(())
    }
    pub async fn broadcast<S: Into<String>>(
        &mut self,
        msg: S,
    ) -> Result<(), impl std::error::Error> {
        self.control.send(Control::Broadcast(msg.into())).await
    }
    /*
        pub fn broadcast();
        pub fn debug() -> Stream;
        pub fn iter_agents() -> Iter<AgentID>;
        pub fn iter_conn_map() -> Iter<(Addr, AgentID, HashMap<AgentID, Weight>)>;
        pub fn iter_subscription() -> Iter<Foo>;
        pub fn publish();
        pub fn iter_index();
    */
}

async fn run_web(id: AgentId, ctl_tx: Sender<Control>, ctl_rx: Receiver<Control>) {
    let mut ctl_rx = ctl_rx.fuse();
    let conns = Arc::new(Mutex::new(Slab::new()));
    loop {
        select! {
            request = ctl_rx.next() => {
                use Control::*;
                let request = match request {
                    Some(rq) => rq,
                    None => break,
                };
                match request {
                    Connect(name, frames) => {
                        let conns = Arc::clone(&conns);
                        let id = id.clone();
                        task::spawn_local(handle_connection(id, conns, frames));
                    },
                    Listen(name, mut listener) => {
                        let mut ctl_tx = ctl_tx.clone();
                        task::spawn({async move {
                            loop {
                                let (frames, addr) = match listener.next().await {
                                    Some(x) => x,
                                    None => break,
                                };
                                let name = format!("{:?}", addr);
                                ctl_tx.send(Connect(name, frames)).await.expect("Control socket closed??");
                            }
                        }});
                    },
                    Broadcast(msg) => {},
                }
            },
        }
    }
}

async fn handle_connection(
    id: AgentId,
    conns: Arc<Mutex<Slab<FrameSender>>>,
    (mut frame_tx, mut frame_rx): Frames,
) {
    let _ = frame_tx.send(Frame::Hello(id));
    match frame_rx.next().await {
        Some(Ok(Frame::Hello(_other_id))) => {
            let idx = conns.lock().await.insert(frame_tx);
            let mut err_desc: Option<String> = None;
            loop {
                match frame_rx.next().await {
                    Some(Ok(frame)) => match frame {
                        Frame::Msg(_) => {
                            for (key, tx) in conns.lock().await.iter_mut() {
                                if key == idx {
                                    continue;
                                } else {
                                    let _ = tx.send(frame.clone()).await;
                                }
                            }
                        }
                        Frame::Hello(_) => {
                            err_desc.replace("Unexpected Hello".to_owned());
                            break;
                        }
                        Frame::Error(e) => {
                            eprintln!("{}: Error {}", idx, e);
                            break;
                        }
                    },
                    _ => break,
                }
            }
            let mut frame_tx = conns.lock().await.remove(idx);
            if let Some(e) = err_desc {
                let _ = frame_tx.send(Frame::Error(e)).await;
            }
        }
        frame => {
            let _ = frame_tx
                .send(Frame::Error(format!("Expected Hello, got:\n{:?}", frame)))
                .await;
        }
    }
}
