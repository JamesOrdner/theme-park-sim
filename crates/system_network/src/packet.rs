use nalgebra_glm::{vec3, Vec3};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct NetworkId(pub u16);

#[repr(u8)]
pub enum PacketType {
    Connect,
    Heartbeat,
    Location,
}

pub enum PacketRef<'a> {
    Connect,
    Heartbeat,
    Location(LocationRef<'a>),
}

impl<'a> From<&'a [u8]> for PacketRef<'a> {
    fn from(data: &'a [u8]) -> Self {
        match data[0] {
            a if a == PacketType::Connect as u8 => Self::Connect,
            a if a == PacketType::Heartbeat as u8 => Self::Heartbeat,
            a if a == PacketType::Location as u8 => {
                Self::Location(LocationRef(data[1..].try_into().unwrap()))
            }
            _ => unreachable!(),
        }
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
        data[1..3].copy_from_slice(&self.network_id.0.to_le_bytes());
        data[3..7].copy_from_slice(&self.location.x.to_le_bytes());
        data[7..11].copy_from_slice(&self.location.y.to_le_bytes());
        data[11..15].copy_from_slice(&self.location.z.to_le_bytes());
        data
    }
}

pub struct LocationRef<'a>(&'a [u8; 14]);

impl LocationRef<'_> {
    pub fn network_id(&self) -> NetworkId {
        NetworkId(u16::from_le_bytes(self.0[0..2].try_into().unwrap()))
    }

    pub fn location(&self) -> Vec3 {
        let x = f32::from_le_bytes(self.0[2..6].try_into().unwrap());
        let y = f32::from_le_bytes(self.0[6..10].try_into().unwrap());
        let z = f32::from_le_bytes(self.0[10..14].try_into().unwrap());
        vec3(x, y, z)
    }
}
