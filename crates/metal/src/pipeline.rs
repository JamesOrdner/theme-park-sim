use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Error, Result};
use metal::{
    CompileOptions, Device, Function, MTLLanguageVersion, MTLPixelFormat, MTLVertexFormat,
    MTLVertexStepFunction, RenderPipelineDescriptor, RenderPipelineState, VertexDescriptor,
};
use naga::{
    back::msl,
    front::spv,
    valid::{Capabilities, ValidationFlags, Validator},
};

pub struct Pipeline {
    pub state: RenderPipelineState,
}

impl Pipeline {
    pub fn new(name: &str, device: &Device) -> Result<Self> {
        let vertex_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../res/shaders")
            .join(name)
            .with_extension("vert.spv");

        let vertex_function =
            convert_spv(&vertex_path, device).with_context(|| vertex_path.display().to_string())?;

        let fragment_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../res/shaders")
            .join(name)
            .with_extension("frag.spv");

        let fragment_function = convert_spv(&fragment_path, device)
            .with_context(|| fragment_path.display().to_string())?;

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

fn convert_spv(path: &Path, device: &Device) -> Result<Function> {
    let module = {
        let source = fs::read(path)?;
        let options = spv::Options {
            adjust_coordinate_space: false,
            strict_capabilities: true,
            block_ctx_dump_prefix: None,
        };
        spv::parse_u8_slice(&source, &options)?
    };

    let info = Validator::new(ValidationFlags::all(), Capabilities::all()).validate(&module)?;

    let options = msl::Options {
        lang_version: (2, 2), // macOS 10.15+
        fake_missing_bindings: false,
        ..Default::default()
    };

    let pipeline_options = msl::PipelineOptions {
        allow_point_size: false,
    };

    let (source, _) = msl::write_string(&module, &info, &options, &pipeline_options)?;

    let compile_options = CompileOptions::new();
    compile_options.set_language_version(MTLLanguageVersion::V2_2);

    device
        .new_library_with_source(&source, &compile_options)
        .map_err(Error::msg)?
        .get_function("main_", None)
        .map_err(Error::msg)
}
