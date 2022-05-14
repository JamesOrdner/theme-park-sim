use game_entity::EntityId;
use nalgebra_glm::Vec3;
use rand::Rng;
use update_buffer::UpdateBufferRef;

pub enum Role {
    Client,
    Server,
}

#[derive(Default)]
pub struct FixedData {}

impl FixedData {
    pub fn set_role(&mut self, _role: Role) {}

    pub async fn update(&mut self, update_buffer: UpdateBufferRef<'_>) {
        // take data from static mesh/other systems and update network

        // update static mesh/other systems from network

        let mut rng = rand::thread_rng();
        update_buffer.push_location(
            EntityId::new(1),
            Vec3::from([0; 3].map(|_| rng.gen::<f32>() * 0.1 - 0.05)),
        );
    }
}
