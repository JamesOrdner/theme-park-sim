use event::{AsyncEventDelegate, GameEvent};
use frame_buffer::AsyncFrameBufferDelegate;
use game_data::system_swap_data::SystemSwapData;
use game_entity::EntityMap;
use nalgebra_glm::Vec3;
use system_interfaces::static_mesh::Data as SharedData;
use update_buffer::StaticMeshUpdateBufferRef;

pub fn shared_data() -> SharedData {
    Default::default()
}

#[derive(Default)]
struct SwapData {
    modified_entities: EntityMap<Vec3>,
}

pub struct FrameData {
    shared_data: SharedData,
    swap_data: SystemSwapData<SwapData>,
}

impl FrameData {
    pub fn new(shared_data: SharedData) -> Self {
        Self {
            shared_data,
            swap_data: Default::default(),
        }
    }

    pub async fn update(
        &mut self,
        event_delegate: &AsyncEventDelegate<'_>,
        frame_buffer: &AsyncFrameBufferDelegate<'_>,
    ) {
        // update system data

        let mut data = self.shared_data.write_single().await;

        let frame_buffer_writer = frame_buffer.writer();

        if let Some(swap_data) = self.swap_data.swapped() {
            for (entity_id, modified_location) in &swap_data.modified_entities {
                if let Some(location) = data.locations.get_mut(*entity_id) {
                    *location = *modified_location;
                    frame_buffer_writer.push_location(*entity_id, *modified_location);
                }
            }

            swap_data.modified_entities.clear();
        }

        for game_event in event_delegate.game_events() {
            match game_event {
                GameEvent::Spawn { entity_id, .. } => {
                    data.locations.insert(*entity_id, Vec3::zeros());
                }
                GameEvent::Despawn(entity_id) => {
                    data.locations.remove(*entity_id);
                }
                GameEvent::UpdateEntityId { old_id, new_id } => {
                    let location = data.locations.remove(*old_id);
                    data.locations.insert(*new_id, location);
                    frame_buffer_writer.push_update_entity_id(*old_id, *new_id);
                }
                GameEvent::StaticMeshLocation(entity_id, location) => {
                    data.locations[*entity_id] = *location;
                    self.swap_data
                        .modified_entities
                        .insert(*entity_id, *location);
                }
                _ => {}
            }
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

    pub async fn update(&mut self, update_buffer: StaticMeshUpdateBufferRef<'_>) {
        // notify other of system changes

        for (entity_id, location) in &self.swap_data.modified_entities {
            update_buffer.push_location(*entity_id, *location);
        }

        self.swap_data.modified_entities.clear();

        // update system from other changes

        self.swap_data.modified_entities.extend(
            update_buffer
                .locations()
                .map(|(entity_id, location)| (entity_id, *location)),
        );
    }
}
