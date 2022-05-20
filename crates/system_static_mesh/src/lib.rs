use std::mem;

use event::{AsyncEventDelegate, GameEvent};
use frame_buffer::AsyncFrameBufferDelegate;
use game_entity::EntityMap;
use nalgebra_glm::Vec3;
use system_interfaces::static_mesh::Data as SharedData;
use update_buffer::UpdateBufferRef;

pub fn shared_data() -> SharedData {
    Default::default()
}

pub struct FrameData {
    shared_data: SharedData,
    modified_entities: EntityMap<Vec3>,
}

impl FrameData {
    pub fn new(shared_data: SharedData) -> Self {
        Self {
            shared_data,
            modified_entities: EntityMap::new(),
        }
    }

    pub async fn update(
        &mut self,
        event_delegate: &AsyncEventDelegate<'_>,
        _frame_buffer: &AsyncFrameBufferDelegate<'_>,
    ) {
        // update system data

        let mut data = self.shared_data.write_single().await;

        for game_event in event_delegate.game_events() {
            match game_event {
                GameEvent::Spawn(entity_id) => {
                    data.locations.insert(*entity_id, Vec3::zeros());
                }
                GameEvent::Despawn(entity_id) => {
                    data.locations.remove(*entity_id);
                }
                GameEvent::StaticMeshLocation(entity_id, location) => {
                    data.locations[*entity_id] = *location;
                }
            }
        }

        for (modified_entity_id, modified_location) in &self.modified_entities {
            if let Some(location) = data.locations.get_mut(*modified_entity_id) {
                *location = *modified_location;
            }
        }

        drop(data);

        // update frame buffer

        // notify other systems of changes

        self.modified_entities.clear();
    }
}

#[derive(Default)]
pub struct FixedData {
    modified_entities: EntityMap<Vec3>,
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

        for (entity_id, location) in &self.modified_entities {
            update_buffer.push_location(*entity_id, *location);
        }

        self.modified_entities.clear();

        // update system from other changes

        self.modified_entities.extend(
            update_buffer
                .locations()
                .map(|location| (location.entity_id, location.data)),
        );
    }
}
