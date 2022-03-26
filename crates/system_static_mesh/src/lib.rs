use std::mem;

use event::{AsyncEventDelegate, GameEvent};
use frame_buffer::FrameBufferWriter;
use nalgebra_glm::Vec3;
use system_interfaces::SystemInterfaces;
use update_buffer::UpdateBufferRef;

#[derive(Default)]
pub struct FrameData {
    location: Vec3,
    modified_entities: Vec<Vec3>,
}

impl FrameData {
    pub async fn update(
        &mut self,
        event_delegate: &AsyncEventDelegate<'_>,
        frame_buffer: &FrameBufferWriter<'_>,
        _system_interfaces: SystemInterfaces<'_>,
    ) {
        for event in event_delegate.game_events() {
            if let GameEvent::Spawn(id) = event {
                println!("spawning {id}");
            }
        }

        if let Some(entity) = self.modified_entities.first() {
            self.location = *entity;
            frame_buffer.push_location(*entity);
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
