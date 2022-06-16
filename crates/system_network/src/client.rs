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
        ClientSpawn, ClientSpawnAckRef, Heartbeat, Location, LocationRef, PacketRef, SpawnRef,
    },
    POLL_INTERVAL, SERVER_ADDR,
};

#[derive(Default)]
struct SwapData {
    server_spawned: Vec<EntityId>,
    client_spawned: Vec<EntityId>,
    client_spawned_ack: Vec<(EntityId, EntityId)>,
}

#[derive(Default)]
pub struct ClientFrameData {
    swap_data: SystemSwapData<SwapData>,
}

impl ClientFrameData {
    pub fn update(&mut self, event_delegate: &AsyncEventDelegate) {
        // push network events from last update to event_deleage
        if let Some(swap_data) = self.swap_data.swapped() {
            for entity_id in &swap_data.server_spawned {
                event_delegate.push_system_game_event(SystemGameEvent::NetworkSpawn(*entity_id));
            }

            swap_data.server_spawned.clear();

            for (client_id, server_id) in &swap_data.client_spawned_ack {
                event_delegate.push_system_game_event(SystemGameEvent::NetworkClientSpawnAck {
                    client_id: *client_id,
                    replicable_id: *server_id,
                });
            }

            swap_data.client_spawned_ack.clear();
        }

        // queue spawn events from event_delegate
        for game_event in event_delegate.game_events() {
            match game_event {
                GameEvent::Spawn {
                    entity_id,
                    replicate: true,
                } => {
                    self.swap_data.client_spawned.push(*entity_id);
                }
                GameEvent::Despawn(_) => todo!(),
                _ => {}
            }
        }
    }
}

pub struct Client {
    socket_thread_join: Arc<AtomicBool>,
    sender: Sender<Packet>,
    receiver: Receiver<SocketEvent>,
    server_addr: SocketAddr,
    swap_data: SystemSwapData<SwapData>,
}

impl Default for Client {
    fn default() -> Self {
        let mut socket = Socket::bind_any().unwrap();

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

        let server_addr = SERVER_ADDR.parse().unwrap();

        Self {
            socket_thread_join,
            sender,
            receiver,
            server_addr,
            swap_data: Default::default(),
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.socket_thread_join.store(true, Ordering::Relaxed);
    }
}

impl Client {
    pub fn swap(&mut self, frame_data: &mut ClientFrameData) {
        self.swap_data.swap(&mut frame_data.swap_data);
    }

    pub fn update(&mut self, update_buffer: NetworkUpdateBufferRef) {
        self.update_swap();

        self.update_state(update_buffer);

        // recv

        while let Ok(msg) = self.receiver.try_recv() {
            match &msg {
                SocketEvent::Packet(packet) => self.recv(packet, update_buffer),
                SocketEvent::Connect(_) => log::info!("connect"),
                SocketEvent::Timeout(_) => log::info!("timeout"),
                SocketEvent::Disconnect(_) => log::info!("disconnect"),
            }
        }

        // send

        // heartbeat packet
        self.sender
            .send(Packet::reliable_unordered(
                self.server_addr,
                Heartbeat.serialize().to_vec(),
            ))
            .unwrap();
    }

    fn update_swap(&mut self) {
        // send spawn request
        for entity_id in &self.swap_data.client_spawned {
            let spawn_packet = ClientSpawn {
                entity_id: *entity_id,
            };

            broadcast_reliable_ordered(
                &[self.server_addr],
                &self.sender,
                &spawn_packet.serialize(),
            );
        }

        self.swap_data.client_spawned.clear();
    }

    fn update_state(&mut self, update_buffer: NetworkUpdateBufferRef) {
        update_buffer
            .locations()
            .filter(|(entity_id, _)| entity_id.get() <= u16::MAX.into()) // TEMP
            .map(|(entity_id, location)| Location {
                entity_id,
                location: location.into(),
            })
            .for_each(|packet| {
                broadcast_unreliable_sequenced(
                    &[self.server_addr],
                    &self.sender,
                    &packet.serialize(),
                )
            });
    }

    fn recv(&mut self, packet: &Packet, update_buffer: NetworkUpdateBufferRef) {
        match PacketRef::from(packet.payload()) {
            PacketRef::ClientSpawnAck(packet) => {
                self.handle_client_spawn_ack(packet);
            }
            PacketRef::Location(packet) => {
                self.handle_location(packet, update_buffer);
            }
            PacketRef::Spawn(packet) => {
                self.handle_spawn(packet);
            }
            _ => {}
        }
    }

    fn handle_client_spawn_ack(&mut self, client_spawn_ack: ClientSpawnAckRef) {
        self.swap_data
            .client_spawned_ack
            .push((client_spawn_ack.client_id(), client_spawn_ack.server_id()));
    }

    fn handle_location(&mut self, location: LocationRef, update_buffer: NetworkUpdateBufferRef) {
        update_buffer.push_location(location.entity_id(), location.location().into());
    }

    fn handle_spawn(&mut self, spawn: SpawnRef) {
        self.swap_data.server_spawned.push(spawn.entity_id());
    }
}
