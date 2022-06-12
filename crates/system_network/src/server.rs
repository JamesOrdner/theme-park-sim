use std::{
    mem,
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
pub struct ServerFrameData {
    spawned: Vec<EntityId>,
    client_spawned: Vec<u16>,
    client_spawned_acks: Vec<(u16, EntityId)>,
    swapped: bool,
}

impl ServerFrameData {
    pub fn update(&mut self, event_delegate: &AsyncEventDelegate) {
        // push network events from last update to event_deleage
        if self.swapped {
            self.swapped = false;

            for spawn_id in &self.client_spawned {
                event_delegate
                    .push_system_game_event(SystemGameEvent::NetworkClientSpawn(*spawn_id));
            }

            self.client_spawned.clear();
        }

        // queue spawn events from event_delegate
        for game_event in event_delegate.game_events() {
            match game_event {
                GameEvent::Spawn {
                    entity_id,
                    replicable,
                } if *replicable => {
                    self.spawned.push(*entity_id);
                }
                GameEvent::Despawn(_) => todo!(),
                GameEvent::NetworkClientSpawnAck {
                    spawn_id,
                    entity_id,
                } => {
                    self.client_spawned_acks.push((*spawn_id, *entity_id));
                }
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
    spawned: Vec<EntityId>,
    client_spawned: Vec<u16>,
    client_spawned_acks: Vec<(u16, EntityId)>,
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
            spawned: Vec::new(),
            client_spawned: Vec::new(),
            client_spawned_acks: Vec::new(),
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
        mem::swap(&mut self.spawned, &mut frame_data.spawned);
        mem::swap(&mut self.client_spawned, &mut frame_data.client_spawned);
        mem::swap(
            &mut self.client_spawned_acks,
            &mut frame_data.client_spawned_acks,
        );

        frame_data.swapped = true;
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
        for entity_id in &self.spawned {
            let spawn_packet = Spawn {
                network_id: entity_id.into(),
            };

            broadcast_reliable_ordered(
                self.connected_clients.iter().map(|client| &client.addr),
                &self.sender,
                &spawn_packet.serialize(),
            );
        }

        self.spawned.clear();

        for (spawn_id, entity_id) in &self.client_spawned_acks {
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
                    server_id: entity_id.into(),
                };

                broadcast_reliable_ordered(
                    &[client.addr],
                    &self.sender,
                    &spawn_ack_packet.serialize(),
                );
            }

            // let spawn_packet = Spawn {
            //     network_id: entity_id.into(),
            // };

            // broadcast_reliable_ordered(
            //     self.connected_clients.iter().map(|client| &client.addr),
            //     &self.sender,
            //     &spawn_packet.serialize(),
            // );
        }

        self.client_spawned_acks.clear();
    }

    fn update_state(&mut self, update_buffer: NetworkUpdateBufferRef) {
        update_buffer
            .locations()
            .map(|(entity_id, location)| Location {
                network_id: entity_id.into(),
                location: *location,
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
                self.handle_location(location, update_buffer);
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
            self.client_spawned.push(spawn_id);
        }
    }

    fn handle_location(&mut self, location: LocationRef, update_buffer: NetworkUpdateBufferRef) {
        update_buffer.push_location(location.network_id().entity_id(), location.location());
    }
}
