use std::sync::Arc;

use anyhow::{Error, Result};
use erupt::{vk, DeviceLoader};
use smallvec::SmallVec;

use crate::VulkanInfo;

pub struct Swapchain {
    device: Arc<DeviceLoader>,
    present_queue: vk::Queue,
    pub surface_extent: vk::Extent2D,
    pub surface_format: vk::SurfaceFormatKHR,
    swapchain: vk::SwapchainKHR,
    pub images: SmallVec<[vk::Image; 3]>,
    pub image_views: SmallVec<[vk::ImageView; 3]>,
}

impl Swapchain {
    pub fn new(vulkan: &VulkanInfo) -> Result<Self> {
        if vulkan.device.queues.graphics.family_index != vulkan.device.queues.present.family_index {
            return Err(Error::msg(
                "separate graphics and present queue families is unsupported",
            ));
        }

        // swapchain

        let surface_capabilities = unsafe {
            vulkan
                .instance
                .get_physical_device_surface_capabilities_khr(
                    vulkan.device.physical_device,
                    vulkan.instance.surface,
                )
                .result()?
        };

        let device_surface_formats = unsafe {
            vulkan
                .instance
                .get_physical_device_surface_formats_khr(
                    vulkan.device.physical_device,
                    vulkan.instance.surface,
                    None,
                )
                .result()?
        };

        let surface_extent = surface_capabilities.current_extent;

        let mut surface_format = *device_surface_formats
            .first()
            .ok_or_else(|| Error::msg("no valid swapchain surface formats"))?;

        // search for preferred image format
        for device_surface_format in &device_surface_formats {
            if device_surface_format.format == vk::Format::B8G8R8_UNORM
                && device_surface_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR_KHR
            {
                surface_format = *device_surface_format;
                break;
            }
        }

        let mut min_image_count = surface_capabilities.min_image_count + 1;
        if surface_capabilities.max_image_count != 0
            && min_image_count > surface_capabilities.max_image_count
        {
            min_image_count = surface_capabilities.max_image_count;
        }

        let create_info = vk::SwapchainCreateInfoKHRBuilder::new()
            .surface(vulkan.instance.surface)
            .min_image_count(min_image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(surface_extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .pre_transform(surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagBitsKHR::OPAQUE_KHR)
            .present_mode(vk::PresentModeKHR::FIFO_KHR)
            .clipped(true);

        let swapchain = unsafe {
            vulkan
                .device
                .create_swapchain_khr(&create_info, None)
                .result()?
        };

        // image views

        let images: SmallVec<_> = unsafe {
            vulkan
                .device
                .get_swapchain_images_khr(swapchain, None)
                .result()?
                .into_iter()
                .collect()
        };

        let image_views = images
            .iter()
            .map(|image| {
                let image_view_subresource_range = vk::ImageSubresourceRangeBuilder::new()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1);

                let image_view_create_info = vk::ImageViewCreateInfoBuilder::new()
                    .image(*image)
                    .view_type(vk::ImageViewType::_2D)
                    .format(surface_format.format)
                    .subresource_range(*image_view_subresource_range);

                unsafe {
                    vulkan
                        .device
                        .create_image_view(&image_view_create_info, None)
                        .expect("create_image_view")
                }
            })
            .collect();

        Ok(Swapchain {
            device: vulkan.device.clone_loader(),
            present_queue: vulkan.device.queues.present.queue,
            surface_extent,
            surface_format,
            swapchain,
            images,
            image_views,
        })
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            for image_view in &self.image_views {
                self.device.destroy_image_view(*image_view, None);
            }

            self.device.destroy_swapchain_khr(self.swapchain, None);
        }
    }
}

impl Swapchain {
    pub fn acquire_next_image(&self, acquire_semaphore: vk::Semaphore) -> Result<u32> {
        unsafe {
            self.device
                .acquire_next_image_khr(
                    self.swapchain,
                    u64::MAX,
                    acquire_semaphore,
                    vk::Fence::null(),
                )
                .map_err(Error::from)
        }
    }

    pub fn present(&self, wait_semaphore: vk::Semaphore, image_index: u32) -> Result<()> {
        let wait_semaphores = [wait_semaphore];
        let swapchains = [self.swapchain];
        let image_indices = [image_index];
        let present_info = vk::PresentInfoKHRBuilder::new()
            .wait_semaphores(&wait_semaphores)
            .swapchains(&swapchains)
            .image_indices(&image_indices);

        unsafe {
            self.device
                .queue_present_khr(self.present_queue, &present_info)
                .result()?;
        }

        Ok(())
    }
}
