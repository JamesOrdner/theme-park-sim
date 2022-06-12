use std::{num::NonZeroU16, ops::Deref};

use game_entity::EntityId;
use nalgebra_glm::{vec3, Vec3};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetworkId(NonZeroU16);

impl NetworkId {
    #[track_caller]
    pub fn new(val: u16) -> Self {
        #[cfg(debug_assertions)]
        return Self(NonZeroU16::new(val).expect("EntityId may not be 0"));

        #[cfg(not(debug_assertions))]
        return Self(unsafe { NonZeroU32::new_unchecked(val) });
    }

    pub fn entity_id(&self) -> EntityId {
        EntityId::new(self.0.get().into())
    }
}

impl From<EntityId> for NetworkId {
    fn from(entity_id: EntityId) -> Self {
        let id: u16 = entity_id.get().try_into().unwrap();
        // SAFETY: EntityId cannot be 0
        NetworkId(unsafe { NonZeroU16::new_unchecked(id) })
    }
}

impl From<&EntityId> for NetworkId {
    fn from(entity_id: &EntityId) -> Self {
        let id: u16 = entity_id.get().try_into().unwrap();
        // SAFETY: EntityId cannot be 0
        NetworkId(unsafe { NonZeroU16::new_unchecked(id) })
    }
}

impl Deref for NetworkId {
    type Target = NonZeroU16;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(u8)]
pub enum PacketType {
    ClientSpawn,
    ClientSpawnAck,
    Connect,
    Heartbeat,
    Location,
    Spawn,
}

pub enum PacketRef<'a> {
    ClientSpawn(ClientSpawnRef<'a>),
    ClientSpawnAck(ClientSpawnAckRef<'a>),
    Connect,
    Heartbeat,
    Location(LocationRef<'a>),
    Spawn(SpawnRef<'a>),
}

impl<'a> From<&'a [u8]> for PacketRef<'a> {
    fn from(data: &'a [u8]) -> Self {
        match data[0] {
            a if a == PacketType::ClientSpawn as u8 => {
                Self::ClientSpawn(ClientSpawnRef(data[1..].try_into().unwrap()))
            }
            a if a == PacketType::ClientSpawnAck as u8 => {
                Self::ClientSpawnAck(ClientSpawnAckRef(data[1..].try_into().unwrap()))
            }
            a if a == PacketType::Connect as u8 => Self::Connect,
            a if a == PacketType::Heartbeat as u8 => Self::Heartbeat,
            a if a == PacketType::Location as u8 => {
                Self::Location(LocationRef(data[1..].try_into().unwrap()))
            }
            a if a == PacketType::Spawn as u8 => {
                Self::Spawn(SpawnRef(data[1..].try_into().unwrap()))
            }
            _ => unreachable!(),
        }
    }
}

pub struct ClientSpawn {
    pub entity_id: EntityId,
}

impl ClientSpawn {
    pub fn serialize(&self) -> [u8; 5] {
        let mut data = [0; 5];
        data[0] = PacketType::ClientSpawn as u8;
        data[1..5].copy_from_slice(&self.entity_id.get().to_le_bytes());
        data
    }
}

pub struct ClientSpawnRef<'a>(&'a [u8; 4]);

impl ClientSpawnRef<'_> {
    pub fn entity_id(&self) -> EntityId {
        EntityId::new(u32::from_le_bytes(self.0[0..4].try_into().unwrap()))
    }
}

pub struct ClientSpawnAck {
    pub client_id: EntityId,
    pub server_id: NetworkId,
}

impl ClientSpawnAck {
    pub fn serialize(&self) -> [u8; 7] {
        let mut data = [0; 7];
        data[0] = PacketType::ClientSpawnAck as u8;
        data[1..5].copy_from_slice(&self.client_id.get().to_le_bytes());
        data[5..7].copy_from_slice(&self.server_id.get().to_le_bytes());
        data
    }
}

pub struct ClientSpawnAckRef<'a>(&'a [u8; 6]);

impl ClientSpawnAckRef<'_> {
    pub fn client_id(&self) -> EntityId {
        EntityId::new(u32::from_le_bytes(self.0[0..4].try_into().unwrap()))
    }

    pub fn server_id(&self) -> NetworkId {
        NetworkId::new(u16::from_le_bytes(self.0[4..6].try_into().unwrap()))
    }
}

pub struct Connect;

impl Connect {
    pub fn serialize(&self) -> [u8; 1] {
        let mut data = [0; 1];
        data[0] = PacketType::Connect as u8;
        data
    }
}

pub struct Heartbeat;

impl Heartbeat {
    pub fn serialize(&self) -> [u8; 1] {
        let mut data = [0; 1];
        data[0] = PacketType::Heartbeat as u8;
        data
    }
}

pub struct Location {
    pub network_id: NetworkId,
    pub location: Vec3,
}

impl Location {
    pub fn serialize(&self) -> [u8; 15] {
        let mut data = [0; 15];
        data[0] = PacketType::Location as u8;
        data[1..3].copy_from_slice(&self.network_id.get().to_le_bytes());
        data[3..7].copy_from_slice(&self.location.x.to_le_bytes());
        data[7..11].copy_from_slice(&self.location.y.to_le_bytes());
        data[11..15].copy_from_slice(&self.location.z.to_le_bytes());
        data
    }
}

pub struct LocationRef<'a>(&'a [u8; 14]);

impl LocationRef<'_> {
    pub fn network_id(&self) -> NetworkId {
        NetworkId::new(u16::from_le_bytes(self.0[0..2].try_into().unwrap()))
    }

    pub fn location(&self) -> Vec3 {
        let x = f32::from_le_bytes(self.0[2..6].try_into().unwrap());
        let y = f32::from_le_bytes(self.0[6..10].try_into().unwrap());
        let z = f32::from_le_bytes(self.0[10..14].try_into().unwrap());
        vec3(x, y, z)
    }
}

pub struct Spawn {
    pub network_id: NetworkId,
}

impl Spawn {
    pub fn serialize(&self) -> [u8; 3] {
        let mut data = [0; 3];
        data[0] = PacketType::Spawn as u8;
        data[1..3].copy_from_slice(&self.network_id.get().to_le_bytes());
        data
    }
}

pub struct SpawnRef<'a>(&'a [u8; 2]);

impl SpawnRef<'_> {
    pub fn network_id(&self) -> NetworkId {
        NetworkId::new(u16::from_le_bytes(self.0[0..2].try_into().unwrap()))
    }
}
