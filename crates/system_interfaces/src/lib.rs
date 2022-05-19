pub mod navigation;
pub mod physics;
pub mod static_mesh;

pub struct SystemData {
    pub navigation: navigation::Data,
    pub physics: physics::Data,
    pub static_mesh: static_mesh::Data,
}
