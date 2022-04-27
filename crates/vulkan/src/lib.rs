#![cfg(not(target_vendor = "apple"))]

use anyhow::Result;
use erupt::{vk, EntryLoader};
use frame_buffer::FrameBufferReader;
use nalgebra_glm::Mat4;
use scene::Scene;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    allocator::GpuAllocator,
    descriptor_set_layouts::{DescriptorSetLayouts, InstanceData},
    device::Device,
    frame::Frame,
    instance::Instance,
    pipeline::Pipeline,
    swapchain::Swapchain,
};

mod allocator;
mod descriptor_set_layouts;
mod device;
mod frame;
mod instance;
mod pipeline;
mod scene;
mod static_mesh;
mod swapchain;

macro_rules! cstr {
    ($s:expr) => {
        concat!($s, "\0").as_ptr().cast::<::std::os::raw::c_char>()
    };
}

pub(crate) use cstr;

/// VulkanInfo contains constant data which will not be mutated during the lifetime of an instance
pub struct VulkanInfo {
    descriptor_set_layouts: DescriptorSetLayouts,
    device: Device,
    instance: Instance,
    _entry: EntryLoader,
}

pub struct Vulkan {
    scene: Scene,
    frames: [Option<Frame>; 2],
    current_frame_index: bool,
    allocator: GpuAllocator,
    pipeline: Pipeline,
    swapchain: Swapchain,
    vulkan_info: VulkanInfo,
}

impl Vulkan {
    pub fn new(window: &Window) -> Result<Self> {
        let entry = EntryLoader::new()?;
        let instance = Instance::new(&entry, window)?;
        let device = Device::new(&instance)?;
        let descriptor_set_layouts = DescriptorSetLayouts::new(&device)?;

        let vulkan_info = VulkanInfo {
            descriptor_set_layouts,
            device,
            instance,
            _entry: entry,
        };

        let swapchain = Swapchain::new(&vulkan_info)?;

        let pipeline = Pipeline::new(&vulkan_info, &swapchain, "default")?;

        let mut allocator = GpuAllocator::new(&vulkan_info)?;

        let frames = [
            Some(Frame::new(&vulkan_info, &mut allocator)?),
            Some(Frame::new(&vulkan_info, &mut allocator)?),
        ];

        Ok(Self {
            scene: Default::default(),
            frames,
            current_frame_index: false,
            allocator,
            pipeline,
            swapchain,
            vulkan_info,
        })
    }
}

impl Drop for Vulkan {
    fn drop(&mut self) {
        unsafe {
            self.vulkan_info.device.device_wait_idle().unwrap();

            for frame in &mut self.frames {
                frame.take().unwrap().destroy(&mut self.allocator);
            }
        }
    }
}

impl Vulkan {
    pub fn window_resized(&mut self, _size: PhysicalSize<u32>) {}

    pub async fn frame(&mut self, frame_buffer: &FrameBufferReader<'_>) {
        self.update_scene(frame_buffer);

        let frame_info = self.frames[self.current_frame_index as usize]
            .as_mut()
            .unwrap()
            .begin()
            .unwrap();

        let swapchain_image_index = self
            .swapchain
            .acquire_next_image(frame_info.acquire_semaphore)
            .unwrap();

        let swapchain_image = self.swapchain.images[swapchain_image_index as usize];
        let swapchain_image_view = self.swapchain.image_views[swapchain_image_index as usize];

        // transition swapchain image to color attachment

        let image_memory_barriers = [vk::ImageMemoryBarrier2Builder::new()
            .src_stage_mask(vk::PipelineStageFlags2::TOP_OF_PIPE)
            .dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
            .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .image(swapchain_image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })];

        let dependency_info =
            vk::DependencyInfoBuilder::new().image_memory_barriers(&image_memory_barriers);

        unsafe {
            self.vulkan_info
                .device
                .cmd_pipeline_barrier2(frame_info.command_buffer, &dependency_info);
        }

        // render

        let color_attachments = [vk::RenderingAttachmentInfoBuilder::new()
            .image_view(swapchain_image_view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 0.0],
                },
            })];

        let rendering_info = vk::RenderingInfoBuilder::new()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: self.swapchain.surface_extent,
            })
            .layer_count(1)
            .color_attachments(&color_attachments);

        unsafe {
            self.vulkan_info
                .device
                .cmd_begin_rendering(frame_info.command_buffer, &rendering_info);
        }

        self.pipeline.bind(frame_info.command_buffer);

        unsafe {
            self.vulkan_info
                .device
                .cmd_end_rendering(frame_info.command_buffer);
        }

        // transition swapchain image to present layout

        let image_memory_barriers = [vk::ImageMemoryBarrier2Builder::new()
            .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
            .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(vk::PipelineStageFlags2::BOTTOM_OF_PIPE)
            .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .image(swapchain_image)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            })];

        let dependency_info =
            vk::DependencyInfoBuilder::new().image_memory_barriers(&image_memory_barriers);

        unsafe {
            self.vulkan_info
                .device
                .cmd_pipeline_barrier2(frame_info.command_buffer, &dependency_info);
        }

        let present_semaphore = self.frames[self.current_frame_index as usize]
            .as_mut()
            .unwrap()
            .end_and_submit(frame_info)
            .unwrap();

        self.swapchain
            .present(present_semaphore, swapchain_image_index)
            .unwrap();

        self.current_frame_index = !self.current_frame_index;
    }

    fn update_scene(&mut self, frame_buffer: &FrameBufferReader) {
        for spawned in frame_buffer.spawned_static_meshes() {
            self.scene.static_meshes.push(scene::StaticMesh {
                entity_id: spawned.entity_id,
            });
        }

        let frame = self.frames[self.current_frame_index as usize]
            .as_mut()
            .unwrap();

        frame.update_instance(
            0,
            &InstanceData {
                model_matrix: Mat4::identity(),
            },
        );
    }
}
