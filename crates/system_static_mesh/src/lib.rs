use std::mem;

use event::{AsyncEventDelegate, GameEvent};
use frame_buffer::FrameBufferWriter;
use nalgebra_glm::Vec3;
use system_interfaces::static_mesh::Data as SharedData;
use update_buffer::UpdateBufferRef;

pub fn shared_data() -> SharedData {
    Default::default()
}

pub struct FrameData {
    shared_data: SharedData,
    modified_entities: Vec<Vec3>,
}

impl FrameData {
    pub fn new(shared_data: SharedData) -> Self {
        Self {
            shared_data,
            modified_entities: Vec::new(),
        }
    }

    pub async fn update(
        &mut self,
        event_delegate: &AsyncEventDelegate<'_>,
        frame_buffer: &FrameBufferWriter<'_>,
    ) {
        for spawn_id in event_delegate
            .game_events()
            .filter_map(|event| match event {
                GameEvent::Spawn(id) => Some(id),
                _ => None,
            })
        {
            println!("spawning {}", spawn_id.get());
        }

        if let Some(entity) = self.modified_entities.first() {
            frame_buffer.push_location(*entity);

            let mut data = self.shared_data.write_single().await;
            if let Some(location) = data.locations.first_mut() {
                *location = *entity;
            } else {
                data.locations.push(*entity);
            }
        }

        self.modified_entities.clear();
    }
}

#[derive(Default)]
pub struct FixedData {
    location: Vec3,
    modified_entities: Vec<Vec3>,
}

impl FixedData {
    pub async fn swap(&mut self, frame_data: &mut FrameData) {
        // swap network updates to frame update, and local changes to fixed update thread
        mem::swap(
            &mut self.modified_entities,
            &mut frame_data.modified_entities,
        );

        self.modified_entities.clear();
    }

    pub async fn update(&mut self, _update_buffer: UpdateBufferRef<'_>) {
        self.location.x += 1.0;
        self.location.y += 1.0;
        self.location.z += 1.0;

        if let Some(entity) = self.modified_entities.first_mut() {
            *entity = self.location;
        } else {
            self.modified_entities.push(self.location);
        }
    }
}
