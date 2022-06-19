#![cfg(not(target_vendor = "apple"))]

use std::{mem, slice};

use anyhow::Result;
use erupt::{vk, EntryLoader};
use frame_buffer::FrameBufferReader;
use nalgebra_glm::{look_at_lh, perspective_lh_zo, Mat4};
use pipeline::SceneData;
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
    transfer::Transfer,
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
mod transfer;

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
    transfer: Transfer,
    frames: [Frame; 2],
    current_frame_index: bool,
    allocator: GpuAllocator,
    pipeline: Pipeline,
    swapchain: Swapchain,
    vulkan_info: VulkanInfo,
    aspect: f32,
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

        let transfer = Transfer::new(&vulkan_info)?;

        let frames = [
            Frame::new(&vulkan_info, &mut allocator)?,
            Frame::new(&vulkan_info, &mut allocator)?,
        ];

        let size = window.inner_size();
        let aspect = size.width as f32 / size.height as f32;

        let scene = Scene::new(&mut allocator);

        Ok(Self {
            scene,
            frames,
            transfer,
            current_frame_index: false,
            allocator,
            pipeline,
            swapchain,
            vulkan_info,
            aspect,
        })
    }

    pub fn destroy(mut self) {
        unsafe {
            self.vulkan_info.device.device_wait_idle().unwrap();

            self.scene.destroy(&mut self.allocator);

            for frame in self.frames {
                frame.destroy(&mut self.allocator);
            }
        }
    }
}

impl Vulkan {
    pub fn window_resized(&mut self, size: PhysicalSize<u32>) {
        self.aspect = size.width as f32 / size.height as f32;
    }

    pub async fn frame(&mut self, frame_buffer: &FrameBufferReader<'_>) {
        self.update_scene(frame_buffer);

        let frame_info = self.frames[self.current_frame_index as usize]
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

        // render static mesh instances

        self.pipeline.bind(frame_info.command_buffer);

        let scene_data = {
            let camera_info = frame_buffer.camera_info();

            let mut proj_matrix = perspective_lh_zo(
                self.aspect,
                camera_info.fov,
                camera_info.near_plane,
                camera_info.far_plane,
            );
            proj_matrix[5] *= -1.0;

            let view_matrix =
                look_at_lh(&camera_info.location, &camera_info.focus, &camera_info.up);

            SceneData {
                proj_matrix,
                view_matrix,
            }
        };

        unsafe {
            self.vulkan_info.device.cmd_push_constants(
                frame_info.command_buffer,
                self.pipeline.layout(),
                vk::ShaderStageFlags::VERTEX,
                0,
                mem::size_of::<SceneData>() as u32,
                &scene_data as *const _ as *const _,
            )
        }

        for (i, static_mesh) in self.scene.static_meshes.values().enumerate() {
            frame_info.bind_instance_descriptor_set(
                &self.vulkan_info.device,
                i,
                self.pipeline.layout(),
            );

            unsafe {
                self.vulkan_info.device.cmd_bind_index_buffer(
                    frame_info.command_buffer,
                    static_mesh.vertex_buffer.buffer,
                    0,
                    vk::IndexType::UINT16,
                );

                self.vulkan_info.device.cmd_bind_vertex_buffers(
                    frame_info.command_buffer,
                    0,
                    &[static_mesh.vertex_buffer.buffer],
                    &[static_mesh.vertex_offset],
                );

                self.vulkan_info
                    .device
                    .cmd_draw_indexed(frame_info.command_buffer, 3, 1, 0, 0, 0);
            }
        }

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
            .end_and_submit(frame_info)
            .unwrap();

        self.swapchain
            .present(present_semaphore, swapchain_image_index)
            .unwrap();

        self.current_frame_index = !self.current_frame_index;
    }

    fn update_scene(&mut self, frame_buffer: &FrameBufferReader<'_>) {
        // despawn

        for buffer in self.scene.delete_queue.drain(..) {
            self.allocator.dealloc(buffer);
        }

        for entity_id in frame_buffer.despawned() {
            let static_mesh = self.scene.static_meshes.remove(*entity_id);
            self.scene.delete_queue.push(static_mesh.vertex_buffer);
        }

        for (old_id, new_id) in frame_buffer.updated_entity_ids() {
            let static_mesh = self.scene.static_meshes.remove(*old_id);
            self.scene.static_meshes.insert(*new_id, static_mesh);
        }

        // spawn

        self.transfer.begin_transfers(&mut self.allocator).unwrap();

        for entity_id in frame_buffer
            .spawned_static_meshes()
            .map(|static_mesh| &static_mesh.entity_id)
            .chain(frame_buffer.spawned_guests())
        {
            const INDICES: [u16; 3] = [0, 1, 2];
            const VERTEX_DATA: [f32; 9] = [0.0, 0.0, 1.0, -1.0, 0.0, -1.0, 1.0, 0.0, -1.0];

            const VERTEX_OFFSET: usize = 8;
            const SIZE: usize = VERTEX_OFFSET + mem::size_of::<[f32; 9]>();

            let mut data = [0; SIZE];

            unsafe {
                let data = data.as_mut_ptr();
                let indices = slice::from_raw_parts_mut(data as *mut u16, INDICES.len());
                indices.copy_from_slice(&INDICES);

                let data = data.add(VERTEX_OFFSET);
                let vertices = slice::from_raw_parts_mut(data as *mut f32, VERTEX_DATA.len());
                vertices.copy_from_slice(&VERTEX_DATA);
            }

            let vertex_buffer = self.transfer.transfer_buffer(
                &data,
                vk::BufferUsageFlags::VERTEX_BUFFER | vk::BufferUsageFlags::INDEX_BUFFER,
                &mut self.allocator,
            );

            // create vertex buffer and transfer

            self.scene.static_meshes.insert(
                *entity_id,
                scene::StaticMesh {
                    vertex_buffer,
                    vertex_offset: VERTEX_OFFSET as vk::DeviceSize,
                    transform: Mat4::identity(),
                },
            );
        }

        self.transfer.submit_transfers().unwrap();

        // update instances

        for (entity_id, location) in frame_buffer.locations() {
            let transform = nalgebra_glm::translate(&Mat4::identity(), location);
            self.scene.static_meshes[entity_id].transform = transform;
        }

        let frame = &mut self.frames[self.current_frame_index as usize];

        for (i, static_mesh) in self.scene.static_meshes.values().enumerate() {
            frame.update_instance(
                i,
                &InstanceData {
                    model_matrix: static_mesh.transform,
                },
            );
        }

        unsafe {
            self.vulkan_info.device.device_wait_idle().unwrap();
        }
    }
}
