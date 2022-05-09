pub mod navigation;
pub mod static_mesh;

pub struct SystemData {
    pub navigation: navigation::Data,
    pub static_mesh: static_mesh::Data,
}
