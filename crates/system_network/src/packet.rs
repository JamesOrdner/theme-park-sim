use game_entity::EntityId;
use nalgebra_glm::Vec3;
use system_network_packet_macro::{NetworkPacket, NetworkPacketTypes};

pub struct Vec3_32 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3_32 {
    fn from_le_bytes(bytes: [u8; 12]) -> Self {
        Self {
            x: f32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            y: f32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            z: f32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        }
    }

    fn to_le_bytes(&self) -> [u8; 12] {
        let mut data = [0; 12];
        data[0..4].copy_from_slice(&self.x.to_le_bytes());
        data[4..8].copy_from_slice(&self.y.to_le_bytes());
        data[8..12].copy_from_slice(&self.z.to_le_bytes());
        data
    }
}

impl From<&Vec3> for Vec3_32 {
    fn from(vec: &Vec3) -> Self {
        Self {
            x: vec.x,
            y: vec.y,
            z: vec.z,
        }
    }
}

impl From<Vec3_32> for Vec3 {
    fn from(vec: Vec3_32) -> Self {
        Self::from([vec.x, vec.y, vec.z])
    }
}

#[repr(u8)]
#[derive(NetworkPacketTypes)]
pub enum PacketType {
    ClientSpawn,
    ClientSpawnAck,
    Connect,
    Heartbeat,
    Location,
    Spawn,
}

#[derive(NetworkPacket)]
pub struct ClientSpawn {
    pub entity_id: EntityId,
}

#[derive(NetworkPacket)]
pub struct ClientSpawnAck {
    pub client_id: EntityId,
    pub server_id: EntityId,
}

#[derive(NetworkPacket)]
pub struct Connect;

#[derive(NetworkPacket)]
pub struct Heartbeat;

#[derive(NetworkPacket)]
pub struct Location {
    pub entity_id: EntityId,
    pub location: Vec3_32,
}

#[derive(NetworkPacket)]
pub struct Spawn {
    pub entity_id: EntityId,
}
