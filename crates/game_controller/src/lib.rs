use event::{GameEvent, InputEvent, SyncEventDelegate};
use frame_buffer::{SpawnedStaticMesh, SyncFrameBufferDelegate};
use game_entity::EntityId;
use game_input::GameInputInterface;
use game_resources::ResourceManager;
use system_camera::CameraInterface;
use system_interfaces::physics::Interface as PhysicsInterface;

use crate::world::World;

mod world;

pub struct GameController {
    physics: PhysicsInterface,
    resource_manager: ResourceManager,
    world: World,
}

impl GameController {
    pub fn new(physics: PhysicsInterface) -> Self {
        Self {
            physics,
            resource_manager: Default::default(),
            world: Default::default(),
        }
    }

    pub fn update(
        &mut self,
        event_delegate: &mut SyncEventDelegate,
        frame_buffer: &mut SyncFrameBufferDelegate,
        input: GameInputInterface,
        camera: CameraInterface,
    ) {
        // temp
        let entity_id = EntityId::new(1);

        // object spawning

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
                self.world.despawn(entity_id);
                event_delegate.push_game_event(GameEvent::Despawn(entity_id));
                frame_buffer.despawn(entity_id);
            }
        }

        // object placement

        if self.world.contains(entity_id) {
            let origin = camera.location();
            let orientation = camera.deproject(&input.cursor_position_ndc());

            if let Some(hit_location) = self.physics.raycast(origin, &orientation) {
                let event = GameEvent::StaticMeshLocation(entity_id, hit_location);
                event_delegate.push_game_event(event);
                frame_buffer.push_location(entity_id, hit_location);
            }
        }
    }
}
