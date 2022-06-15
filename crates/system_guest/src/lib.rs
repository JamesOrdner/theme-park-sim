use event::{AsyncEventDelegate, GameEvent};
use frame_buffer::AsyncFrameBufferDelegate;
use game_data::system_swap_data::SystemSwapData;
use game_entity::EntityMap;
use nalgebra_glm::Vec3;
use rand::prelude::*;

#[derive(Default)]
struct SwapData;

struct Guest {
    location: Vec3,
    goal: Vec3,
    /// m/s
    speed: f32,
}

#[derive(Default)]
pub struct FrameData {
    swap_data: SystemSwapData<SwapData>,
    guests: EntityMap<Guest>,
    client: bool,
}

impl FrameData {
    pub async fn update(
        &mut self,
        event_delegate: &AsyncEventDelegate<'_>,
        frame_buffer: &AsyncFrameBufferDelegate<'_>,
        delta_time: f32,
    ) {
        let mut rng = thread_rng();

        for game_event in event_delegate.game_events() {
            match game_event {
                GameEvent::SpawnGuest { entity_id, .. } => {
                    let guest = Guest {
                        location: Vec3::zeros(),
                        goal: Vec3::zeros(),
                        speed: 0.0,
                    };

                    self.guests.insert(*entity_id, guest);
                }
                GameEvent::Despawn(entity_id) => {
                    self.guests.remove(*entity_id);
                }
                GameEvent::NetworkRoleOffline | GameEvent::NetworkRoleServer => {
                    self.client = false;
                }
                GameEvent::NetworkRoleClient => {
                    self.client = true;
                }
                _ => {}
            }
        }

        // check if guest has reached goal
        if !self.client {
            for guest in self.guests.values_mut() {
                if (guest.location - guest.goal).norm() < 0.5 {
                    let x = rng.gen_range(-25.0..25.0);
                    let z = rng.gen_range(-25.0..25.0);
                    guest.goal = Vec3::from([x, 0.0, z]);
                    guest.speed = 1.4;
                }
            }
        } else {
            self.guests
                .values_mut()
                .filter(|guest| (guest.location - guest.goal).norm() < 0.5)
                .for_each(|guest| guest.speed = 0.0);
        }

        // update guest positions
        for (entity_id, guest) in self
            .guests
            .iter_mut()
            .filter(|(_, guest)| guest.speed != 0.0)
        {
            guest.location += (guest.goal - guest.location).normalize() * guest.speed * delta_time;
            frame_buffer
                .writer()
                .push_location(*entity_id, guest.location);
        }
    }
}

#[derive(Default)]
pub struct FixedData {
    swap_data: SystemSwapData<SwapData>,
}

impl FixedData {
    pub async fn swap(&mut self, frame_data: &mut FrameData) {
        // swap network updates to frame update, and local changes to fixed update thread
        self.swap_data.swap(&mut frame_data.swap_data);
    }

    pub async fn update(&mut self) {}
}
