use std::{ops::Deref, sync::Arc};

use anyhow::Result;
use erupt::{vk, DeviceLoader, ExtendableFrom};
use smallvec::SmallVec;

use crate::{swapchain::Swapchain, VulkanInfo};

pub struct VrRenderPassCreateInfo<'a> {
    pub surface_extent: vk::Extent2D,
    pub image_format: vk::Format,
    pub image_views: &'a [vk::ImageView],
}

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

    pub fn new_vr(vulkan: &VulkanInfo, vr_info: &VrRenderPassCreateInfo) -> Result<Self> {
        let color_attachment = vk::AttachmentDescriptionBuilder::new()
            .format(vr_info.image_format)
            .samples(vk::SampleCountFlagBits::_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let color_attachment_ref = vk::AttachmentReferenceBuilder::new()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL);

        let color_attachments = [color_attachment_ref];

        let subpass = vk::SubpassDescriptionBuilder::new()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachments);

        let attachments = [color_attachment];
        let subpasses = [subpass];

        let dependencies = [vk::SubpassDependencyBuilder::new()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(vk::AccessFlags::COLOR_ATTACHMENT_WRITE)];

        let view_masks = [0b11];
        let mut multiview_create_info = vk::RenderPassMultiviewCreateInfoBuilder::new()
            .view_masks(&view_masks)
            .correlation_masks(&view_masks);

        let create_info = vk::RenderPassCreateInfoBuilder::new()
            .attachments(&attachments)
            .subpasses(&subpasses)
            .dependencies(&dependencies)
            .extend_from(&mut multiview_create_info);

        let render_pass = unsafe {
            vulkan
                .device
                .create_render_pass(&create_info, None)
                .result()?
        };

        let framebuffers = vr_info
            .image_views
            .iter()
            .map(|image_view| {
                let attachments = [*image_view];

                let create_info = vk::FramebufferCreateInfoBuilder::new()
                    .render_pass(render_pass)
                    .attachments(&attachments)
                    .width(vr_info.surface_extent.width)
                    .height(vr_info.surface_extent.height)
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
            extent: vr_info.surface_extent,
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
