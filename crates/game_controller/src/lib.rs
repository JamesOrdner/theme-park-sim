use event::{GameEvent, InputEvent, SyncEventDelegate, SystemGameEvent};
use frame_buffer::{SpawnedStaticMesh, SyncFrameBufferDelegate};
use game_entity::EntityId;
use game_input::GameInputInterface;
use game_resources::ResourceManager;
use nalgebra_glm::Vec3;
use system_camera::CameraInterface;
use system_interfaces::physics::Interface as PhysicsInterface;

use self::world::World;

mod world;

#[derive(PartialEq, Eq)]
pub enum NetworkRole {
    Offline,
    Client,
    Server,
}

pub struct GameController {
    physics: PhysicsInterface,
    resource_manager: ResourceManager,
    world: World,
    placing_object: Option<EntityId>,
    network_role: NetworkRole,
}

impl GameController {
    pub fn new(physics: PhysicsInterface) -> Self {
        Self {
            physics,
            resource_manager: Default::default(),
            world: Default::default(),
            placing_object: None,
            network_role: NetworkRole::Offline,
        }
    }

    pub fn update(
        &mut self,
        event_delegate: &mut SyncEventDelegate,
        frame_buffer: &mut SyncFrameBufferDelegate,
        input: GameInputInterface,
        camera: CameraInterface,
    ) {
        self.handle_system_game_events(event_delegate, frame_buffer);

        self.handle_input_events(event_delegate, frame_buffer, input, camera);

        // object placement

        if let Some(entity_id) = &self.placing_object {
            if let Some(hit_location) = self.location_under_cursor(input, camera) {
                let event = GameEvent::StaticMeshLocation(*entity_id, hit_location);
                event_delegate.push_game_event(event);
                frame_buffer.push_location(*entity_id, hit_location);
            }
        }
    }

    fn handle_system_game_events(
        &mut self,
        event_delegate: &mut SyncEventDelegate,
        frame_buffer: &mut SyncFrameBufferDelegate,
    ) {
        let (mut game_event_writer, events) = event_delegate.system_game_events_mut();
        for event in events {
            use SystemGameEvent::*;
            match event {
                NetworkSpawn(entity_id) => {
                    // client-only
                    self.world.remote_spawn(*entity_id);
                    game_event_writer.push_game_event(GameEvent::Spawn {
                        entity_id: *entity_id,
                        replicate: false,
                    });
                    frame_buffer.spawn_static_mesh(SpawnedStaticMesh {
                        entity_id: *entity_id,
                        resource: self.resource_manager.resource("sphere".to_string()),
                    });
                }
                NetworkSpawnGuest(entity_id) => {
                    // client-only
                    self.world.remote_spawn(*entity_id);
                    game_event_writer.push_game_event(GameEvent::SpawnGuest {
                        entity_id: *entity_id,
                        replicate: false,
                    });
                    frame_buffer.spawn_static_mesh(SpawnedStaticMesh {
                        entity_id: *entity_id,
                        resource: self.resource_manager.resource("sphere".to_string()),
                    });
                }
                NetworkDespawn(entity_id) => {
                    self.world.despawn(*entity_id);
                    game_event_writer.push_game_event(GameEvent::Despawn(*entity_id));
                    frame_buffer.despawn(*entity_id);
                }
                NetworkClientSpawn(spawn_id) => {
                    // server-only
                    let replicable_id = self.world.spawn_replicable();
                    game_event_writer.push_game_event(GameEvent::Spawn {
                        entity_id: replicable_id,
                        replicate: false,
                    });
                    frame_buffer.spawn_static_mesh(SpawnedStaticMesh {
                        entity_id: replicable_id,
                        resource: self.resource_manager.resource("sphere".to_string()),
                    });
                    game_event_writer.push_game_event(GameEvent::NetworkClientSpawnAck {
                        spawn_id: *spawn_id,
                        entity_id: replicable_id,
                    });
                }
                NetworkClientSpawnAck {
                    client_id,
                    replicable_id,
                } => {
                    // client-only
                    self.world.local_to_replicable(*client_id, *replicable_id);

                    frame_buffer.update_entity_id(*client_id, *replicable_id);

                    game_event_writer.push_game_event(GameEvent::UpdateEntityId {
                        old_id: *client_id,
                        new_id: *replicable_id,
                    });

                    if self.placing_object == Some(*client_id) {
                        self.placing_object = Some(*replicable_id);
                    }
                }
            }
        }
    }

    fn handle_input_events(
        &mut self,
        event_delegate: &mut SyncEventDelegate,
        frame_buffer: &mut SyncFrameBufferDelegate,
        input: GameInputInterface,
        camera: CameraInterface,
    ) {
        let (mut game_event_writer, input_events) = event_delegate.input_events_mut();
        for input_event in input_events {
            match input_event {
                InputEvent::Spawn if self.placing_object.is_none() => {
                    let entity_id = if self.network_role != NetworkRole::Client {
                        self.world.spawn_replicable()
                    } else {
                        self.world.spawn()
                    };

                    game_event_writer.push_game_event(GameEvent::Spawn {
                        entity_id,
                        replicate: true,
                    });

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
                    game_event_writer.push_game_event(GameEvent::NetworkRoleServer);
                    self.network_role = NetworkRole::Server;
                }
                InputEvent::ServerConnect => {
                    game_event_writer.push_game_event(GameEvent::NetworkRoleClient);
                    self.network_role = NetworkRole::Client;
                }
                InputEvent::ServerDisconnect => {
                    game_event_writer.push_game_event(GameEvent::NetworkRoleOffline);
                    self.network_role = NetworkRole::Offline;
                }
                InputEvent::SpawnGuest if self.network_role != NetworkRole::Client => {
                    let entity_id = self.world.spawn_replicable();

                    game_event_writer.push_game_event(GameEvent::SpawnGuest {
                        entity_id,
                        replicate: true,
                    });

                    frame_buffer.spawn_guest(entity_id);
                }
                _ => {}
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
