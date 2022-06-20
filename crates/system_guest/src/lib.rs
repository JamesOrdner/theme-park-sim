use event::{AsyncEventDelegate, GameEvent};
use frame_buffer::AsyncFrameBufferDelegate;
use game_data::system_swap_data::SystemSwapData;
use game_entity::EntityId;
use game_entity::EntityMap;
use nalgebra_glm::Vec3;
use rand::prelude::*;
use update_buffer::GuestUpdateBufferRef;

#[derive(Default)]
struct SwapData {
    guest_goals: Vec<(EntityId, Vec3)>,
}

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
        _delta_time: f32,
    ) {
        if let Some(swap_data) = self.swap_data.swapped() {
            for (entity_id, goal) in &swap_data.guest_goals {
                let guest = &mut self.guests[*entity_id];
                guest.goal = *goal;
                guest.speed = 1.4;

                frame_buffer
                    .writer()
                    .push_guest_goal(*entity_id, guest.goal, guest.speed);
            }

            swap_data.guest_goals.clear();
        }

        for game_event in event_delegate.game_events() {
            match game_event {
                GameEvent::SpawnGuest { entity_id, .. } => {
                    let guest = Guest {
                        location: Vec3::zeros(),
                        goal: Vec3::zeros(),
                        speed: 0.0,
                    };

                    self.guests.insert(*entity_id, guest);
                    frame_buffer
                        .writer()
                        .push_location(*entity_id, Vec3::zeros());
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
            let mut rng = thread_rng();

            for (entity_id, guest) in &mut self.guests {
                if (guest.location - guest.goal).norm() < 0.5 {
                    let x = rng.gen_range(-25.0..25.0);
                    let z = rng.gen_range(-25.0..25.0);
                    guest.goal = Vec3::from([x, 0.0, z]);
                    guest.speed = 1.4;

                    self.swap_data.guest_goals.push((*entity_id, guest.goal));

                    frame_buffer
                        .writer()
                        .push_guest_goal(*entity_id, guest.goal, guest.speed);
                }
            }
        } else {
            self.guests
                .values_mut()
                .filter(|guest| (guest.location - guest.goal).norm() < 0.5)
                .for_each(|guest| guest.speed = 0.0);
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

    pub async fn update(&mut self, update_buffer: GuestUpdateBufferRef<'_>) {
        // push local changes to the update buffer
        for (entity_id, goal) in &self.swap_data.guest_goals {
            update_buffer.push_goal(*entity_id, *goal);
        }

        self.swap_data.guest_goals.clear();

        // push remote changes to swap data
        self.swap_data.guest_goals.extend(update_buffer.goals());
    }
}
