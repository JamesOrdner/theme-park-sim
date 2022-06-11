use std::time::Duration;

use event::SyncEventDelegate;
use update_buffer::NetworkUpdateBufferRef;

use self::{client::Client, server::Server};

mod client;
mod packet;
mod server;

const SERVER_ADDR: &str = "127.0.0.1:12351";
const POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Offline,
    Client,
    Server,
}

impl Default for Role {
    fn default() -> Self {
        Self::Offline
    }
}

enum RoleImpl {
    Offline,
    Client(Box<Client>),
    Server(Box<Server>),
}

impl Default for RoleImpl {
    fn default() -> Self {
        Self::Offline
    }
}

impl PartialEq<Role> for RoleImpl {
    fn eq(&self, other: &Role) -> bool {
        matches!(
            (self, other),
            (Self::Offline, Role::Offline)
                | (Self::Client(_), Role::Client)
                | (Self::Server(_), Role::Server)
        )
    }
}

#[derive(Default)]
pub struct FrameData {
    pub role: Role,
}

#[derive(Default)]
pub struct FixedData {
    target_role: Role,
    role: RoleImpl,
}

impl FixedData {
    pub async fn swap(
        &mut self,
        frame_data: &mut FrameData,
        event_delegate: &mut SyncEventDelegate<'_>,
    ) {
        self.target_role = frame_data.role;

        if let RoleImpl::Client(client) = &mut self.role {
            client.swap(event_delegate);
        }
    }

    pub async fn update(&mut self, update_buffer: NetworkUpdateBufferRef<'_>) {
        if self.role != self.target_role {
            log::info!("setting network role to {:?}", self.target_role);

            self.role = match self.target_role {
                Role::Offline => RoleImpl::Offline,
                Role::Client => RoleImpl::Client(Client::default().into()),
                Role::Server => RoleImpl::Server(Server::default().into()),
            };
        }

        match &mut self.role {
            RoleImpl::Offline => {}
            RoleImpl::Client(client) => {
                client.update(update_buffer).await;
            }
            RoleImpl::Server(server) => {
                server.update(update_buffer).await;
            }
        }
    }
}
