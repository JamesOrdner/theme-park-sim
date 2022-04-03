use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, Result};

pub use mesh::Mesh;

mod mesh;

#[derive(Default)]
pub struct ResourceManager;

impl ResourceManager {
    pub fn resource(&mut self, name: String) -> Arc<Resource> {
        Arc::new(Resource::new(name))
    }
}

pub struct Resource {
    name: String,
}

impl Resource {
    fn new(name: String) -> Self {
        Self { name }
    }

    pub fn mesh(&self) -> Result<Mesh> {
        let path = PathBuf::from(&self.name);
        mesh::load(&path).with_context(|| format!("could not load mesh {}", self.name))
    }
}
