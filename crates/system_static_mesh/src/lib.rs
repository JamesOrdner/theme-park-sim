use std::{iter::zip, mem};

use event::{AsyncEventDelegate, GameEvent};
use frame_buffer::FrameBufferWriter;
use game_entity::EntityId;
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

        // update system data

        let mut data = self.shared_data.write_single().await;

        // temp until entity/spawning system is in place
        if data.locations.is_empty() {
            data.locations.push(Vec3::zeros());
        }

        for (loc, modified) in zip(&mut data.locations, &self.modified_entities) {
            *loc = *modified;
        }

        drop(data);

        // update frame buffer

        let data = self.shared_data.read_single().await;
        for location in &data.locations {
            frame_buffer.push_location(*location);
        }

        // notify other systems of changes

        self.modified_entities.clear();
    }
}

#[derive(Default)]
pub struct FixedData {
    modified_entities: Vec<Vec3>,
}

impl FixedData {
    pub async fn swap(&mut self, frame_data: &mut FrameData) {
        // swap network updates to frame update, and local changes to fixed update thread
        mem::swap(
            &mut self.modified_entities,
            &mut frame_data.modified_entities,
        );
    }

    pub async fn update(&mut self, update_buffer: UpdateBufferRef<'_>) {
        // notify other of system changes

        for modified in self.modified_entities.drain(..) {
            update_buffer.push_location(EntityId::new(1), modified);
        }

        // update system from other changes

        self.modified_entities
            .extend(update_buffer.locations().map(|location| location.data));
    }
}
