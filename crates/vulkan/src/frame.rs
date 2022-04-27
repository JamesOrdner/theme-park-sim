use std::{mem::size_of, sync::Arc};

use anyhow::Result;
use erupt::{vk, DeviceLoader};
use gpu_alloc::{Request, UsageFlags};

use crate::{
    allocator::{Buffer, GpuAllocator},
    descriptor_set_layouts::{descriptor_pool_sizes, InstanceData},
    VulkanInfo,
};

/// CurrentFrameInfo does not implement Clone or Copy, providing safety
/// by ensuring that the command buffer is not accessed at an unexpected time
pub struct CurrentFrameInfo {
    pub command_buffer: vk::CommandBuffer,
    pub acquire_semaphore: vk::Semaphore,
    pub instance_descriptor_set: vk::DescriptorSet,
}

pub struct Frame {
    device: Arc<DeviceLoader>,
    graphics_queue: vk::Queue,
    descriptor_pool: vk::DescriptorPool,
    command_pool: vk::CommandPool,
    command_fence: vk::Fence,
    command_buffer: vk::CommandBuffer,
    acquire_semaphore: vk::Semaphore,
    present_semaphore: vk::Semaphore,
    instance_data_alignment: vk::DeviceSize,
    instance_buffer: Buffer,
    instance_descriptor_set: vk::DescriptorSet,
}

unsafe impl Send for Frame {}

impl Frame {
    pub fn new(vulkan: &VulkanInfo, allocator: &mut GpuAllocator) -> Result<Self> {
        // descriptor pool
        let descriptor_pool_sizes = descriptor_pool_sizes();

        let descriptor_pool_create_info = vk::DescriptorPoolCreateInfoBuilder::new()
            .max_sets(1)
            .pool_sizes(&descriptor_pool_sizes);

        let descriptor_pool = unsafe {
            vulkan
                .device
                .create_descriptor_pool(&descriptor_pool_create_info, None)
                .result()?
        };

        // command pool + sync

        let command_pool_create_info = vk::CommandPoolCreateInfoBuilder::new()
            .flags(vk::CommandPoolCreateFlags::TRANSIENT)
            .queue_family_index(vulkan.device.queues.graphics.family_index);

        let command_pool = unsafe {
            vulkan
                .device
                .create_command_pool(&command_pool_create_info, None)
                .result()?
        };

        let command_fence_create_info =
            vk::FenceCreateInfoBuilder::new().flags(vk::FenceCreateFlags::SIGNALED);

        let command_fence = unsafe {
            vulkan
                .device
                .create_fence(&command_fence_create_info, None)
                .result()?
        };

        // allocate command buffer

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfoBuilder::new()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let command_buffer = unsafe {
            vulkan
                .device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .result()?
                .into_iter()
                .next()
                .unwrap()
        };

        // acquire + present semaphores

        let semaphore_create_info = vk::SemaphoreCreateInfoBuilder::new();

        let acquire_semaphore = unsafe {
            vulkan
                .device
                .create_semaphore(&semaphore_create_info, None)
                .result()?
        };

        let present_semaphore = unsafe {
            vulkan
                .device
                .create_semaphore(&semaphore_create_info, None)
                .result()?
        };

        // allocate descriptor set

        let descriptor_set_layouts = [vulkan.descriptor_set_layouts.instance_layout];

        let descriptor_set_allocate_info = vk::DescriptorSetAllocateInfoBuilder::new()
            .descriptor_pool(descriptor_pool)
            .set_layouts(&descriptor_set_layouts);

        let instance_descriptor_set = unsafe {
            vulkan
                .device
                .allocate_descriptor_sets(&descriptor_set_allocate_info)
                .result()?
                .into_iter()
                .last()
                .unwrap()
        };

        // allocate uniform buffer memory

        let min_ubo_alignment = unsafe {
            vulkan
                .instance
                .get_physical_device_properties(vulkan.device.physical_device)
                .limits
                .min_uniform_buffer_offset_alignment
        };

        let instance_data_alignment =
            (size_of::<InstanceData>() as vk::DeviceSize + min_ubo_alignment - 1)
                & !(min_ubo_alignment - 1);

        let uniform_buffer_create_info = vk::BufferCreateInfoBuilder::new()
            .size(instance_data_alignment * 4)
            .usage(vk::BufferUsageFlags::UNIFORM_BUFFER)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let instance_buffer = allocator.alloc(
            &uniform_buffer_create_info,
            Request {
                size: instance_data_alignment * 4,
                align_mask: instance_data_alignment,
                usage: UsageFlags::UPLOAD,
                memory_types: !0,
            },
        )?;

        // associate uniform buffer memory with descirptor set

        let instance_descriptor_buffer_info = [vk::DescriptorBufferInfoBuilder::new()
            .buffer(instance_buffer.buffer)
            .offset(0)
            .range(vk::WHOLE_SIZE)];

        let instance_descriptor_set_writes = [vk::WriteDescriptorSetBuilder::new()
            .dst_set(instance_descriptor_set)
            .dst_binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .buffer_info(&instance_descriptor_buffer_info)];

        unsafe {
            vulkan
                .device
                .update_descriptor_sets(&instance_descriptor_set_writes, &[]);
        }

        Ok(Frame {
            device: vulkan.device.clone_loader(),
            graphics_queue: vulkan.device.queues.graphics.queue,
            descriptor_pool,
            command_pool,
            command_fence,
            command_buffer,
            acquire_semaphore,
            present_semaphore,
            instance_data_alignment,
            instance_buffer,
            instance_descriptor_set,
        })
    }

    pub unsafe fn destroy(self, allocator: &mut GpuAllocator) {
        allocator.dealloc(self.instance_buffer);
        self.device.destroy_semaphore(self.acquire_semaphore, None);
        self.device.destroy_semaphore(self.present_semaphore, None);
        self.device.destroy_fence(self.command_fence, None);
        self.device.destroy_command_pool(self.command_pool, None);
        self.device
            .destroy_descriptor_pool(self.descriptor_pool, None);
    }
}

impl Frame {
    pub fn begin(&self) -> Result<CurrentFrameInfo> {
        let command_buffer_begin_info = vk::CommandBufferBeginInfoBuilder::new()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            self.device
                .wait_for_fences(&[self.command_fence], false, u64::MAX)
                .result()?;
            self.device.reset_fences(&[self.command_fence]).result()?;
            self.device
                .reset_command_pool(self.command_pool, vk::CommandPoolResetFlags::empty())
                .result()?;
            self.device
                .begin_command_buffer(self.command_buffer, &command_buffer_begin_info)
                .result()?;
        }

        Ok(CurrentFrameInfo {
            command_buffer: self.command_buffer,
            acquire_semaphore: self.acquire_semaphore,
            instance_descriptor_set: self.instance_descriptor_set,
        })
    }

    pub fn update_instance(&mut self, instance_index: usize, transform: &InstanceData) {
        unsafe {
            self.instance_buffer.write(
                &transform,
                instance_index * self.instance_data_alignment as usize,
            );
        };
    }

    pub fn end_and_submit(&self, _current_frame_info: CurrentFrameInfo) -> Result<vk::Semaphore> {
        let wait_semaphores = [self.acquire_semaphore];
        let command_buffers = [self.command_buffer];
        let signal_semaphores = [self.present_semaphore];

        let submits_info = [vk::SubmitInfoBuilder::new()
            .wait_semaphores(&wait_semaphores)
            .wait_dst_stage_mask(&[vk::PipelineStageFlags::TOP_OF_PIPE])
            .command_buffers(&command_buffers)
            .signal_semaphores(&signal_semaphores)];

        unsafe {
            self.device
                .end_command_buffer(self.command_buffer)
                .result()?;

            self.device
                .queue_submit(self.graphics_queue, &submits_info, self.command_fence)
                .result()?;
        }

        Ok(self.present_semaphore)
    }
}
