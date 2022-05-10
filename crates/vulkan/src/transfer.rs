use std::sync::Arc;

use anyhow::Result;
use erupt::{vk, DeviceLoader};
use gpu_alloc::{Request, UsageFlags};

use crate::{
    allocator::{GpuAllocator, GpuBuffer},
    VulkanInfo,
};

pub struct Transfer {
    device: Arc<DeviceLoader>,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    transfer_queue: vk::Queue,
    fence: vk::Fence,
    transient_buffers: Vec<GpuBuffer>,
}

impl Transfer {
    pub fn new(vulkan: &VulkanInfo) -> Result<Self> {
        let command_pool_create_info = vk::CommandPoolCreateInfoBuilder::new()
            .flags(vk::CommandPoolCreateFlags::TRANSIENT)
            .queue_family_index(vulkan.device.queues.transfer.family_index);

        let command_pool = unsafe {
            vulkan
                .device
                .create_command_pool(&command_pool_create_info, None)
                .result()?
        };

        let command_buffer_allocate_info = vk::CommandBufferAllocateInfoBuilder::new()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);

        let command_buffer = unsafe {
            vulkan
                .device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .result()?[0]
        };

        let fence_create_info =
            vk::FenceCreateInfoBuilder::new().flags(vk::FenceCreateFlags::SIGNALED);

        let fence = unsafe {
            vulkan
                .device
                .create_fence(&fence_create_info, None)
                .result()?
        };

        Ok(Self {
            device: vulkan.device.clone_loader(),
            command_pool,
            command_buffer,
            transfer_queue: vulkan.device.queues.transfer.queue,
            fence,
            transient_buffers: Vec::new(),
        })
    }
}

impl Drop for Transfer {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_fence(self.fence, None);
            self.device
                .free_command_buffers(self.command_pool, &[self.command_buffer]);
            self.device.destroy_command_pool(self.command_pool, None);
        }
    }
}

impl Transfer {
    pub fn begin_transfers(&mut self, allocator: &mut GpuAllocator) -> Result<()> {
        unsafe {
            self.device
                .wait_for_fences(&[self.fence], false, u64::MAX)
                .result()?;
        }

        unsafe {
            self.device.reset_fences(&[self.fence]).result()?;
        }

        for buffer in self.transient_buffers.drain(..) {
            allocator.dealloc(buffer);
        }

        unsafe {
            self.device
                .reset_command_pool(self.command_pool, vk::CommandPoolResetFlags::empty())
                .result()?;
        }

        let command_buffer_begin_info = vk::CommandBufferBeginInfoBuilder::new()
            .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

        unsafe {
            self.device
                .begin_command_buffer(self.command_buffer, &command_buffer_begin_info)
                .result()?;
        }

        Ok(())
    }

    pub fn transfer_buffer(
        &mut self,
        data: &[u8],
        dst_usage: vk::BufferUsageFlags,
        allocator: &mut GpuAllocator,
    ) -> GpuBuffer {
        let transfer_buffer_create_info = vk::BufferCreateInfoBuilder::new()
            .size(data.len() as u64)
            .usage(vk::BufferUsageFlags::TRANSFER_SRC)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let transfer_buffer = allocator.alloc(
            &transfer_buffer_create_info,
            Request {
                size: data.len() as u64,
                align_mask: 0,
                usage: UsageFlags::UPLOAD | UsageFlags::TRANSIENT,
                memory_types: !0,
            },
        );

        let dst_buffer_create_info = vk::BufferCreateInfoBuilder::new()
            .size(data.len() as u64)
            .usage(vk::BufferUsageFlags::TRANSFER_DST | dst_usage)
            .sharing_mode(vk::SharingMode::EXCLUSIVE);

        let dst_buffer = allocator.alloc(
            &dst_buffer_create_info,
            Request {
                size: data.len() as u64,
                align_mask: 0,
                usage: UsageFlags::FAST_DEVICE_ACCESS,
                memory_types: !0,
            },
        );

        self.transient_buffers.push(transfer_buffer);

        dst_buffer
    }

    pub fn submit_transfers(&mut self) -> Result<()> {
        unsafe {
            self.device
                .end_command_buffer(self.command_buffer)
                .result()?;
        }

        let command_buffers = [self.command_buffer];
        let submit_info = vk::SubmitInfoBuilder::new().command_buffers(&command_buffers);

        unsafe {
            self.device
                .queue_submit(self.transfer_queue, &[submit_info], self.fence)
                .result()?;
        }

        Ok(())
    }
}
