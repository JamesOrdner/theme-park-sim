use std::{mem, slice, sync::Arc};

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
        usage: UsageFlags,
    ) -> GpuBuffer {
        let buffer = unsafe { self.device.create_buffer(buffer_create_info, None).unwrap() };

        let memory_requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };

        let request = Request {
            size: buffer_create_info.size,
            align_mask: memory_requirements.alignment - 1,
            usage,
            memory_types: memory_requirements.memory_type_bits,
        };

        let block = unsafe {
            self.allocator
                .alloc(EruptMemoryDevice::wrap(&self.device), request)
                .unwrap()
        };

        unsafe {
            self.device
                .bind_buffer_memory(buffer, *block.memory(), block.offset())
                .unwrap();
        }

        GpuBuffer { buffer, block }
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
}

unsafe impl Send for GpuBuffer {}

impl GpuBuffer {
    pub unsafe fn write<T>(&mut self, device: &DeviceLoader, data: &T, offset: usize)
    where
        T: Copy,
    {
        let len = mem::size_of::<T>();
        let bytes = slice::from_raw_parts(data as *const T as *const u8, len);

        self.block
            .write_bytes(EruptMemoryDevice::wrap(device), offset as u64, bytes)
            .unwrap();
    }

    pub unsafe fn write_bytes(&mut self, device: &DeviceLoader, data: &[u8], offset: usize) {
        self.block
            .write_bytes(EruptMemoryDevice::wrap(device), offset as u64, data)
            .unwrap();
    }
}
