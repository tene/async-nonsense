use futures::{SinkExt, StreamExt};
use std::path::Path;
use tokio::{
    net::ToSocketAddrs,
    sync::mpsc::{channel, Receiver, Sender},
    task,
};

use crate::{
    connection::framing::{
        connect_framed_tcp, connect_framed_unix, listen_framed_tcp, listen_framed_unix,
    },
    reactor::Reactor,
    types::{Frame, FrameListener, FrameReceiver, Frames, Request, Session},
    AgentId,
};

#[derive(Clone)]
pub struct Web {
    pub id: AgentId,
    control: Sender<Control>,
    request: Sender<Request>,
}

enum Control {
    Connect(String, Frames),
    Listen(String, FrameListener),
}

impl std::fmt::Debug for Control {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Control::*;
        match self {
            Connect(name, _) => write!(f, "Connect({})", name),
            Listen(name, _) => write!(f, "Listen({})", name),
        }
    }
}

impl Web {
    pub async fn new() -> Self {
        let id = AgentId::new_local();
        let (control, request) = spawn_tasks(id.clone()).await;
        Self {
            id,
            control,
            request,
        }
    }
    pub async fn connect_unix<P: AsRef<Path>>(
        &mut self,
        p: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let addr: String = p.as_ref().to_string_lossy().into_owned();
        let frames = connect_framed_unix(p).await?;
        self.control.send(Control::Connect(addr, frames)).await?;
        Ok(())
    }
    pub async fn listen_unix<P: AsRef<Path>>(
        &mut self,
        p: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let addr: String = p.as_ref().to_string_lossy().into_owned();
        let listen: FrameListener = listen_framed_unix(p).await?;
        self.control.send(Control::Listen(addr, listen)).await?;
        Ok(())
    }
    pub async fn connect_tcp<S: ToSocketAddrs + AsRef<str>>(
        &mut self,
        s: S,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let addr: String = s.as_ref().to_owned();
        let frames = connect_framed_tcp(s).await?;
        self.control.send(Control::Connect(addr, frames)).await?;
        Ok(())
    }
    pub async fn listen_tcp<S: ToSocketAddrs + AsRef<str>>(
        &mut self,
        s: S,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let addr: String = s.as_ref().to_owned();
        let listen: FrameListener = listen_framed_tcp(s).await?;
        self.control.send(Control::Listen(addr, listen)).await?;
        Ok(())
    }
    pub async fn broadcast<S: Into<String>>(
        &mut self,
        msg: S,
    ) -> Result<(), impl std::error::Error> {
        self.request.send(Request::Broadcast(msg.into())).await
    }
    pub async fn observe(&mut self) -> Receiver<String> {
        let (tx, rx) = channel(128);
        let _ = self.request.send(Request::Observe(tx)).await;
        rx
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

async fn spawn_tasks(id: AgentId) -> (Sender<Control>, Sender<Request>) {
    let (ctl_tx, ctl_rx) = channel(128);
    let (mut session_tx, session_rx) = channel(1024);
    let (rq_tx, mut rq_rx) = channel(128);
    task::spawn(reactor_task(id.clone(), session_tx.clone(), session_rx));
    task::spawn(listener_task(
        id,
        ctl_tx.clone(),
        ctl_rx,
        session_tx.clone(),
    ));
    task::spawn(async move {
        while let Some(rq) = rq_rx.next().await {
            if session_tx.send(Session::Request(rq)).await.is_err() {
                break;
            }
        }
    });
    (ctl_tx, rq_tx)
}

async fn reactor_task(
    _my_id: AgentId,
    session_tx: Sender<Session>,
    mut session_rx: Receiver<Session>,
) {
    let mut reactor = Reactor::new();
    loop {
        match session_rx.next().await {
            Some(s) => match s {
                Session::Ready(addr, id, (tx, rx)) => {
                    let link = reactor.link_ready(addr, id, tx).await;
                    task::spawn(wrap_session(link, session_tx.clone(), rx));
                }
                Session::Closed(link) => {
                    reactor.link_lost(link).await;
                }
                Session::Message(link, m) => {
                    reactor.handle_message(link, m).await;
                }
                Session::Request(rq) => {
                    reactor.handle_request(rq).await;
                }
            },
            None => break,
        }
    }
}

async fn wrap_session(link: usize, mut tx: Sender<Session>, mut rx: FrameReceiver) {
    loop {
        match rx.next().await {
            Some(Ok(Frame::Message(m))) => {
                let _ = tx.send(Session::Message(link, m)).await;
            }
            Some(_mf) => {
                // TODO implement Session::Error(link, SessionError)
                break;
            }
            _ => {
                break;
            }
        }
    }
    let _ = tx.send(Session::Closed(link)).await;
}

async fn handshake_connection(
    addr: String,
    id: AgentId,
    (mut frame_tx, mut frame_rx): Frames,
    mut session_tx: Sender<Session>,
) {
    let _ = frame_tx.send(Frame::Hello(id)).await;
    match frame_rx.next().await {
        Some(Ok(Frame::Hello(id))) => {
            let _ = session_tx
                .send(Session::Ready(addr, id, (frame_tx, frame_rx)))
                .await;
        }
        frame => {
            let _ = frame_tx
                .send(Frame::Error(format!("Expected Hello, got:\n{:?}", frame)))
                .await;
        }
    }
}

async fn listener_task(
    id: AgentId,
    ctl_tx: Sender<Control>,
    mut ctl_rx: Receiver<Control>,
    session_tx: Sender<Session>,
) {
    loop {
        use Control::*;
        match ctl_rx.next().await {
            Some(r) => match r {
                Connect(addr, frames) => {
                    task::spawn(handshake_connection(
                        addr,
                        id.clone(),
                        frames,
                        session_tx.clone(),
                    ));
                }
                Listen(_name, mut listener) => {
                    let mut ctl_tx = ctl_tx.clone();
                    task::spawn({
                        async move {
                            loop {
                                let (frames, addr) = match listener.next().await {
                                    Some(x) => x,
                                    None => break,
                                };
                                let addr = format!("{:?}", addr);
                                ctl_tx
                                    .send(Connect(addr, frames))
                                    .await
                                    .expect("Control socket closed??");
                            }
                        }
                    });
                }
            },
            None => break,
        }
    }
}
