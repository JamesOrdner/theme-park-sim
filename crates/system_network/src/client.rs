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
use game_entity::EntityId;
use laminar::{Packet, Socket, SocketEvent};
use update_buffer::NetworkUpdateBufferRef;

use crate::{
    packet::{Heartbeat, LocationRef, PacketRef},
    POLL_INTERVAL, SERVER_ADDR,
};

pub struct Client {
    socket_thread_join: Arc<AtomicBool>,
    sender: Sender<Packet>,
    receiver: Receiver<SocketEvent>,
    server_addr: SocketAddr,
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
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.socket_thread_join.store(true, Ordering::Relaxed);
    }
}

impl Client {
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
        if let PacketRef::Location(location) = PacketRef::from(packet.payload()) {
            self.handle_location(location, update_buffer);
        }
    }

    fn handle_location(&mut self, location: LocationRef, update_buffer: NetworkUpdateBufferRef) {
        let entity_id = EntityId::new(location.network_id().0 as u32);
        update_buffer.push_location(entity_id, location.location());
    }
}
