use event::{GameEvent, InputEvent, NetworkEvent, SyncEventDelegate};
use frame_buffer::{SpawnedStaticMesh, SyncFrameBufferDelegate};
use game_entity::EntityId;
use game_input::GameInputInterface;
use game_resources::ResourceManager;
use nalgebra_glm::Vec3;
use system_camera::CameraInterface;
use system_interfaces::physics::Interface as PhysicsInterface;
use system_network::{FrameData as NetworkSystem, Role};

use crate::world::World;

mod world;

pub struct GameController {
    physics: PhysicsInterface,
    resource_manager: ResourceManager,
    world: World,
    placing_object: Option<EntityId>,
}

impl GameController {
    pub fn new(physics: PhysicsInterface) -> Self {
        Self {
            physics,
            resource_manager: Default::default(),
            world: Default::default(),
            placing_object: None,
        }
    }

    pub fn update(
        &mut self,
        event_delegate: &mut SyncEventDelegate,
        frame_buffer: &mut SyncFrameBufferDelegate,
        input: GameInputInterface,
        camera: CameraInterface,
        network: &mut NetworkSystem,
    ) {
        let (mut game_event_writer, network_events) = event_delegate.network_events_mut();
        for network_event in network_events {
            match network_event {
                NetworkEvent::Spawn(entity_id) => {
                    self.world.remote_spawn(*entity_id);
                    game_event_writer.push_game_event(GameEvent::Spawn(*entity_id));
                    frame_buffer.spawn_static_mesh(SpawnedStaticMesh {
                        entity_id: *entity_id,
                        resource: self.resource_manager.resource("sphere".to_string()),
                    });
                }
                NetworkEvent::Despawn(entity_id) => {
                    game_event_writer.push_game_event(GameEvent::Despawn(*entity_id));
                    frame_buffer.despawn(*entity_id);
                }
            }
        }

        let (mut game_event_writer, input_events) = event_delegate.input_events_mut();
        for input_event in input_events {
            match input_event {
                InputEvent::Spawn if self.placing_object.is_none() => {
                    let entity_id = self.world.spawn_replicable();
                    game_event_writer.push_game_event(GameEvent::Spawn(entity_id));
                    frame_buffer.spawn_static_mesh(SpawnedStaticMesh {
                        entity_id,
                        resource: self.resource_manager.resource("sphere".to_string()),
                    });

                    self.placing_object = Some(entity_id);
                }
                InputEvent::MouseButton(true) => {
                    if let Some(entity_id) = self.placing_object.take() {
                        if let Some(location) = self.location_under_cursor(input, camera) {
                            let event = GameEvent::StaticMeshLocation(entity_id, location);
                            game_event_writer.push_game_event(event);
                            frame_buffer.push_location(entity_id, location);
                        }
                    }
                }
                InputEvent::ServerBegin => {
                    network.role = Role::Server;
                }
                InputEvent::ServerConnect => {
                    network.role = Role::Client;
                }
                InputEvent::ServerDisconnect => {
                    network.role = Role::Offline;
                }
                _ => {}
            }
        }

        // object placement

        if let Some(entity_id) = &self.placing_object {
            if let Some(hit_location) = self.location_under_cursor(input, camera) {
                let event = GameEvent::StaticMeshLocation(*entity_id, hit_location);
                event_delegate.push_game_event(event);
                frame_buffer.push_location(*entity_id, hit_location);
            }
        }
    }

    fn location_under_cursor(
        &self,
        input: GameInputInterface,
        camera: CameraInterface,
    ) -> Option<Vec3> {
        let origin = camera.location();
        let orientation = camera.deproject(&input.cursor_position_ndc());

        self.physics.raycast(origin, &orientation)
    }
}
