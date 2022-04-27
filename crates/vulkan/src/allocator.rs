use std::{mem::size_of, ptr::NonNull, sync::Arc};

use anyhow::Result;
use erupt::{vk, DeviceLoader};
use gpu_alloc::{Config, MemoryBlock, Request};
use gpu_alloc_erupt::{device_properties, EruptMemoryDevice};

use crate::VulkanInfo;

pub struct GpuAllocator {
    device: Arc<DeviceLoader>,
    allocator: gpu_alloc::GpuAllocator<vk::DeviceMemory>,
}

impl GpuAllocator {
    pub fn new(vulkan: &VulkanInfo) -> Result<Self> {
        let config = Config::i_am_prototyping();
        let mut props =
            unsafe { device_properties(&vulkan.instance, vulkan.device.physical_device) }?;
        props.buffer_device_address = false;

        let allocator = gpu_alloc::GpuAllocator::new(config, props);

        Ok(Self {
            device: vulkan.device.clone_loader(),
            allocator,
        })
    }
}

impl Drop for GpuAllocator {
    fn drop(&mut self) {
        unsafe {
            self.allocator
                .cleanup(EruptMemoryDevice::wrap(&self.device))
        }
    }
}

impl GpuAllocator {
    pub fn alloc(
        &mut self,
        buffer_create_info: &vk::BufferCreateInfo,
        request: Request,
    ) -> Result<Buffer> {
        let buffer = unsafe {
            self.device
                .create_buffer(buffer_create_info, None)
                .result()?
        };

        let mut block = unsafe {
            self.allocator
                .alloc(EruptMemoryDevice::wrap(&self.device), request)?
        };

        unsafe {
            self.device
                .bind_buffer_memory(buffer, *block.memory(), 0)
                .result()?;
        }

        let data = unsafe {
            block.map(
                EruptMemoryDevice::wrap(&self.device),
                0,
                buffer_create_info.size as usize,
            )?
        };

        Ok(Buffer {
            buffer,
            block,
            data,
        })
    }

    pub fn dealloc(&mut self, buffer: Buffer) {
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
            self.allocator
                .dealloc(EruptMemoryDevice::wrap(&self.device), buffer.block)
        }
    }
}

pub struct Buffer {
    block: MemoryBlock<vk::DeviceMemory>,
    pub buffer: vk::Buffer,
    data: NonNull<u8>,
}

impl Buffer {
    pub unsafe fn write<T>(&mut self, data: &T, offset: usize)
    where
        T: Copy,
    {
        let data_offset = self.data.as_ptr().add(offset) as *mut T;
        debug_assert!(offset + size_of::<T>() <= self.block.size() as usize);
        *data_offset = *data;
    }
}
