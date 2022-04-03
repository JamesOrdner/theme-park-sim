#![cfg(target_vendor = "apple")]

use std::mem;

use anyhow::{Error, Result};
use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use frame_buffer::FrameBufferReader;
use metal::{
    Device, MTLClearColor, MTLLoadAction, MTLPixelFormat, MetalLayer, RenderPassDescriptor,
};
use objc::{rc::autoreleasepool, runtime::YES};
use winit::{dpi::PhysicalSize, platform::macos::WindowExtMacOS, window::Window};

pub struct Metal {
    device: Device,
    layer: MetalLayer,
}

unsafe impl Send for Metal {}

impl Metal {
    pub fn new(window: &Window) -> Result<Self> {
        let device = Device::system_default().ok_or_else(|| Error::msg("no device found"))?;
        log::info!("Metal device: {}", device.name());

        let layer = MetalLayer::new();
        layer.set_device(&device);
        layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        layer.set_framebuffer_only(true);

        unsafe {
            let view = window.ns_view() as cocoa_id;
            view.setWantsLayer(YES);
            view.setLayer(mem::transmute(layer.as_ref()));
        }

        let size = window.inner_size();
        layer.set_drawable_size(CGSize::new(size.width as f64, size.height as f64));

        Ok(Self { device, layer })
    }

    pub fn window_resized(&mut self, size: PhysicalSize<u32>) {
        self.layer
            .set_drawable_size(CGSize::new(size.width as f64, size.height as f64));
    }

    pub async fn frame(&mut self, _frame_buffer: &FrameBufferReader<'_>) {
        autoreleasepool(|| {
            let drawable = self.layer.next_drawable().unwrap();

            let descriptor = RenderPassDescriptor::new();

            let color_attachment = descriptor.color_attachments().object_at(0).unwrap();
            color_attachment.set_texture(Some(drawable.texture()));
            color_attachment.set_load_action(MTLLoadAction::Clear);
            color_attachment.set_clear_color(MTLClearColor::new(0.0, 0.6, 0.9, 1.0));

            let queue = self.device.new_command_queue();
            let cmd_buf = queue.new_command_buffer();

            let encoder = cmd_buf.new_render_command_encoder(descriptor);
            encoder.end_encoding();

            cmd_buf.present_drawable(drawable);
            cmd_buf.commit();
        });
    }
}
