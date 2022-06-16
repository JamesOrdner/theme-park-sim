use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, sleep},
    time::Instant,
};

use crossbeam_channel::{Receiver, Sender};
use event::{AsyncEventDelegate, GameEvent, SystemGameEvent};
use game_data::system_swap_data::SystemSwapData;
use game_entity::EntityId;
use laminar::{Packet, Socket, SocketEvent};
use update_buffer::NetworkUpdateBufferRef;

use crate::{
    broadcast_reliable_ordered, broadcast_unreliable_sequenced,
    packet::{
        ClientSpawnAck, ClientSpawnRef, Connect, Heartbeat, Location, LocationRef, PacketRef, Spawn,
    },
    POLL_INTERVAL, SERVER_ADDR,
};

#[derive(Default)]
struct SwapData {
    server_spawned: Vec<EntityId>,
    client_spawned: Vec<u16>,
    client_spawned_acks: Vec<(u16, EntityId)>,
}

#[derive(Default)]
pub struct ServerFrameData {
    swap_data: SystemSwapData<SwapData>,
}

impl ServerFrameData {
    pub fn update(&mut self, event_delegate: &AsyncEventDelegate) {
        // push network events from last update to event_deleage
        if let Some(swap_data) = self.swap_data.swapped() {
            for spawn_id in &swap_data.client_spawned {
                event_delegate
                    .push_system_game_event(SystemGameEvent::NetworkClientSpawn(*spawn_id));
            }

            swap_data.client_spawned.clear();
        }

        // queue spawn events from event_delegate
        for game_event in event_delegate.game_events() {
            match game_event {
                GameEvent::Spawn {
                    entity_id,
                    replicate: true,
                } => {
                    self.swap_data.server_spawned.push(*entity_id);
                }
                GameEvent::NetworkClientSpawnAck {
                    spawn_id,
                    entity_id,
                } => {
                    self.swap_data
                        .client_spawned_acks
                        .push((*spawn_id, *entity_id));
                }
                GameEvent::Despawn(_) => todo!(),
                _ => {}
            }
        }
    }
}

struct ConnectedClient {
    addr: SocketAddr,
    /// entities spawned by the client which are awaiting ack, locally identified by a u16
    spawned_entities: Vec<(u16, EntityId)>,
}

impl ConnectedClient {
    fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            spawned_entities: Vec::new(),
        }
    }
}

pub struct Server {
    socket_thread_join: Arc<AtomicBool>,
    sender: Sender<Packet>,
    receiver: Receiver<SocketEvent>,
    connected_clients: Vec<ConnectedClient>,
    swap_data: SystemSwapData<SwapData>,
    spawn_id_free_list: Vec<u16>,
    next_spawn_id: u16,
}

impl Default for Server {
    fn default() -> Self {
        let mut socket = Socket::bind(SERVER_ADDR).unwrap();

        let sender = socket.get_packet_sender();
        let receiver = socket.get_event_receiver();

        let socket_thread_join = Arc::new(AtomicBool::new(false));

        let quit = socket_thread_join.clone();
        thread::spawn(move || {
            while !quit.load(Ordering::Relaxed) {
                socket.manual_poll(Instant::now());
                sleep(POLL_INTERVAL);
            }
        });

        Self {
            socket_thread_join,
            sender,
            receiver,
            connected_clients: Vec::new(),
            swap_data: Default::default(),
            spawn_id_free_list: Vec::new(),
            next_spawn_id: 0,
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.socket_thread_join.store(true, Ordering::Relaxed);
    }
}

impl Server {
    pub fn swap(&mut self, frame_data: &mut ServerFrameData) {
        self.swap_data.swap(&mut frame_data.swap_data);
    }

    pub fn update(&mut self, update_buffer: NetworkUpdateBufferRef) {
        // post-swap update

        self.update_swap();

        // read update buffer

        self.update_state(update_buffer);

        // recv

        while let Ok(msg) = self.receiver.try_recv() {
            match &msg {
                SocketEvent::Packet(packet) => self.recv(packet, update_buffer),
                SocketEvent::Connect(addr) => self.connect(addr),
                SocketEvent::Timeout(_) => log::info!("timeout"),
                SocketEvent::Disconnect(addr) => self.disconnect(addr),
            }
        }

        // heartbeat

        broadcast_reliable_ordered(
            self.connected_clients.iter().map(|client| &client.addr),
            &self.sender,
            &Heartbeat.serialize(),
        );
    }

    fn connect(&mut self, addr: &SocketAddr) {
        log::info!("connected client {addr}");

        self.connected_clients.push(ConnectedClient::new(*addr));
    }

    fn disconnect(&mut self, addr: &SocketAddr) {
        log::info!("disconnected client {addr}");

        self.connected_clients.retain(|client| client.addr != *addr);
    }

    fn update_swap(&mut self) {
        // broadcast server spawns

        for entity_id in &self.swap_data.server_spawned {
            let spawn_packet = Spawn {
                entity_id: *entity_id,
            };

            broadcast_reliable_ordered(
                self.connected_clients.iter().map(|client| &client.addr),
                &self.sender,
                &spawn_packet.serialize(),
            );
        }

        self.swap_data.server_spawned.clear();

        // broadcast client spawn acks by server

        for (spawn_id, entity_id) in &self.swap_data.client_spawned_acks {
            let mut spawning_client_addr = None;

            if let Some((i, client)) = self.connected_clients.iter_mut().find_map(|client| {
                client
                    .spawned_entities
                    .iter_mut()
                    .enumerate()
                    .find(|(_, entity)| entity.0 == *spawn_id)
                    .map(|(i, _)| i)
                    .map(|i| (i, client))
            }) {
                let client_id = client.spawned_entities.remove(i).1;

                let spawn_ack_packet = ClientSpawnAck {
                    client_id,
                    server_id: *entity_id,
                };

                broadcast_reliable_ordered(
                    &[client.addr],
                    &self.sender,
                    &spawn_ack_packet.serialize(),
                );

                spawning_client_addr = Some(client.addr);
            }

            let other_clients = self
                .connected_clients
                .iter()
                .map(|client| &client.addr)
                .filter(|addr| Some(**addr) != spawning_client_addr);

            let spawn_packet = Spawn {
                entity_id: *entity_id,
            };

            broadcast_reliable_ordered(other_clients, &self.sender, &spawn_packet.serialize());
        }

        self.swap_data.client_spawned_acks.clear();
    }

    fn update_state(&mut self, update_buffer: NetworkUpdateBufferRef) {
        update_buffer
            .locations()
            .map(|(entity_id, location)| Location {
                entity_id,
                location: location.into(),
            })
            .for_each(|packet| {
                broadcast_unreliable_sequenced(
                    self.connected_clients.iter().map(|client| &client.addr),
                    &self.sender,
                    &packet.serialize(),
                )
            });
    }

    fn recv(&mut self, packet: &Packet, update_buffer: NetworkUpdateBufferRef) {
        if !self
            .connected_clients
            .iter()
            .any(|client| client.addr == packet.addr())
        {
            // send handshake packet
            self.sender
                .send(Packet::reliable_unordered(
                    packet.addr(),
                    Connect.serialize().to_vec(),
                ))
                .unwrap();
        }

        match PacketRef::from(packet.payload()) {
            PacketRef::ClientSpawn(spawn) => {
                self.handle_client_spawn(spawn, &packet.addr());
            }
            PacketRef::Location(location) => {
                self.handle_location(location, &packet.addr(), update_buffer);
            }
            _ => {}
        }
    }

    fn handle_client_spawn(&mut self, spawn: ClientSpawnRef, addr: &SocketAddr) {
        if let Some(client) = self
            .connected_clients
            .iter_mut()
            .find(|client| client.addr == *addr)
        {
            let spawn_id = self.spawn_id_free_list.pop().unwrap_or_else(|| {
                let spawn_id = self.next_spawn_id;
                self.next_spawn_id += 1;
                spawn_id
            });

            client.spawned_entities.push((spawn_id, spawn.entity_id()));
            self.swap_data.client_spawned.push(spawn_id);
        }
    }

    fn handle_location(
        &mut self,
        location: LocationRef,
        addr: &SocketAddr,
        update_buffer: NetworkUpdateBufferRef,
    ) {
        update_buffer.push_location(location.entity_id(), location.location().into());

        let location_packet = Location {
            entity_id: location.entity_id(),
            location: location.location(),
        };

        let other_clients = self
            .connected_clients
            .iter()
            .map(|client| &client.addr)
            .filter(|client_addr| *client_addr != addr);

        broadcast_unreliable_sequenced(other_clients, &self.sender, &location_packet.serialize());
    }
}
