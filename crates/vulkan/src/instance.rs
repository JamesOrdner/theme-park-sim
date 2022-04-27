use std::{ops::Deref, os::raw::c_char, sync::Arc};

use anyhow::Result;
use erupt::{
    utils::surface::{create_surface, enumerate_required_extensions},
    vk, EntryLoader, InstanceLoader,
};
use winit::window::Window;

use crate::cstr;

pub struct Instance {
    pub surface: vk::SurfaceKHR,
    loader: Arc<InstanceLoader>,
}

impl Instance {
    pub fn new(entry: &EntryLoader, window: &Window) -> Result<Instance> {
        let application_info = vk::ApplicationInfoBuilder::new()
            .application_version(vk::make_api_version(0, 0, 0, 0))
            .api_version(vk::API_VERSION_1_3);

        #[cfg(debug_assertions)]
        let layer_names: [*const c_char; 1] = [cstr!("VK_LAYER_KHRONOS_validation")];
        #[cfg(not(debug_assertions))]
        let layer_names: [*const c_char; 0] = [];

        let extension_names = required_extensions(window)?;

        let create_info = vk::InstanceCreateInfoBuilder::new()
            .application_info(&application_info)
            .enabled_layer_names(&layer_names)
            .enabled_extension_names(&extension_names);

        let instance_loader = unsafe { InstanceLoader::new(entry, &create_info)? };
        let instance_loader = Arc::new(instance_loader);

        let surface = unsafe { create_surface(&instance_loader, window, None).result()? };

        let instance = Instance {
            surface,
            loader: instance_loader,
        };

        Ok(instance)
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            self.loader.destroy_surface_khr(self.surface, None);
            self.loader.destroy_instance(None);
        }
    }
}

impl Deref for Instance {
    type Target = InstanceLoader;

    fn deref(&self) -> &Self::Target {
        &*self.loader
    }
}

fn required_extensions(window: &Window) -> Result<[*const c_char; 3]> {
    let extensions = enumerate_required_extensions(window).result()?;
    assert_eq!(extensions.len(), 2);
    Ok([
        extensions[0],
        extensions[1],
        vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES_2_EXTENSION_NAME,
    ])
}
