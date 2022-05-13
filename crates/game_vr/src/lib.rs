use std::mem;

use anyhow::{Error, Result};
use erupt::vk;
use frame_buffer::FrameBufferReader;
use openxr as xr;
use vulkan::Vulkan;
use winit::window::Window;

const BLEND_MODE: xr::EnvironmentBlendMode = xr::EnvironmentBlendMode::OPAQUE;
const VIEW_CONFIGURATION: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

pub struct GameVr {
    session_state: xr::SessionState,
    swapchain_extent: xr::Extent2Di,
    space: xr::Space,
    swapchain: xr::Swapchain<xr::Vulkan>,
    frame_stream: xr::FrameStream<xr::Vulkan>,
    frame_waiter: xr::FrameWaiter,
    session: xr::Session<xr::Vulkan>,
    _system: xr::SystemId,
    instance: xr::Instance,
}

impl GameVr {
    pub fn new(window: &Window) -> Result<(Self, Vulkan)> {
        let entry = xr::Entry::load_from(std::path::Path::new("C:\\Program Files (x86)\\Steam\\steamapps\\common\\SteamVR\\bin\\win64\\openxr_loader.dll"))?;

        let available_extensions = entry.enumerate_extensions()?;

        if !available_extensions.khr_vulkan_enable2 {
            return Err(Error::msg("khr_vulkan_enable2 extension unavailable"));
        }

        let mut enabled_extensions = xr::ExtensionSet::default();
        enabled_extensions.khr_vulkan_enable2 = true;

        let instance = entry.create_instance(
            &xr::ApplicationInfo {
                application_name: "Theme Park Sim",
                application_version: 0,
                engine_name: "Theme Park Engine",
                engine_version: 0,
            },
            &enabled_extensions,
            &[],
        )?;

        let system = instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;

        let graphics_requirements = instance.graphics_requirements::<xr::Vulkan>(system)?;

        if graphics_requirements.min_api_version_supported > xr::Version::new(1, 3, 0) {
            return Err(Error::msg(format!(
                "Vulkan version older than supported by OpenXR: min version {}.{}.{}",
                graphics_requirements.min_api_version_supported.major(),
                graphics_requirements.min_api_version_supported.minor(),
                graphics_requirements.min_api_version_supported.patch(),
            )));
        }

        if graphics_requirements.max_api_version_supported < xr::Version::new(1, 3, 0) {
            log::warn!(
                "Vulkan version newer than supported by OpenXR: max version {}.{}.{}",
                graphics_requirements.max_api_version_supported.major(),
                graphics_requirements.max_api_version_supported.minor(),
                graphics_requirements.max_api_version_supported.patch(),
            );
        }

        instance
            .enumerate_environment_blend_modes(system, VIEW_CONFIGURATION)?
            .iter()
            .find(|mode| **mode == BLEND_MODE)
            .copied()
            .ok_or(Error::msg("opaque blend mode unavailable"))?;

        let (mut vulkan, xr_create_info) = Vulkan::new_vr(
            window,
            |get_instance_proc_addr, create_info| unsafe {
                let vk_instance = instance
                    .create_vulkan_instance(
                        system,
                        mem::transmute(get_instance_proc_addr),
                        create_info as *const _ as _,
                    )
                    .expect("OpenXR error creating Vulkan instance")
                    .expect("Vulkan error creating Vulkan instance");

                let physical_device = instance
                    .vulkan_graphics_device(system, vk_instance)
                    .expect("OpenXR error selecting Vulkan physcial device");

                (
                    vk::Instance(vk_instance as _),
                    vk::PhysicalDevice(physical_device as _),
                )
            },
            |get_instance_proc_addr, physical_device, create_info| unsafe {
                let device = instance
                    .create_vulkan_device(
                        system,
                        mem::transmute(get_instance_proc_addr),
                        physical_device.0 as *const _,
                        create_info as *const _ as *const _,
                    )
                    .expect("OpenXR error creating Vulkan device")
                    .expect("Vulkan error creating Vulkan device");

                vk::Device(device as _)
            },
        )?;

        let (session, frame_waiter, frame_stream) = unsafe {
            instance.create_session::<xr::Vulkan>(
                system,
                &xr::vulkan::SessionCreateInfo {
                    instance: xr_create_info.instance.0 as _,
                    physical_device: xr_create_info.physical_device.0 as _,
                    device: xr_create_info.device.0 as _,
                    queue_family_index: xr_create_info.queue_family_index,
                    queue_index: xr_create_info.queue_index,
                },
            )?
        };

        let views = instance.enumerate_view_configuration_views(system, VIEW_CONFIGURATION)?;

        if views.len() != 2 || views[0] != views[1] {
            return Err(Error::msg("invalid view configuration"));
        }

        let swapchain_extent = xr::Extent2Di {
            width: views[0].recommended_image_rect_width as i32,
            height: views[0].recommended_image_rect_height as i32,
        };

        let swapchain = session.create_swapchain(&xr::SwapchainCreateInfo {
            create_flags: xr::SwapchainCreateFlags::EMPTY,
            usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT
                | xr::SwapchainUsageFlags::SAMPLED,
            format: vk::Format::R8G8B8A8_SRGB.0 as _,
            sample_count: 1,
            width: swapchain_extent.width as u32,
            height: swapchain_extent.height as u32,
            face_count: 1,
            array_size: 2,
            mip_count: 1,
        })?;

        vulkan.create_vr_swapchain(&mut vulkan::VrSwapchainCreateInfo {
            surface_extent: vk::Extent2D {
                width: swapchain_extent.width as u32,
                height: swapchain_extent.height as u32,
            },
            image_format: vk::Format::R8G8B8A8_SRGB,
            images: swapchain.enumerate_images()?.into_iter().map(vk::Image),
        })?;

        let space =
            session.create_reference_space(xr::ReferenceSpaceType::STAGE, xr::Posef::IDENTITY)?;

        let vr = Self {
            session_state: xr::SessionState::UNKNOWN,
            swapchain_extent,
            space,
            swapchain,
            frame_stream,
            frame_waiter,
            session,
            _system: system,
            instance,
        };

        Ok((vr, vulkan))
    }

    pub async fn frame(&mut self, vulkan: &mut Vulkan, frame_buffer: &FrameBufferReader<'_>) {
        self.poll_events();

        if self.session_state != xr::SessionState::READY
            && self.session_state != xr::SessionState::SYNCHRONIZED
            && self.session_state != xr::SessionState::VISIBLE
            && self.session_state != xr::SessionState::FOCUSED
        {
            vulkan.frame(frame_buffer).await;
            return;
        }

        let frame_state = self.frame_waiter.wait().unwrap();

        self.frame_stream.begin().unwrap();

        if !frame_state.should_render {
            self.frame_stream
                .end(frame_state.predicted_display_time, BLEND_MODE, &[])
                .unwrap();
            return;
        }

        let swapchain_index = self.swapchain.acquire_image().unwrap();

        // start writing render commands

        self.swapchain.wait_image(xr::Duration::INFINITE).unwrap();

        let (view_flags, views) = self
            .session
            .locate_views(
                VIEW_CONFIGURATION,
                frame_state.predicted_display_time,
                &self.space,
            )
            .unwrap();

        // set user-interactive matrices (i.e. view matrix) and submit to GPU

        self.swapchain.release_image().unwrap();

        let rect = xr::Rect2Di {
            offset: xr::Offset2Di { x: 0, y: 0 },
            extent: self.swapchain_extent,
        };

        let views = [
            xr::CompositionLayerProjectionView::new()
                .pose(views[0].pose)
                .fov(views[0].fov)
                .sub_image(
                    xr::SwapchainSubImage::new()
                        .swapchain(&self.swapchain)
                        .image_array_index(0)
                        .image_rect(rect),
                ),
            xr::CompositionLayerProjectionView::new()
                .pose(views[1].pose)
                .fov(views[1].fov)
                .sub_image(
                    xr::SwapchainSubImage::new()
                        .swapchain(&self.swapchain)
                        .image_array_index(1)
                        .image_rect(rect),
                ),
        ];

        self.frame_stream
            .end(
                frame_state.predicted_display_time,
                BLEND_MODE,
                &[&xr::CompositionLayerProjection::new()
                    .space(&self.space)
                    .views(&views)],
            )
            .unwrap();
    }

    fn poll_events(&mut self) {
        let mut event_storage = xr::EventDataBuffer::new();
        while let Some(event) = self.instance.poll_event(&mut event_storage).unwrap() {
            use xr::Event::*;
            match event {
                SessionStateChanged(state_change) => {
                    log::info!("OpenXR state changed to {:?}", state_change.state());
                    match state_change.state() {
                        xr::SessionState::IDLE => {
                            assert_eq!(self.session_state, xr::SessionState::UNKNOWN)
                        }
                        xr::SessionState::READY => {
                            self.session.begin(VIEW_CONFIGURATION).unwrap();
                        }
                        xr::SessionState::SYNCHRONIZED => {}
                        xr::SessionState::VISIBLE => {}
                        xr::SessionState::FOCUSED => {}
                        xr::SessionState::STOPPING => {
                            self.session.end().unwrap();
                        }
                        xr::SessionState::LOSS_PENDING => todo!(),
                        xr::SessionState::EXITING => todo!(),
                        _ => unreachable!(),
                    }

                    self.session_state = state_change.state();
                }
                InstanceLossPending(_) => {
                    panic!("OpenXR instance lost");
                }
                EventsLost(events_lost) => {
                    log::warn!("OpenXR lost {} events", events_lost.lost_event_count())
                }
                _ => {}
            }
        }
    }
}
