use event::{GameEvent, InputEvent, SyncEventDelegate};
use frame_buffer::{SpawnedStaticMesh, SyncFrameBufferDelegate};
use game_entity::EntityId;
use game_resources::ResourceManager;

#[derive(Default)]
pub struct GameController {
    resource_manager: ResourceManager,
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
            let entity_id = EntityId::new(1);

            event_delegate.push_game_event(GameEvent::Spawn(entity_id));

            frame_buffer.spawn_static_mesh(SpawnedStaticMesh {
                entity_id,
                resource: self.resource_manager.resource("sphere".to_string()),
            });
        }
    }
}
