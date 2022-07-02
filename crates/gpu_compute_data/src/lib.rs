use game_entity::EntityId;
use nalgebra_glm::Vec3;

pub trait GuestGpuComputeData: Sync {
    fn location(&self, entity_id: EntityId) -> Vec3;
}
