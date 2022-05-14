use event::{GameEvent, InputEvent, SyncEventDelegate};
use frame_buffer::{SpawnedStaticMesh, SyncFrameBufferDelegate};
use game_entity::EntityId;
use game_resources::ResourceManager;

use crate::world::World;

mod world;

#[derive(Default)]
pub struct GameController {
    resource_manager: ResourceManager,
    world: World,
}

impl GameController {
    pub fn update(
        &mut self,
        event_delegate: &mut SyncEventDelegate,
        frame_buffer: &mut SyncFrameBufferDelegate,
    ) {
        if event_delegate
            .input_events()
            .any(|event| matches!(event, InputEvent::MouseButton(true)))
        {
            if !self.world.contains(EntityId::new(1)) {
                let entity_id = self.world.spawn();
                event_delegate.push_game_event(GameEvent::Spawn(entity_id));
                frame_buffer.spawn_static_mesh(SpawnedStaticMesh {
                    entity_id,
                    resource: self.resource_manager.resource("sphere".to_string()),
                });
            } else {
                let entity_id = EntityId::new(1);

                self.world.despawn(entity_id);
                event_delegate.push_game_event(GameEvent::Despawn(entity_id));
                frame_buffer.despawn(entity_id);
            }
        }
    }
}
