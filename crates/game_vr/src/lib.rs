use std::path::Path;

use anyhow::{Error, Result};
use openxr as xr;

pub struct GameVr {}

impl GameVr {
    pub fn new() -> Result<Self> {
        let entry = xr::Entry::load().or_else(|_| {
            let linux_path = Path::new("/home/james/snap/steam/common/.local/share/Steam/steamapps/common/SteamVR/bin/linux64/libopenxr_loader.so");
            xr::Entry::load_from(linux_path)
        })?;

        let available_extensions = entry.enumerate_extensions()?;

        if !available_extensions.khr_vulkan_enable2 {
            return Err(Error::msg("khr_vulkan_enable2 extension unavailable"));
        }

        let mut enabled_extensions = xr::ExtensionSet::default();
        enabled_extensions.khr_vulkan_enable2 = true;

        let instance =
            entry.create_instance(&xr::ApplicationInfo::default(), &enabled_extensions, &[])?;

        let system = instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;

        let blend_mode = instance
            .enumerate_environment_blend_modes(system, xr::ViewConfigurationType::PRIMARY_STEREO)?
            .iter()
            .find(|mode| **mode == xr::EnvironmentBlendMode::OPAQUE)
            .copied()
            .ok_or(Error::msg("opaque blend mode unavailable"))?;

        println!("blend_mode {}", blend_mode.into_raw());

        Ok(Self {})
    }
}
