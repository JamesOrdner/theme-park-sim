use std::{ffi::CStr, fs, mem::size_of, path::Path, sync::Arc};

use anyhow::{Context, Error, Result};
use erupt::{utils::decode_spv, vk, DeviceLoader, ExtendableFrom};
use memoffset::offset_of;
use nalgebra_glm::Mat4;

use crate::{cstr, static_mesh::Vertex, swapchain::Swapchain, VulkanInfo};

#[repr(C)]
pub struct SceneData {
    pub proj_matrix: Mat4,
    pub view_matrix: Mat4,
}

pub struct Pipeline {
    device: Arc<DeviceLoader>,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
}

impl Pipeline {
    pub fn new(vulkan: &VulkanInfo, swapchain: &Swapchain, shader_name: &str) -> Result<Self> {
        let shader_entry = cstr!("main");
        let shader = Shader::new(vulkan, shader_name, unsafe { CStr::from_ptr(shader_entry) })?;

        let vertex_input_binding_descriptions = [vk::VertexInputBindingDescriptionBuilder::new()
            .binding(0)
            .stride(size_of::<Vertex>() as u32)
            .input_rate(vk::VertexInputRate::VERTEX)];

        let vertex_input_attribue_descriptions =
            [vk::VertexInputAttributeDescriptionBuilder::new()
                .location(0)
                .format(vk::Format::R32G32B32_SFLOAT)
                .offset(offset_of!(Vertex, location) as u32)];

        let vertex_input_create_info = vk::PipelineVertexInputStateCreateInfoBuilder::new()
            .vertex_binding_descriptions(&vertex_input_binding_descriptions)
            .vertex_attribute_descriptions(&vertex_input_attribue_descriptions);

        let input_assembly_create_info = vk::PipelineInputAssemblyStateCreateInfoBuilder::new()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST);

        let viewports = [vk::ViewportBuilder::new()
            .width(swapchain.surface_extent.width as f32)
            .height(swapchain.surface_extent.height as f32)
            .min_depth(0.0)
            .max_depth(1.0)];

        let scissors = [vk::Rect2DBuilder::new()
            .offset(vk::Offset2D { x: 0, y: 0 })
            .extent(swapchain.surface_extent)];

        let viewport_create_info = vk::PipelineViewportStateCreateInfoBuilder::new()
            .viewports(&viewports)
            .scissors(&scissors);

        let rasterization_create_info = vk::PipelineRasterizationStateCreateInfoBuilder::new()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0);

        let multisample_create_info = vk::PipelineMultisampleStateCreateInfoBuilder::new()
            .rasterization_samples(vk::SampleCountFlagBits::_1);

        let depth_stencil_create_info = vk::PipelineDepthStencilStateCreateInfoBuilder::new()
            .depth_test_enable(true)
            .depth_write_enable(true)
            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
            .depth_bounds_test_enable(false)
            .stencil_test_enable(false);

        let color_blend_attachments = [vk::PipelineColorBlendAttachmentStateBuilder::new()
            .blend_enable(false)
            .color_write_mask(vk::ColorComponentFlags::all())];

        let color_blend_create_info = vk::PipelineColorBlendStateCreateInfoBuilder::new()
            .attachments(&color_blend_attachments);

        let color_attachment_formats = [swapchain.surface_format.format];
        let mut pipeline_rendering_create_info = vk::PipelineRenderingCreateInfoBuilder::new()
            .color_attachment_formats(&color_attachment_formats);

        let pipeline_layout = pipeline_layout(vulkan)?;

        let pipeline_create_info = [vk::GraphicsPipelineCreateInfoBuilder::new()
            .stages(&shader.stages)
            .vertex_input_state(&vertex_input_create_info)
            .input_assembly_state(&input_assembly_create_info)
            .viewport_state(&viewport_create_info)
            .rasterization_state(&rasterization_create_info)
            .multisample_state(&multisample_create_info)
            .depth_stencil_state(&depth_stencil_create_info)
            .color_blend_state(&color_blend_create_info)
            .layout(pipeline_layout)
            .extend_from(&mut pipeline_rendering_create_info)];

        let pipeline = unsafe {
            vulkan
                .device
                .create_graphics_pipelines(vk::PipelineCache::null(), &pipeline_create_info, None)
                .map_err(|_| Error::msg("create_graphics_pipelines"))?[0]
        };

        Ok(Pipeline {
            device: vulkan.device.clone_loader(),
            pipeline,
            pipeline_layout,
        })
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe {
            self.device.destroy_pipeline(self.pipeline, None);
            self.device
                .destroy_pipeline_layout(self.pipeline_layout, None);
        }
    }
}

impl Pipeline {
    pub fn layout(&self) -> vk::PipelineLayout {
        self.pipeline_layout
    }

    pub fn bind(&self, command_buffer: vk::CommandBuffer) {
        unsafe {
            self.device.cmd_bind_pipeline(
                command_buffer,
                vk::PipelineBindPoint::GRAPHICS,
                self.pipeline,
            );
        }
    }
}

fn pipeline_layout(vulkan: &VulkanInfo) -> Result<vk::PipelineLayout> {
    let descriptor_set_layouts = [vulkan.descriptor_set_layouts.instance_layout];

    let push_constant_ranges = [vk::PushConstantRangeBuilder::new()
        .stage_flags(vk::ShaderStageFlags::VERTEX)
        .offset(0)
        .size(size_of::<SceneData>().try_into()?)];

    let pipeline_layout_create_info = vk::PipelineLayoutCreateInfoBuilder::new()
        .set_layouts(&descriptor_set_layouts)
        .push_constant_ranges(&push_constant_ranges);

    let pipeline_layout = unsafe {
        vulkan
            .device
            .create_pipeline_layout(&pipeline_layout_create_info, None)
            .result()?
    };

    Ok(pipeline_layout)
}

struct Shader<'a> {
    vulkan: &'a VulkanInfo,
    stages: [vk::PipelineShaderStageCreateInfoBuilder<'a>; 2],
    vert_shader_module: vk::ShaderModule,
    frag_shader_module: vk::ShaderModule,
}

const SHADERS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../res/shaders");

impl<'a> Shader<'a> {
    fn new(vulkan: &'a VulkanInfo, name: &str, entry: &'a CStr) -> Result<Self> {
        let path = Path::new(SHADERS_DIR).join(name);

        let vert_code = read_shader_file(&path.with_extension("vert.spv"))?;
        let vert_shader_module = unsafe { create_shader_module(vulkan, &vert_code)? };

        let frag_code = read_shader_file(&path.with_extension("frag.spv"))?;
        let frag_shader_module = unsafe { create_shader_module(vulkan, &frag_code)? };

        let stages = [
            vk::PipelineShaderStageCreateInfoBuilder::new()
                .stage(vk::ShaderStageFlagBits::VERTEX)
                .module(vert_shader_module)
                .name(entry),
            vk::PipelineShaderStageCreateInfoBuilder::new()
                .stage(vk::ShaderStageFlagBits::FRAGMENT)
                .module(frag_shader_module)
                .name(entry),
        ];

        Ok(Shader {
            vulkan,
            stages,
            vert_shader_module,
            frag_shader_module,
        })
    }
}

impl<'a> Drop for Shader<'a> {
    fn drop(&mut self) {
        unsafe {
            self.vulkan
                .device
                .destroy_shader_module(self.vert_shader_module, None);
            self.vulkan
                .device
                .destroy_shader_module(self.frag_shader_module, None);
        }
    }
}

fn read_shader_file(path: &Path) -> Result<Vec<u32>> {
    let file = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    decode_spv(&file).with_context(|| format!("failed to decode {}", path.display()))
}

unsafe fn create_shader_module(vulkan: &VulkanInfo, code: &[u32]) -> Result<vk::ShaderModule> {
    let shader_module_create_info = vk::ShaderModuleCreateInfoBuilder::new().code(code);
    let shader_module = vulkan
        .device
        .create_shader_module(&shader_module_create_info, None)
        .result()?;
    Ok(shader_module)
}
