use std::{fs, path::PathBuf};

use anyhow::{Error, Result};
use metal::{CompileOptions, ComputePipelineState, Device, MTLLanguageVersion};

pub struct ComputePipeline {
    pub state: ComputePipelineState,
}

impl ComputePipeline {
    pub fn new(name: &str, device: &Device) -> Result<Self> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../res/shaders")
            .join(name)
            .with_extension("metal");

        let source = fs::read_to_string(path)?;

        let compile_options = CompileOptions::new();
        compile_options.set_language_version(MTLLanguageVersion::V2_2);

        let function = device
            .new_library_with_source(&source, &compile_options)
            .map_err(Error::msg)?
            .get_function("update_guest_locations", None)
            .map_err(Error::msg)?;

        let state = device
            .new_compute_pipeline_state_with_function(&function)
            .map_err(Error::msg)?;

        Ok(Self { state })
    }
}
