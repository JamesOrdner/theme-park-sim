use nalgebra_glm::{Vec2, Vec3};

#[repr(C)]
pub struct Vertex {
    pub location: Vec3,
    pub normal: Vec3,
    pub tex_coord: Vec2,
}
