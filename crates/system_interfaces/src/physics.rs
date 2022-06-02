use game_data::SharedData;
use nalgebra_glm::{vec3, Vec3};

pub type Data = SharedData;

pub struct Interface {}

impl From<Data> for Interface {
    fn from(_data: Data) -> Self {
        Self {}
    }
}

impl Interface {
    pub fn raycast(&self, origin: &Vec3, direction: &Vec3) -> Option<Vec3> {
        let normal = vec3(0.0, 1.0, 0.0);
        let denom = normal.dot(direction);

        if denom.abs() < 1e-6 {
            return None;
        }

        let t = -(normal.dot(origin) / denom);

        if t > 1e-6 {
            Some(origin + direction * t)
        } else {
            None
        }
    }
}
