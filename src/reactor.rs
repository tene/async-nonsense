use futures::SinkExt;
use slab::Slab;
use tokio::sync::mpsc::Sender;

use crate::{peers::Peers, types::*};

pub(crate) struct Reactor {
    sessions: Slab<(AgentId, String, FrameSender)>,
    peers: Peers,
    observers: Vec<Sender<String>>,
}

impl Reactor {
    pub fn new() -> Self {
        let sessions: Slab<(AgentId, String, FrameSender)> = Slab::new();
        let peers = Peers::new();
        let observers = Vec::new();
        Reactor {
            sessions,
            peers,
            observers,
        }
    }
    pub async fn link_ready(&mut self, addr: String, id: AgentId, tx: FrameSender) -> usize {
        let link = self.sessions.insert((id.clone(), addr, tx));
        self.peers.peer(link, id);
        link
    }
    pub async fn link_lost(&mut self, link: usize) {
        self.sessions.remove(link);
        let _lost = self.peers.drop_link(link);
    }
    pub async fn handle_message(&mut self, link: usize, msg: Message) {
        match msg {
            Message::Peers(map) => {
                let (id, _addr, ref mut _tx) = &mut self.sessions[link];
                let (_new, _lost) = self.peers.update(link, id.clone(), map);
            }
            Message::Broadcast(b) => {
                for (l2, (_, _, tx)) in self.sessions.iter_mut() {
                    if l2 == link {
                        continue;
                    }
                    let _ = tx.send(Frame::Message(Message::Broadcast(b.clone()))).await;
                }
                for tx in &mut self.observers {
                    // XXX TODO detect and remove stale observers
                    let _ = tx.send(b.clone()).await;
                }
            }
        }
    }
    pub async fn handle_request(&mut self, rq: Request) {
        match rq {
            Request::Broadcast(b) => {
                for (_, (_, _, tx)) in self.sessions.iter_mut() {
                    let _ = tx.send(Frame::Message(Message::Broadcast(b.clone()))).await;
                }
                for tx in &mut self.observers {
                    // XXX TODO detect and remove stale observers
                    let _ = tx.send(b.clone()).await;
                }
            }
            Request::Observe(tx) => {
                self.observers.push(tx);
            }
        }
    }
}
