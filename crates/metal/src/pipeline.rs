use std::{fs, path::PathBuf};

use anyhow::{Error, Result};
use metal::{
    CompileOptions, Device, MTLLanguageVersion, MTLPixelFormat, MTLVertexFormat,
    MTLVertexStepFunction, RenderPipelineDescriptor, RenderPipelineState, VertexDescriptor,
};
pub struct Pipeline {
    pub state: RenderPipelineState,
}

impl Pipeline {
    pub fn new(name: &str, device: &Device) -> Result<Self> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../res/shaders")
            .join(name)
            .with_extension("metal");

        let source = fs::read_to_string(path)?;

        let compile_options = CompileOptions::new();
        compile_options.set_language_version(MTLLanguageVersion::V2_2);

        let library = device
            .new_library_with_source(&source, &compile_options)
            .map_err(Error::msg)?;

        let vertex_function = library
            .get_function("vertexShader", None)
            .map_err(Error::msg)?;

        let fragment_function = library
            .get_function("fragmentShader", None)
            .map_err(Error::msg)?;

        let pipeline_descriptor = RenderPipelineDescriptor::new();
        pipeline_descriptor.set_vertex_function(Some(&vertex_function));
        pipeline_descriptor.set_fragment_function(Some(&fragment_function));

        let vertex_descriptor = VertexDescriptor::new();
        let position_attr = vertex_descriptor.attributes().object_at(0).unwrap();
        position_attr.set_format(MTLVertexFormat::Float3);
        position_attr.set_offset(0);
        position_attr.set_buffer_index(0);
        let position_layout = vertex_descriptor.layouts().object_at(0).unwrap();
        position_layout.set_stride(12);
        position_layout.set_step_rate(1);
        position_layout.set_step_function(MTLVertexStepFunction::PerVertex);
        pipeline_descriptor.set_vertex_descriptor(Some(vertex_descriptor));

        let attachment = pipeline_descriptor
            .color_attachments()
            .object_at(0)
            .unwrap();
        attachment.set_pixel_format(MTLPixelFormat::BGRA8Unorm);

        let state = device
            .new_render_pipeline_state(&pipeline_descriptor)
            .map_err(Error::msg)?;

        Ok(Self { state })
    }
}
