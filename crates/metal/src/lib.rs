#![cfg(target_vendor = "apple")]

use std::mem;

use anyhow::{Context, Error, Result};
use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use frame_buffer::FrameBufferReader;
use metal::{
    Buffer, CommandQueue, Device, MTLClearColor, MTLLoadAction, MTLPixelFormat, MTLPrimitiveType,
    MTLResourceOptions, MetalLayer, RenderPassDescriptor,
};
use objc::{rc::autoreleasepool, runtime::YES};
use winit::{dpi::PhysicalSize, platform::macos::WindowExtMacOS, window::Window};

use crate::pipeline::Pipeline;

mod pipeline;

pub struct Metal {
    _device: Device,
    layer: MetalLayer,
    queue: CommandQueue,
    vertex_buffer: Buffer,
    pipeline: Pipeline,
}

unsafe impl Send for Metal {}

impl Metal {
    pub fn new(window: &Window) -> Result<Self> {
        autoreleasepool(|| {
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

            let queue = device.new_command_queue();

            let vertex_data = [0.0_f32, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0, -1.0, 0.0];
            let vertex_buffer = device.new_buffer_with_data(
                vertex_data.as_ptr() as *const _,
                mem::size_of_val(&vertex_data) as u64,
                MTLResourceOptions::empty(),
            );

            let pipeline = Pipeline::new("default", &device)
                .context("pipeline creation failed for: default")?;

            Ok(Self {
                _device: device,
                layer,
                queue,
                vertex_buffer,
                pipeline,
            })
        })
    }

    pub fn window_resized(&mut self, size: PhysicalSize<u32>) {
        autoreleasepool(|| {
            self.layer
                .set_drawable_size(CGSize::new(size.width as f64, size.height as f64));
        });
    }

    pub async fn frame(&mut self, frame_buffer: &FrameBufferReader<'_>) {
        autoreleasepool(|| {
            let clear_color = frame_buffer.camera_location().unwrap_or_default();

            let drawable = self.layer.next_drawable().unwrap();

            let descriptor = RenderPassDescriptor::new();

            let color_attachment = descriptor.color_attachments().object_at(0).unwrap();
            color_attachment.set_texture(Some(drawable.texture()));
            color_attachment.set_load_action(MTLLoadAction::Clear);
            color_attachment.set_clear_color(MTLClearColor::new(
                clear_color.x as f64,
                clear_color.y as f64,
                clear_color.z as f64,
                1.0,
            ));

            let cmd_buf = self.queue.new_command_buffer();

            let encoder = cmd_buf.new_render_command_encoder(descriptor);
            encoder.set_render_pipeline_state(&self.pipeline.state);
            encoder.set_vertex_buffer(0, Some(&self.vertex_buffer), 0);
            encoder.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
            encoder.end_encoding();

            cmd_buf.present_drawable(drawable);
            cmd_buf.commit();
        });
    }
}
