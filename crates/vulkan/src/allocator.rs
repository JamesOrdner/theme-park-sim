use std::{mem::size_of, ptr::NonNull, sync::Arc};

use anyhow::Result;
use erupt::{vk, DeviceLoader};
use gpu_alloc::{Config, MemoryBlock, Request, UsageFlags};
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
    ) -> GpuBuffer {
        let buffer = unsafe { self.device.create_buffer(buffer_create_info, None).unwrap() };

        let mut block = unsafe {
            self.allocator
                .alloc(EruptMemoryDevice::wrap(&self.device), request)
                .unwrap()
        };

        unsafe {
            self.device
                .bind_buffer_memory(buffer, *block.memory(), 0)
                .unwrap();
        }

        let data = if !request.usage.contains(UsageFlags::FAST_DEVICE_ACCESS) {
            let data = unsafe {
                block
                    .map(
                        EruptMemoryDevice::wrap(&self.device),
                        0,
                        buffer_create_info.size as usize,
                    )
                    .unwrap()
            };
            Some(data)
        } else {
            None
        };

        GpuBuffer {
            buffer,
            block,
            data,
        }
    }

    pub fn dealloc(&mut self, buffer: GpuBuffer) {
        unsafe {
            self.device.destroy_buffer(buffer.buffer, None);
            self.allocator
                .dealloc(EruptMemoryDevice::wrap(&self.device), buffer.block)
        }
    }
}

pub struct GpuBuffer {
    block: MemoryBlock<vk::DeviceMemory>,
    pub buffer: vk::Buffer,
    data: Option<NonNull<u8>>,
}

unsafe impl Send for GpuBuffer {}

impl GpuBuffer {
    pub unsafe fn write<T>(&mut self, data: &T, offset: usize)
    where
        T: Copy,
    {
        debug_assert!(self.data.is_some(), "cannot write to device local memory");
        let data_ptr = self.data.unwrap_unchecked().as_ptr();
        let data_offset = data_ptr.add(offset) as *mut T;
        debug_assert!(offset + size_of::<T>() <= self.block.size() as usize);
        *data_offset = *data;
    }
}
