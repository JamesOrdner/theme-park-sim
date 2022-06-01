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
use laminar::{Packet, Socket, SocketEvent};
use update_buffer::UpdateBufferRef;

use crate::{
    packet::{Connect, Heartbeat, Location, NetworkId},
    POLL_INTERVAL, SERVER_ADDR,
};

pub struct Server {
    socket_thread_join: Arc<AtomicBool>,
    sender: Sender<Packet>,
    receiver: Receiver<SocketEvent>,
    connected_clients: Vec<SocketAddr>,
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
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.socket_thread_join.store(true, Ordering::Relaxed);
    }
}

impl Server {
    pub async fn update(&mut self, update_buffer: UpdateBufferRef<'_>) {
        // recv

        while let Ok(msg) = self.receiver.try_recv() {
            match &msg {
                SocketEvent::Packet(packet) => self.recv(packet),
                SocketEvent::Connect(addr) => self.connect(addr),
                SocketEvent::Timeout(_) => log::info!("timeout"),
                SocketEvent::Disconnect(addr) => self.disconnect(addr),
            }
        }

        if !self.connected_clients.is_empty() {
            self.broadcast_all(&Heartbeat.serialize());

            self.update_locations(update_buffer);
        }
    }

    fn connect(&mut self, addr: &SocketAddr) {
        log::info!("connected client {addr}");

        self.connected_clients.push(*addr);
    }

    fn disconnect(&mut self, addr: &SocketAddr) {
        log::info!("disconnected client {addr}");

        self.connected_clients.retain(|client| client != addr);
    }

    fn recv(&mut self, packet: &Packet) {
        if !self.connected_clients.contains(&packet.addr()) {
            // send handshake packet
            self.sender
                .send(Packet::reliable_unordered(
                    packet.addr(),
                    Connect.serialize().to_vec(),
                ))
                .unwrap();
        }
    }

    fn update_locations(&mut self, update_buffer: UpdateBufferRef) {
        for (entity_id, location) in update_buffer.locations() {
            let location = Location {
                network_id: NetworkId(entity_id.get() as u16),
                location: *location,
            };

            self.broadcast_all(&location.serialize());
        }
    }

    fn broadcast_all(&self, data: &[u8]) {
        for client in &self.connected_clients {
            self.sender
                .send(Packet::unreliable_sequenced(
                    *client,
                    data.to_vec(),
                    Some(0),
                ))
                .unwrap();
        }
    }
}
