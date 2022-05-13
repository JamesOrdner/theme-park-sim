use std::{ops::Deref, sync::Arc};

use anyhow::Result;
use erupt::{vk, DeviceLoader};
use smallvec::SmallVec;

use crate::{swapchain::Swapchain, VulkanInfo};

pub struct RenderPass {
    device: Arc<DeviceLoader>,
    render_pass: vk::RenderPass,
    framebuffers: SmallVec<[vk::Framebuffer; 3]>,
    render_area: vk::Rect2D,
}

impl RenderPass {
    pub fn new(vulkan: &VulkanInfo, swapchain: &Swapchain) -> Result<Self> {
        let color_attachment = vk::AttachmentDescriptionBuilder::new()
            .format(swapchain.surface_format.format)
            .samples(vk::SampleCountFlagBits::_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .stencil_load_op(vk::AttachmentLoadOp::DONT_CARE)
            .stencil_store_op(vk::AttachmentStoreOp::DONT_CARE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR);

        let color_attachment_ref = vk::AttachmentReferenceBuilder::new()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let color_attachments = [color_attachment_ref];

        let subpass = vk::SubpassDescriptionBuilder::new()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachments);

        let attachments = [color_attachment];
        let subpasses = [subpass];

        let create_info = vk::RenderPassCreateInfoBuilder::new()
            .attachments(&attachments)
            .subpasses(&subpasses);

        let render_pass = unsafe {
            vulkan
                .device
                .create_render_pass(&create_info, None)
                .result()?
        };

        let framebuffers = swapchain
            .image_views
            .iter()
            .map(|image_view| {
                let attachments = [*image_view];

                let create_info = vk::FramebufferCreateInfoBuilder::new()
                    .render_pass(render_pass)
                    .attachments(&attachments)
                    .width(swapchain.surface_extent.width)
                    .height(swapchain.surface_extent.height)
                    .layers(1);

                unsafe {
                    vulkan
                        .device
                        .create_framebuffer(&create_info, None)
                        .unwrap()
                }
            })
            .collect();

        let render_area = vk::Rect2D {
            offset: vk::Offset2D::default(),
            extent: swapchain.surface_extent,
        };

        Ok(Self {
            device: vulkan.device.clone_loader(),
            render_pass,
            framebuffers,
            render_area,
        })
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        unsafe {
            for framebuffer in &self.framebuffers {
                self.device.destroy_framebuffer(*framebuffer, None);
            }

            self.device.destroy_render_pass(self.render_pass, None);
        }
    }
}

impl Deref for RenderPass {
    type Target = vk::RenderPass;

    fn deref(&self) -> &Self::Target {
        &self.render_pass
    }
}

impl RenderPass {
    pub fn begin(&self, cmd: vk::CommandBuffer, swapchain_index: usize) {
        let clear_values = [vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [0.0, 0.0, 0.0, 0.0],
            },
        }];

        let begin_info = vk::RenderPassBeginInfoBuilder::new()
            .render_pass(self.render_pass)
            .framebuffer(self.framebuffers[swapchain_index])
            .render_area(self.render_area)
            .clear_values(&clear_values);

        unsafe {
            self.device
                .cmd_begin_render_pass(cmd, &begin_info, vk::SubpassContents::INLINE);
        }
    }

    pub fn end(&self, cmd: vk::CommandBuffer) {
        unsafe {
            self.device.cmd_end_render_pass(cmd);
        }
    }
}
