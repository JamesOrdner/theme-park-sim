use std::path::Path;

use anyhow::{Error, Result};
use nalgebra_glm::Vec3;

#[derive(Default)]
pub struct Mesh {
    pub vertex_indices: Vec<u16>,
    pub vertex_positions: Vec<Vec3>,
    pub vertex_normals: Vec<Vec3>,
}

pub fn load(path: &Path) -> Result<Mesh> {
    let (document, buffers, _) = gltf::import(path)?;

    let mut mesh = Mesh::default();

    for primitive in document.meshes().flat_map(|mesh| mesh.primitives()) {
        let reader = primitive.reader(|buffer| buffers.get(buffer.index()).map(|a| a.0.as_slice()));

        mesh.vertex_indices = reader
            .read_indices()
            .ok_or_else(|| Error::msg("primitive contains no vertex indices"))?
            .into_u32()
            .map(u16::try_from)
            .collect::<Result<_, _>>()?;

        mesh.vertex_positions = reader
            .read_positions()
            .ok_or_else(|| Error::msg("primitive contains no vertex positions"))?
            .map(Vec3::from)
            .collect();

        mesh.vertex_normals = reader
            .read_normals()
            .ok_or_else(|| Error::msg("primitive contains no vertex normals"))?
            .map(Vec3::from)
            .collect();
    }

    Ok(mesh)
}
