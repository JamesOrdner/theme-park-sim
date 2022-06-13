#![cfg(not(target_vendor = "apple"))]

use std::{mem, slice};

use anyhow::Result;
use erupt::{vk, EntryLoader};
use frame_buffer::FrameBufferReader;
use nalgebra_glm::{look_at_lh, perspective_lh_zo, Mat4};
use pipeline::SceneData;
use render_pass::{RenderPass, VrRenderPassCreateInfo};
use scene::Scene;
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
    vr_render_pass: Option<RenderPass>,
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
            vr_render_pass: None,
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
            vr_render_pass: None,
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

    pub fn create_vr_swapchain(&mut self, create_info: &VrSwapchainCreateInfo) -> Result<()> {
        let vr_swapchain = VrSwapchain::new(&self.vulkan_info, create_info)?;

        self.vr_render_pass = Some(RenderPass::new_vr(
            &self.vulkan_info,
            &VrRenderPassCreateInfo {
                surface_extent: create_info.surface_extent,
                image_format: create_info.image_format,
                image_views: &vr_swapchain.image_views,
            },
        )?);

        self.vr_swapchain = Some(vr_swapchain);

        Ok(())
    }

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

        // render pass begin

        self.render_pass
            .begin(frame_info.command_buffer, swapchain_image_index as usize);

        // render

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
            .end_and_submit(frame_info);

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

        self.update_scene(frame_buffer);

        let render_pass = self.vr_render_pass.as_ref().unwrap();

        let frame_info = self.frames[self.current_frame_index as usize]
            .as_mut()
            .unwrap()
            .begin()
            .unwrap();

        // render pass begin

        render_pass.begin(
            frame_info.command_buffer,
            vr_frame_info.swapchain_index as usize,
        );

        // render

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

        render_pass.end(frame_info.command_buffer);

        self.frames[self.current_frame_index as usize]
            .as_mut()
            .unwrap()
            .end_and_submit_vr(frame_info);

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

        // spawn

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

            self.scene.static_meshes.insert(
                spawned.entity_id,
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

        let frame = self.frames[self.current_frame_index as usize]
            .as_mut()
            .unwrap();

        for static_mesh in self.scene.static_meshes.values() {
            frame.update_instance(
                0,
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
