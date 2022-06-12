use std::{net::SocketAddr, time::Duration};

use client::ClientFrameData;
use crossbeam_channel::Sender;
use event::{AsyncEventDelegate, GameEvent};
use laminar::Packet;
use server::ServerFrameData;
use update_buffer::NetworkUpdateBufferRef;

use self::{client::Client, server::Server};

mod client;
mod packet;
mod server;

const SERVER_ADDR: &str = "127.0.0.1:12351";
const POLL_INTERVAL: Duration = Duration::from_millis(50);

enum FrameUpdateImpl {
    Server(ServerFrameData),
    Client(ClientFrameData),
    Offline,
}

impl Default for FrameUpdateImpl {
    fn default() -> Self {
        Self::Offline
    }
}

#[derive(Default)]
pub struct FrameData {
    update_impl: FrameUpdateImpl,
}

impl FrameData {
    pub async fn update(&mut self, event_delegate: &AsyncEventDelegate<'_>) {
        use FrameUpdateImpl::*;

        for event in event_delegate.game_events() {
            use GameEvent::*;
            match event {
                NetworkRoleServer => {
                    self.update_impl = Server(Default::default());
                }
                NetworkRoleClient => {
                    self.update_impl = Client(Default::default());
                }
                NetworkRoleOffline => {
                    self.update_impl = Offline;
                }
                _ => {}
            }
        }

        match &mut self.update_impl {
            Server(frame_data) => {
                frame_data.update(event_delegate);
            }
            Client(frame_data) => {
                frame_data.update(event_delegate);
            }
            Offline => {}
        }
    }
}

enum FixedUpdateImpl {
    Offline,
    Client(Box<Client>),
    Server(Box<Server>),
}

impl Default for FixedUpdateImpl {
    fn default() -> Self {
        Self::Offline
    }
}

impl PartialEq<FrameUpdateImpl> for FixedUpdateImpl {
    fn eq(&self, other: &FrameUpdateImpl) -> bool {
        matches!(
            (self, other),
            (Self::Server(_), FrameUpdateImpl::Server(_))
                | (Self::Client(_), FrameUpdateImpl::Client(_))
                | (Self::Offline, FrameUpdateImpl::Offline)
        )
    }
}

#[derive(Default)]
pub struct FixedData {
    update_impl: FixedUpdateImpl,
}

impl FixedData {
    pub async fn swap(&mut self, frame_data: &mut FrameData) {
        if self.update_impl != frame_data.update_impl {
            self.update_impl = match &frame_data.update_impl {
                FrameUpdateImpl::Server(_) => FixedUpdateImpl::Server(Server::default().into()),
                FrameUpdateImpl::Client(_) => FixedUpdateImpl::Client(Client::default().into()),
                FrameUpdateImpl::Offline => FixedUpdateImpl::Offline,
            };
        }

        match &mut self.update_impl {
            FixedUpdateImpl::Server(server) => {
                let frame_data = match &mut frame_data.update_impl {
                    FrameUpdateImpl::Server(frame_data) => frame_data,
                    _ => unreachable!(),
                };
                server.swap(frame_data);
            }
            FixedUpdateImpl::Client(client) => {
                let frame_data = match &mut frame_data.update_impl {
                    FrameUpdateImpl::Client(frame_data) => frame_data,
                    _ => unreachable!(),
                };
                client.swap(frame_data);
            }
            FixedUpdateImpl::Offline => {}
        }
    }

    pub async fn update(&mut self, update_buffer: NetworkUpdateBufferRef<'_>) {
        use FixedUpdateImpl::*;
        match &mut self.update_impl {
            Server(server) => {
                server.update(update_buffer);
            }
            Client(client) => {
                client.update(update_buffer);
            }
            Offline => {}
        }
    }
}

fn broadcast_reliable_ordered<'a, I>(clients: I, sender: &Sender<Packet>, data: &[u8])
where
    I: IntoIterator<Item = &'a SocketAddr>,
{
    clients
        .into_iter()
        .map(|client| Packet::reliable_ordered(*client, data.to_vec(), None))
        .for_each(|packet| sender.send(packet).unwrap());
}

fn broadcast_unreliable_sequenced<'a, I>(clients: I, sender: &Sender<Packet>, data: &[u8])
where
    I: IntoIterator<Item = &'a SocketAddr>,
{
    clients
        .into_iter()
        .map(|client| Packet::unreliable_sequenced(*client, data.to_vec(), None))
        .for_each(|packet| sender.send(packet).unwrap());
}
