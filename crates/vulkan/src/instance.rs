use std::{ops::Deref, os::raw::c_char, ptr, sync::Arc};

use anyhow::Result;
use erupt::{
    utils::{
        surface::{create_surface, enumerate_required_extensions},
        VulkanResult,
    },
    vk, EntryLoader, InstanceLoader, InstanceLoaderBuilder,
};
use winit::window::Window;

use crate::cstr;

#[cfg(debug_assertions)]
const LAYER_NAMES: [*const c_char; 1] = [cstr!("VK_LAYER_KHRONOS_validation")];
#[cfg(not(debug_assertions))]
const LAYER_NAMES: [*const c_char; 0] = [];

pub struct Instance {
    pub surface: vk::SurfaceKHR,
    loader: Arc<InstanceLoader>,
}

const APPLICATION_INFO: vk::ApplicationInfo = vk::ApplicationInfo {
    s_type: vk::ApplicationInfo::STRUCTURE_TYPE,
    p_next: ptr::null(),
    p_application_name: ptr::null(),
    application_version: vk::make_api_version(0, 0, 0, 0),
    p_engine_name: ptr::null(),
    engine_version: vk::make_api_version(0, 0, 0, 0),
    api_version: vk::API_VERSION_1_3,
};

impl Instance {
    pub fn new(entry: &EntryLoader, window: &Window) -> Result<Instance> {
        let extension_names = required_extensions(window)?;

        let create_info = vk::InstanceCreateInfoBuilder::new()
            .application_info(&APPLICATION_INFO)
            .enabled_layer_names(&LAYER_NAMES)
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

    pub fn new_vr<F>(entry: &EntryLoader, window: &Window, create_instance: F) -> Result<Instance>
    where
        F: FnOnce(&vk::InstanceCreateInfo) -> vk::Instance,
    {
        let extension_names = required_extensions(window)?;

        let create_info = vk::InstanceCreateInfoBuilder::new()
            .application_info(&APPLICATION_INFO)
            .enabled_layer_names(&LAYER_NAMES)
            .enabled_extension_names(&extension_names);

        let create_instance = Box::new(|create_info, _: Option<&vk::AllocationCallbacks>| {
            VulkanResult::new_ok(create_instance(create_info))
        }) as Box<dyn FnOnce(_, _) -> _>;

        let instance_loader = unsafe {
            // we build the instance before returning, so lifetime of create_instance is OK
            let create_instance = std::mem::transmute(create_instance);

            InstanceLoaderBuilder::new()
                .create_instance_fn(create_instance)
                .build(entry, &create_info)?
        };
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
