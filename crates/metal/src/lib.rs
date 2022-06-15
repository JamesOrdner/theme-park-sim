#![cfg(target_vendor = "apple")]

use std::{collections::HashMap, mem, slice};

use anyhow::{Context, Error, Result};
use cocoa::{appkit::NSView, base::id as cocoa_id};
use core_graphics_types::geometry::CGSize;
use frame_buffer::FrameBufferReader;
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
        for (old_id, new_id) in frame_buffer.updated_entity_ids() {
            let static_mesh = self.static_meshes.remove(old_id).unwrap();
            self.static_meshes.insert(*new_id, static_mesh);
        }

        for entity_id in frame_buffer.despawned() {
            self.static_meshes.remove(entity_id);
        }

        for entity_id in frame_buffer.spawned_guests() {
            self.spawn_static_mesh(*entity_id);
        }

        for static_mesh in frame_buffer.spawned_static_meshes() {
            self.spawn_static_mesh(static_mesh.entity_id);
        }

        for (entity_id, location) in frame_buffer.locations() {
            if let Some(static_mesh) = self.static_meshes.get_mut(&entity_id) {
                static_mesh.location = *location;
            }
        }

        #[repr(C)]
        #[allow(unused)]
        struct ProjView {
            proj: Mat4,
            view: Mat4,
        }

        let proj_view = {
            let camera_info = frame_buffer.camera_info();

            let proj = perspective_lh_zo(
                self.aspect,
                camera_info.fov,
                camera_info.near_plane,
                camera_info.far_plane,
            );

            let view = look_at_lh(&camera_info.location, &camera_info.focus, &camera_info.up);

            ProjView { proj, view }
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

            if !self.static_meshes.is_empty() {
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
                        3,
                        MTLIndexType::UInt16,
                        &static_mesh.buffer,
                        0,
                    );
                }
            }

            encoder.end_encoding();
            cmd_buf.present_drawable(drawable);
            cmd_buf.commit();
        });
    }

    fn spawn_static_mesh(&mut self, entity_id: EntityId) {
        let indices = [0_u16, 1, 2];
        let vertex_data = [0.0_f32, 0.0, 1.0, -1.0, 0.0, -1.0, 1.0, 0.0, -1.0];

        // align vertex locations to 16 bytes
        let locations_offset = (mem::size_of_val(&indices) + 15) & !15;
        let size = (locations_offset + mem::size_of_val(&vertex_data)) as u64;

        let buffer = self
            .device
            .new_buffer(size, MTLResourceOptions::StorageModeManaged);

        unsafe {
            let data = buffer.contents();

            let indices_slice = slice::from_raw_parts_mut(data as *mut u16, indices.len());
            indices_slice.copy_from_slice(&indices);

            let data = (data as *mut u8).add(locations_offset);
            let vertex_slice = slice::from_raw_parts_mut(data as *mut f32, vertex_data.len());
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

        self.static_meshes.insert(entity_id, static_mesh);
    }
}
