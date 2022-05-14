#![cfg(target_vendor = "apple")]

use std::{collections::HashMap, iter::zip, mem, slice};

use anyhow::{Context, Error, Result};
use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use frame_buffer::{FrameBufferReader, SpawnedStaticMesh};
use game_entity::EntityId;
use metal::{
    Buffer, CommandQueue, Device, MTLClearColor, MTLIndexType, MTLLoadAction, MTLPixelFormat,
    MTLPrimitiveType, MTLResourceOptions, MetalLayer, NSRange, NSUInteger, RenderPassDescriptor,
};
use nalgebra_glm::{look_at_lh, perspective_lh_zo, translate, Mat4, Vec3};
use objc::{rc::autoreleasepool, runtime::YES};
use winit::{dpi::PhysicalSize, platform::macos::WindowExtMacOS, window::Window};

use crate::pipeline::Pipeline;

mod pipeline;

struct StaticMesh {
    buffer: Buffer,
    locations_offset: NSUInteger,
    location: Vec3,
}

pub struct Metal {
    device: Device,
    layer: MetalLayer,
    queue: CommandQueue,
    pipeline: Pipeline,
    aspect: f32,
    static_meshes: HashMap<EntityId, StaticMesh>,
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

            let pipeline = Pipeline::new("default", &device)
                .context("pipeline creation failed for: default")?;

            let aspect = size.width as f32 / size.height as f32;

            Ok(Self {
                device,
                layer,
                queue,
                pipeline,
                aspect,
                static_meshes: HashMap::new(),
            })
        })
    }

    pub fn window_resized(&mut self, size: PhysicalSize<u32>) {
        autoreleasepool(|| {
            self.layer
                .set_drawable_size(CGSize::new(size.width as f64, size.height as f64));
        });

        self.aspect = size.width as f32 / size.height as f32;
    }

    pub async fn frame(&mut self, frame_buffer: &FrameBufferReader<'_>) {
        for static_mesh in frame_buffer.spawned_static_meshes() {
            self.spawn_static_mesh(static_mesh);
        }

        for (mesh, new_location) in zip(self.static_meshes.values_mut(), frame_buffer.locations()) {
            mesh.location = *new_location;
        }

        struct ProjView {
            _proj: Mat4,
            _view: Mat4,
        }

        let proj_view = ProjView {
            _proj: perspective_lh_zo(self.aspect, 1.0, 0.01, 50.0),
            _view: frame_buffer
                .camera_info()
                .map(|info| look_at_lh(&info.location, &info.focus, &info.up))
                .unwrap_or_else(Mat4::identity),
        };

        autoreleasepool(|| {
            let drawable = self.layer.next_drawable().unwrap();

            let descriptor = RenderPassDescriptor::new();

            let color_attachment = descriptor.color_attachments().object_at(0).unwrap();
            color_attachment.set_texture(Some(drawable.texture()));
            color_attachment.set_load_action(MTLLoadAction::Clear);
            color_attachment.set_clear_color(MTLClearColor::new(0.0, 0.0, 0.0, 1.0));

            let cmd_buf = self.queue.new_command_buffer();

            let encoder = cmd_buf.new_render_command_encoder(descriptor);
            encoder.set_render_pipeline_state(&self.pipeline.state);
            encoder.set_vertex_bytes(
                1,
                mem::size_of_val(&proj_view) as u64,
                &proj_view as *const _ as *const _,
            );

            for static_mesh in self.static_meshes.values() {
                let model = translate(&Mat4::identity(), &static_mesh.location);
                encoder.set_vertex_bytes(
                    2,
                    mem::size_of_val(&model) as u64,
                    &model as *const _ as *const _,
                );
                encoder.set_vertex_buffer(
                    0,
                    Some(&static_mesh.buffer),
                    static_mesh.locations_offset,
                );
                encoder.draw_indexed_primitives(
                    MTLPrimitiveType::Triangle,
                    static_mesh.locations_offset / 2,
                    MTLIndexType::UInt16,
                    &static_mesh.buffer,
                    0,
                );
            }

            encoder.end_encoding();

            cmd_buf.present_drawable(drawable);
            cmd_buf.commit();
        });
    }

    fn spawn_static_mesh(&mut self, static_mesh_info: &SpawnedStaticMesh) {
        let indices = [0_u16, 1, 2];
        let vertex_data = [0.0_f32, 0.0, 1.0, -1.0, 0.0, -1.0, 1.0, 0.0, -1.0];

        let locations_offset = mem::size_of_val(&indices);
        let size = (locations_offset + mem::size_of_val(&vertex_data)) as u64;

        let buffer = self
            .device
            .new_buffer(size, MTLResourceOptions::StorageModeManaged);

        unsafe {
            let data = buffer.contents();

            let indices_slice = slice::from_raw_parts_mut(data as *mut u16, indices.len());
            indices_slice.copy_from_slice(&indices);

            let vertex_ptr = (data as *mut u16).add(indices.len());
            let vertex_slice = slice::from_raw_parts_mut(vertex_ptr as *mut f32, vertex_data.len());
            vertex_slice.copy_from_slice(&vertex_data);
        }

        buffer.did_modify_range(NSRange {
            location: 0,
            length: size,
        });

        let static_mesh = StaticMesh {
            buffer,
            locations_offset: locations_offset as u64,
            location: Vec3::zeros(),
        };

        self.static_meshes
            .insert(static_mesh_info.entity_id, static_mesh);
    }
}
