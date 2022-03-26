use std::mem;

use event::{EventReader, EventWriter};
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
        _event_reader: EventReader<'_>,
        _event_writer: EventWriter<'_>,
        _frame_buffer_writer: FrameBufferWriter,
        _system_interfaces: SystemInterfaces<'_>,
    ) {
        if let Some(entity) = self.modified_entities.first() {
            self.location = *entity;
        }

        println!("{:?}", self.location);
    }
}

#[derive(Default)]
pub struct FixedData {
    location: Vec3,
    modified_entities: Vec<Vec3>,
}

impl FixedData {
    pub fn swap(&mut self, frame_data: &mut FrameData) {
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
