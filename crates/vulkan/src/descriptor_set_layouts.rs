use std::sync::Arc;

use anyhow::Result;
use erupt::{vk, DeviceLoader};
use nalgebra_glm::Mat4;

use crate::device::Device;

#[repr(C)]
pub struct InstanceData {
    pub model_matrix: Mat4,
}

pub struct DescriptorSetLayouts {
    device: Arc<DeviceLoader>,
    pub instance_layout: vk::DescriptorSetLayout,
}

impl DescriptorSetLayouts {
    pub fn new(device: &Device) -> Result<Self> {
        let instance_layout_binding = [vk::DescriptorSetLayoutBindingBuilder::new()
            .binding(0)
            .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
            .descriptor_count(1)
            .stage_flags(vk::ShaderStageFlags::VERTEX)];

        let instance_layout_create_info =
            vk::DescriptorSetLayoutCreateInfoBuilder::new().bindings(&instance_layout_binding);

        let instance_layout = unsafe {
            device
                .create_descriptor_set_layout(&instance_layout_create_info, None)
                .result()?
        };

        Ok(DescriptorSetLayouts {
            device: device.clone_loader(),
            instance_layout,
        })
    }
}

impl Drop for DescriptorSetLayouts {
    fn drop(&mut self) {
        unsafe {
            self.device
                .destroy_descriptor_set_layout(self.instance_layout, None);
        }
    }
}

pub fn descriptor_pool_sizes() -> [vk::DescriptorPoolSizeBuilder<'static>; 1] {
    [vk::DescriptorPoolSizeBuilder::new()
        ._type(vk::DescriptorType::UNIFORM_BUFFER_DYNAMIC)
        .descriptor_count(1)]
}
