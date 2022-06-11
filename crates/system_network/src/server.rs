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
use update_buffer::NetworkUpdateBufferRef;

use crate::{
    packet::{Connect, Heartbeat, Location, Spawn},
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
    pub async fn update(&mut self, update_buffer: NetworkUpdateBufferRef<'_>) {
        // read update buffer

        self.update_state(update_buffer);

        // recv

        while let Ok(msg) = self.receiver.try_recv() {
            match &msg {
                SocketEvent::Packet(packet) => self.recv(packet),
                SocketEvent::Connect(addr) => self.connect(addr),
                SocketEvent::Timeout(_) => log::info!("timeout"),
                SocketEvent::Disconnect(addr) => self.disconnect(addr),
            }
        }

        // heartbeat

        self.broadcast_all_reliable_ordered(&Heartbeat.serialize());
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

    fn update_state(&mut self, update_buffer: NetworkUpdateBufferRef) {
        for entity_id in update_buffer.spawned() {
            let spawn_packet = Spawn {
                network_id: entity_id.into(),
            };

            self.broadcast_all_reliable_ordered(&spawn_packet.serialize());
        }

        // update locations

        update_buffer
            .locations()
            .map(|(entity_id, location)| Location {
                network_id: entity_id.into(),
                location: *location,
            })
            .for_each(|packet| self.broadcast_all_unreliable_sequenced(&packet.serialize()));
    }

    fn broadcast_all_reliable_ordered(&self, data: &[u8]) {
        for client in &self.connected_clients {
            self.sender
                .send(Packet::reliable_ordered(*client, data.to_vec(), Some(0)))
                .unwrap();
        }
    }

    fn broadcast_all_unreliable_sequenced(&self, data: &[u8]) {
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
