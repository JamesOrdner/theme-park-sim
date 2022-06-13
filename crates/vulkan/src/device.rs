use std::{ffi::CStr, ops::Deref, os::raw::c_char, sync::Arc};

use anyhow::{Error, Result};
use erupt::{vk, DeviceLoader, DeviceLoaderBuilder, ExtendableFrom};
use smallvec::SmallVec;

use crate::instance::Instance;

pub struct Device {
    loader: Arc<DeviceLoader>,
    pub physical_device: vk::PhysicalDevice,
    pub queues: Queues,
}

impl Device {
    pub fn clone_loader(&self) -> Arc<DeviceLoader> {
        self.loader.clone()
    }
}

impl Device {
    pub fn new(instance: &Instance) -> Result<Device> {
        let required_device_extensions = [vk::KHR_SWAPCHAIN_EXTENSION_NAME];

        // select physical device

        let (physical_device, queue_families_info) = unsafe {
            select_physical_device(instance, &required_device_extensions)?
                .ok_or_else(|| Error::msg("no suitable physical device found"))?
        };

        // create logical device

        let queue_priorities = [0.0];
        let queue_create_infos: SmallVec<[_; 3]> = queue_families_info
            .unique_family_indices()
            .into_iter()
            .map(|queue_family_index| {
                vk::DeviceQueueCreateInfoBuilder::new()
                    .queue_family_index(queue_family_index)
                    .queue_priorities(&queue_priorities)
            })
            .collect();

        let create_info = vk::DeviceCreateInfoBuilder::new()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&required_device_extensions);

        let device_loader = unsafe { DeviceLoader::new(instance, physical_device, &create_info)? };
        let device_loader = Arc::new(device_loader);

        // retrieve queue handles

        let graphics_queue = unsafe {
            device_loader.get_device_queue(queue_families_info.graphics_family_index, 0)
        };
        let present_queue = unsafe {
            device_loader.get_device_queue(queue_families_info.present_family_index, 0)
        };
        let transfer_queue = unsafe {
            device_loader.get_device_queue(queue_families_info.transfer_family_index, 0)
        };

        let queues = Queues {
            graphics: Queue {
                queue: graphics_queue,
                family_index: queue_families_info.graphics_family_index,
            },
            present: Queue {
                queue: present_queue,
                family_index: queue_families_info.present_family_index,
            },
            transfer: Queue {
                queue: transfer_queue,
                family_index: queue_families_info.transfer_family_index,
            },
        };

        Ok(Device {
            loader: device_loader,
            physical_device,
            queues,
        })
    }

    pub fn new_vr<F>(
        instance: &Instance,
        physical_device: vk::PhysicalDevice,
        create_device: F,
    ) -> Result<Self>
    where
        F: FnOnce(&vk::DeviceCreateInfo) -> vk::Device,
    {
        let required_device_extensions = [vk::KHR_SWAPCHAIN_EXTENSION_NAME];

        // create logical device

        let queue_families_info =
            physical_device_queue_families_info(instance, physical_device)?
                .ok_or_else(|| Error::msg("no suitable physical device found"))?;
        let queue_priorities = [0.0];
        let queue_create_infos: SmallVec<[_; 3]> = queue_families_info
            .unique_family_indices()
            .into_iter()
            .map(|queue_family_index| {
                vk::DeviceQueueCreateInfoBuilder::new()
                    .queue_family_index(queue_family_index)
                    .queue_priorities(&queue_priorities)
            })
            .collect();

        let mut multiview_feature =
            vk::PhysicalDeviceMultiviewFeaturesBuilder::new().multiview(true);

        let create_info = vk::DeviceCreateInfoBuilder::new()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&required_device_extensions)
            .extend_from(&mut multiview_feature);

        let device = create_device(&create_info);

        let device_loader = unsafe {
            DeviceLoaderBuilder::new().build_with_existing_device(instance, device, &create_info)?
        };
        let device_loader = Arc::new(device_loader);

        // retrieve queue handles

        let graphics_queue =
            unsafe { device_loader.get_device_queue(queue_families_info.graphics_family_index, 0) };
        let present_queue =
            unsafe { device_loader.get_device_queue(queue_families_info.present_family_index, 0) };
        let transfer_queue =
            unsafe { device_loader.get_device_queue(queue_families_info.transfer_family_index, 0) };

        let queues = Queues {
            graphics: Queue {
                queue: graphics_queue,
                family_index: queue_families_info.graphics_family_index,
            },
            present: Queue {
                queue: present_queue,
                family_index: queue_families_info.present_family_index,
            },
            transfer: Queue {
                queue: transfer_queue,
                family_index: queue_families_info.transfer_family_index,
            },
        };

        Ok(Device {
            loader: device_loader,
            physical_device,
            queues,
        })
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.loader.destroy_device(None);
        }
    }
}

impl Deref for Device {
    type Target = DeviceLoader;

    fn deref(&self) -> &Self::Target {
        &*self.loader
    }
}

pub struct Queue {
    pub queue: vk::Queue,
    pub family_index: u32,
}

pub struct Queues {
    pub graphics: Queue,
    pub present: Queue,
    pub transfer: Queue,
}

#[derive(Default)]
struct QueueFamiliesInfo {
    graphics_family_index: u32,
    present_family_index: u32,
    transfer_family_index: u32,
}

impl QueueFamiliesInfo {
    fn unique_family_indices(&self) -> SmallVec<[u32; 3]> {
        let mut unique_family_indices = SmallVec::new();
        unique_family_indices.push(self.graphics_family_index);

        if !unique_family_indices.contains(&self.present_family_index) {
            unique_family_indices.push(self.present_family_index);
        }

        if !unique_family_indices.contains(&self.transfer_family_index) {
            unique_family_indices.push(self.transfer_family_index);
        }

        unique_family_indices
    }
}

unsafe fn select_physical_device(
    instance: &Instance,
    required_device_extensions: &[*const c_char],
) -> Result<Option<(vk::PhysicalDevice, QueueFamiliesInfo)>> {
    let mut selected_device_info = None;

    for physical_device in instance.enumerate_physical_devices(None).result()? {
        let device_extensions = instance
            .enumerate_device_extension_properties(physical_device, None, None)
            .result()?;

        if required_device_extensions
            .iter()
            .map(|ptr| CStr::from_ptr(*ptr))
            .any(|req_name| {
                !device_extensions
                    .iter()
                    .map(|device_ext| CStr::from_ptr(device_ext.extension_name.as_ptr()))
                    .any(|device_ext_name| device_ext_name == req_name)
            })
        {
            continue;
        }

        let queue_families_info =
            match physical_device_queue_families_info(instance, physical_device)? {
                Some(info) => info,
                None => continue,
            };

        if !instance
            .get_physical_device_surface_support_khr(
                physical_device,
                queue_families_info.present_family_index,
                instance.surface,
            )
            .result()?
        {
            continue;
        }

        let surface_formats = instance
            .get_physical_device_surface_formats_khr(physical_device, instance.surface, None)
            .result()?;

        let surface_present_modes = instance
            .get_physical_device_surface_present_modes_khr(physical_device, instance.surface, None)
            .result()?;

        if surface_formats.is_empty() || surface_present_modes.is_empty() {
            continue;
        }

        if selected_device_info.is_none() {
            selected_device_info = Some((physical_device, queue_families_info));
        } else {
            // prefer discrete device
            let device_properties = instance.get_physical_device_properties(physical_device);
            if device_properties.device_type == vk::PhysicalDeviceType::DISCRETE_GPU {
                selected_device_info = Some((physical_device, queue_families_info));
            }
        }
    }

    Ok(selected_device_info)
}

fn physical_device_queue_families_info(
    instance: &Instance,
    physical_device: vk::PhysicalDevice,
) -> Result<Option<QueueFamiliesInfo>> {
    let physical_device_queue_family_properties =
        unsafe { instance.get_physical_device_queue_family_properties(physical_device, None) };

    let mut graphics_family_index = None;
    let mut present_family_index = None;
    let mut transfer_family_index = None;

    for (i, queue_family_properties) in physical_device_queue_family_properties.iter().enumerate() {
        let i = i as u32;

        let has_graphics_support = queue_family_properties
            .queue_flags
            .contains(vk::QueueFlags::GRAPHICS);
        let has_transfer_support = queue_family_properties
            .queue_flags
            .contains(vk::QueueFlags::TRANSFER);
        let has_compute_support = queue_family_properties
            .queue_flags
            .contains(vk::QueueFlags::COMPUTE);
        let has_present_support = unsafe {
            instance
                .get_physical_device_surface_support_khr(physical_device, i, instance.surface)
                .result()?
        };

        if has_graphics_support && graphics_family_index.is_none() {
            graphics_family_index = Some(i);
        }

        if has_present_support {
            if has_graphics_support {
                // prefer graphics and present queue families to be the same
                graphics_family_index = Some(i);
                present_family_index = Some(i);
            } else if present_family_index == None {
                present_family_index = Some(i);
            }
        }

        if has_transfer_support
            && (transfer_family_index.is_none()
                || (graphics_family_index != Some(i) && !has_compute_support))
        {
            transfer_family_index = Some(i);
        }
    }

    if let (Some(graphics_family_index), Some(present_family_index), Some(transfer_family_index)) = (
        graphics_family_index,
        present_family_index,
        transfer_family_index,
    ) {
        Ok(Some(QueueFamiliesInfo {
            graphics_family_index,
            present_family_index,
            transfer_family_index,
        }))
    } else {
        Ok(None)
    }
}
