use std::sync::Arc;

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

    pub fn render_mesh(&self) {
        println!("loading render mesh for: {}", self.name);
    }
}
