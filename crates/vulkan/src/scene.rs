use game_entity::EntityId;

#[derive(Default)]
pub struct Scene {
    pub static_meshes: Vec<StaticMesh>,
}

pub struct StaticMesh {
    pub entity_id: EntityId,
}
