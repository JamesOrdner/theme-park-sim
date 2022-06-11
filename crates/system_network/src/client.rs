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
use event::{NetworkEvent, SyncEventDelegate};
use laminar::{Packet, Socket, SocketEvent};
use update_buffer::NetworkUpdateBufferRef;

use crate::{
    packet::{Heartbeat, LocationRef, NetworkId, PacketRef, SpawnRef},
    POLL_INTERVAL, SERVER_ADDR,
};

pub struct Client {
    socket_thread_join: Arc<AtomicBool>,
    sender: Sender<Packet>,
    receiver: Receiver<SocketEvent>,
    server_addr: SocketAddr,
    spawned: Vec<NetworkId>,
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
            spawned: Vec::new(),
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.socket_thread_join.store(true, Ordering::Relaxed);
    }
}

impl Client {
    pub fn swap(&mut self, event_delegate: &mut SyncEventDelegate) {
        for network_id in &self.spawned {
            event_delegate.push_network_event(NetworkEvent::Spawn(network_id.entity_id()));
        }

        self.spawned.clear();
    }

    pub async fn update(&mut self, update_buffer: NetworkUpdateBufferRef<'_>) {
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

    fn recv(&mut self, packet: &Packet, update_buffer: NetworkUpdateBufferRef) {
        match PacketRef::from(packet.payload()) {
            PacketRef::Location(location) => {
                self.handle_location(location, update_buffer);
            }
            PacketRef::Spawn(spawn) => {
                self.handle_spawn(spawn);
            }
            _ => {}
        }
    }

    fn handle_location(&mut self, location: LocationRef, update_buffer: NetworkUpdateBufferRef) {
        update_buffer.push_location(location.network_id().entity_id(), location.location());
    }

    fn handle_spawn(&mut self, spawn: SpawnRef) {
        println!("spawn NetworkId({})", spawn.network_id().get(),);
        self.spawned.push(spawn.network_id());
    }
}
