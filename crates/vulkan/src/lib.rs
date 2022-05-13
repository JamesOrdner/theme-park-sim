#![cfg(not(target_vendor = "apple"))]

use std::{mem, slice};

use anyhow::Result;
use erupt::{vk, EntryLoader};
use frame_buffer::FrameBufferReader;
use futures::pin_mut;
use nalgebra_glm::{look_at_lh, perspective_lh_zo, Mat4};
use pipeline::SceneData;
use render_pass::RenderPass;
use scene::Scene;
use task_executor::task::parallel;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    allocator::GpuAllocator,
    descriptor_set_layouts::{DescriptorSetLayouts, InstanceData},
    device::Device,
    frame::Frame,
    instance::Instance,
    pipeline::Pipeline,
    swapchain::{Swapchain, VrSwapchain},
    transfer::Transfer,
};

pub use crate::swapchain::VrSwapchainCreateInfo;

mod allocator;
mod descriptor_set_layouts;
mod device;
mod frame;
mod instance;
mod pipeline;
mod render_pass;
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
    frames: [Option<Frame>; 2],
    current_frame_index: bool,
    allocator: GpuAllocator,
    pipeline: Pipeline,
    render_pass: RenderPass,
    swapchain: Swapchain,
    vr_swapchain: Option<VrSwapchain>,
    vulkan_info: VulkanInfo,
    aspect: f32,
}

pub struct XrCreateInfo {
    pub instance: vk::Instance,
    pub physical_device: vk::PhysicalDevice,
    pub device: vk::Device,
    pub queue_family_index: u32,
    pub queue_index: u32,
}

pub struct XrFrameInfo {
    pub swapchain_index: u32,
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

        let render_pass = RenderPass::new(&vulkan_info, &swapchain)?;

        let pipeline = Pipeline::new(&vulkan_info, &swapchain, *render_pass, "default")?;

        let mut allocator = GpuAllocator::new(&vulkan_info)?;

        let transfer = Transfer::new(&vulkan_info)?;

        let frames = [
            Some(Frame::new(&vulkan_info, &mut allocator)?),
            Some(Frame::new(&vulkan_info, &mut allocator)?),
        ];

        let size = window.inner_size();
        let aspect = size.width as f32 / size.height as f32;

        Ok(Self {
            scene: Default::default(),
            frames,
            transfer,
            current_frame_index: false,
            allocator,
            pipeline,
            render_pass,
            swapchain,
            vr_swapchain: None,
            vulkan_info,
            aspect,
        })
    }

    pub fn new_vr<I, D>(
        window: &Window,
        create_instance: I,
        create_device: D,
    ) -> Result<(Self, XrCreateInfo)>
    where
        I: FnOnce(
            vk::PFN_vkGetInstanceProcAddr,
            &vk::InstanceCreateInfo,
        ) -> (vk::Instance, vk::PhysicalDevice),
        D: FnOnce(
            vk::PFN_vkGetInstanceProcAddr,
            vk::PhysicalDevice,
            &vk::DeviceCreateInfo,
        ) -> vk::Device,
    {
        let entry = EntryLoader::new()?;
        let mut physical_device = vk::PhysicalDevice::null();
        let instance = Instance::new_vr(&entry, window, |create_info| {
            let (instance, device) = create_instance(entry.get_instance_proc_addr, create_info);
            physical_device = device;
            instance
        })?;
        let device = Device::new_vr(&instance, physical_device, |create_info| {
            create_device(entry.get_instance_proc_addr, physical_device, create_info)
        })?;
        let descriptor_set_layouts = DescriptorSetLayouts::new(&device)?;

        let vulkan_info = VulkanInfo {
            descriptor_set_layouts,
            device,
            instance,
            _entry: entry,
        };

        let swapchain = Swapchain::new(&vulkan_info)?;

        let render_pass = RenderPass::new(&vulkan_info, &swapchain)?;

        let pipeline = Pipeline::new(&vulkan_info, &swapchain, *render_pass, "default")?;

        let mut allocator = GpuAllocator::new(&vulkan_info)?;

        let transfer = Transfer::new(&vulkan_info)?;

        let frames = [
            Some(Frame::new(&vulkan_info, &mut allocator)?),
            Some(Frame::new(&vulkan_info, &mut allocator)?),
        ];

        let size = window.inner_size();
        let aspect = size.width as f32 / size.height as f32;

        let vulkan = Self {
            scene: Default::default(),
            frames,
            transfer,
            current_frame_index: false,
            allocator,
            pipeline,
            render_pass,
            swapchain,
            vr_swapchain: None,
            vulkan_info,
            aspect,
        };

        let xr_create_info = XrCreateInfo {
            instance: vulkan.vulkan_info.instance.handle,
            physical_device: vulkan.vulkan_info.device.physical_device,
            device: vulkan.vulkan_info.device.handle,
            queue_family_index: vulkan.vulkan_info.device.queues.graphics.family_index,
            queue_index: 0,
        };

        Ok((vulkan, xr_create_info))
    }
}

impl Drop for Vulkan {
    fn drop(&mut self) {
        unsafe {
            self.vulkan_info.device.device_wait_idle().unwrap();

            self.scene.destroy(&mut self.allocator);

            for frame in &mut self.frames {
                frame.take().unwrap().destroy(&mut self.allocator);
            }
        }
    }
}

impl Vulkan {
    pub fn window_resized(&mut self, size: PhysicalSize<u32>) {
        self.aspect = size.width as f32 / size.height as f32;
    }

    pub fn create_vr_swapchain<I>(
        &mut self,
        create_info: &mut VrSwapchainCreateInfo<I>,
    ) -> Result<()>
    where
        I: Iterator<Item = vk::Image>,
    {
        self.vr_swapchain = Some(VrSwapchain::new(&self.vulkan_info, create_info)?);
        Ok(())
    }

    pub async fn frame(&mut self, frame_buffer: &FrameBufferReader<'_>) {
        self.update_scene(frame_buffer).await;

        let frame_info = self.frames[self.current_frame_index as usize]
            .as_mut()
            .unwrap()
            .begin()
            .unwrap();

        let swapchain_image_index = self
            .swapchain
            .acquire_next_image(frame_info.acquire_semaphore)
            .unwrap();

        // render pass begin

        self.render_pass
            .begin(frame_info.command_buffer, swapchain_image_index as usize);

        // render

        self.pipeline.bind(frame_info.command_buffer);

        let scene_data = {
            let mut proj_matrix = perspective_lh_zo(self.aspect, 1.0, 0.01, 50.0);
            proj_matrix[5] *= -1.0;

            let view_matrix = frame_buffer
                .camera_info()
                .map(|info| look_at_lh(&info.location, &info.focus, &info.up))
                .unwrap_or_else(Mat4::identity);

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

        for (i, static_mesh) in self.scene.static_meshes.iter().enumerate() {
            unsafe {
                self.vulkan_info.device.cmd_bind_descriptor_sets(
                    frame_info.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline.layout(),
                    0,
                    &[frame_info.instance_descriptor_set],
                    &[i as u32],
                );

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

        // render pass end

        self.render_pass.end(frame_info.command_buffer);

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

    pub async fn frame_vr(
        &mut self,
        vr_frame_info: &XrFrameInfo,
        frame_buffer: &FrameBufferReader<'_>,
    ) {
        unsafe {
            self.vulkan_info.device.device_wait_idle().unwrap();
        }

        self.update_scene(frame_buffer).await;

        let swapchain = self.vr_swapchain.as_ref().unwrap();

        let frame_info = self.frames[self.current_frame_index as usize]
            .as_mut()
            .unwrap()
            .begin()
            .unwrap();

        let swapchain_image = swapchain.images[vr_frame_info.swapchain_index as usize];
        let swapchain_image_view = swapchain.image_views[vr_frame_info.swapchain_index as usize];

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
                layer_count: 2,
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
                extent: swapchain.surface_extent,
            })
            .layer_count(2)
            .color_attachments(&color_attachments);

        unsafe {
            self.vulkan_info
                .device
                .cmd_begin_rendering(frame_info.command_buffer, &rendering_info);
        }

        // render static mesh instances

        self.pipeline.bind(frame_info.command_buffer);

        let scene_data = {
            let mut proj_matrix = perspective_lh_zo(self.aspect, 1.0, 0.01, 50.0);
            proj_matrix[5] *= -1.0;

            let view_matrix = frame_buffer
                .camera_info()
                .map(|info| look_at_lh(&info.location, &info.focus, &info.up))
                .unwrap_or_else(Mat4::identity);

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

        for (i, static_mesh) in self.scene.static_meshes.iter().enumerate() {
            unsafe {
                self.vulkan_info.device.cmd_bind_descriptor_sets(
                    frame_info.command_buffer,
                    vk::PipelineBindPoint::GRAPHICS,
                    self.pipeline.layout(),
                    0,
                    &[frame_info.instance_descriptor_set],
                    &[i as u32],
                );

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
                layer_count: 2,
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

        self.current_frame_index = !self.current_frame_index;
    }

    async fn update_scene(&mut self, frame_buffer: &FrameBufferReader<'_>) {
        let upload_meshes = async {
            self.transfer.begin_transfers(&mut self.allocator).unwrap();

            for spawned in frame_buffer.spawned_static_meshes() {
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

                self.scene.static_meshes.push(scene::StaticMesh {
                    entity_id: spawned.entity_id,
                    vertex_buffer,
                    vertex_offset: VERTEX_OFFSET as vk::DeviceSize,
                });
            }

            self.transfer.submit_transfers().unwrap();
        };

        let update_instances = async {
            let frame = self.frames[self.current_frame_index as usize]
                .as_mut()
                .unwrap();

            frame.update_instance(
                0,
                &InstanceData {
                    model_matrix: Mat4::identity(),
                },
            );
        };

        pin_mut!(upload_meshes);
        pin_mut!(update_instances);

        parallel([upload_meshes, update_instances]).await;

        unsafe {
            self.vulkan_info.device.device_wait_idle().unwrap();
        }
    }
}
