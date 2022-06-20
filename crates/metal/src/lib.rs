#![cfg(target_vendor = "apple")]

use std::{collections::HashMap, mem, slice};

use anyhow::{Context, Error, Result};
use cocoa::{appkit::NSView, base::id as cocoa_id};
use compute_pipeline::ComputePipeline;
use core_graphics_types::geometry::CGSize;
use frame_buffer::FrameBufferReader;
use game_entity::EntityId;
use metal::{
    Buffer, CommandQueue, Device, MTLClearColor, MTLDispatchType, MTLIndexType, MTLLoadAction,
    MTLPixelFormat, MTLPrimitiveType, MTLResourceOptions, MTLSize, MetalLayer, NSRange, NSUInteger,
    RenderPassDescriptor,
};
use nalgebra_glm::{look_at_lh, perspective_lh_zo, translate, Mat4, Vec3, Vec4};
use objc::{rc::autoreleasepool, runtime::YES};
use winit::{dpi::PhysicalSize, platform::macos::WindowExtMacOS, window::Window};

use crate::pipeline::Pipeline;

mod compute_pipeline;
mod pipeline;

struct StaticMesh {
    buffer: Buffer,
    locations_offset: NSUInteger,
    location: Vec3,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GuestData {
    loc: Vec4,
    goal: Vec3,
    speed: f32,
}

impl Default for GuestData {
    fn default() -> Self {
        Self {
            loc: Vec4::from([0.0, 0.0, 0.0, 1.0]),
            goal: Vec3::zeros(),
            speed: 0.0,
        }
    }
}

pub struct Metal {
    device: Device,
    layer: MetalLayer,
    queue: CommandQueue,
    pipeline: Pipeline,
    compute_pipeline: ComputePipeline,
    aspect: f32,
    static_meshes: HashMap<EntityId, StaticMesh>,
    guests: Vec<(EntityId, StaticMesh)>,
    guests_buffer: Buffer,
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

            let compute_pipeline = ComputePipeline::new("guests", &device)
                .context("pipeline creation failed for: guests")?;

            let aspect = size.width as f32 / size.height as f32;

            let guests_buffer = device.new_buffer(
                100 * mem::size_of::<GuestData>() as u64,
                MTLResourceOptions::StorageModeManaged,
            );

            unsafe {
                let guests_data =
                    std::slice::from_raw_parts_mut(guests_buffer.contents() as *mut GuestData, 100);
                guests_data.fill(Default::default());
                guests_buffer
                    .did_modify_range(NSRange::new(0, 100 * mem::size_of::<GuestData>() as u64));
            }

            Ok(Self {
                device,
                layer,
                queue,
                pipeline,
                compute_pipeline,
                aspect,
                static_meshes: HashMap::new(),
                guests: Vec::new(),
                guests_buffer,
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

    pub async fn frame(&mut self, frame_buffer: &FrameBufferReader<'_>, delta_time: f32) {
        for (old_id, new_id) in frame_buffer.updated_entity_ids() {
            let static_mesh = self.static_meshes.remove(old_id).unwrap();
            self.static_meshes.insert(*new_id, static_mesh);
        }

        for entity_id in frame_buffer.despawned() {
            self.static_meshes.remove(entity_id);
        }

        for entity_id in frame_buffer.spawned_guests() {
            let static_mesh = self.spawn_static_mesh();
            self.guests.push((*entity_id, static_mesh));
        }

        for spawned in frame_buffer.spawned_static_meshes() {
            let static_mesh = self.spawn_static_mesh();
            self.static_meshes.insert(spawned.entity_id, static_mesh);
        }

        for (entity_id, location) in frame_buffer.locations() {
            if let Some(static_mesh) = self.static_meshes.get_mut(&entity_id) {
                static_mesh.location = *location;
            }
        }

        for (entity_id, location, speed) in frame_buffer.guest_goals() {
            if let Some((i, _)) = self
                .guests
                .iter_mut()
                .enumerate()
                .find(|(_, (eid, _))| eid == entity_id)
            {
                let guests_data = unsafe {
                    std::slice::from_raw_parts_mut(
                        self.guests_buffer.contents() as *mut GuestData,
                        100,
                    )
                };

                let guest = &mut guests_data[i];
                guest.goal = *location;
                guest.speed = *speed;

                self.guests_buffer.did_modify_range(NSRange::new(
                    (i * mem::size_of::<GuestData>()) as u64,
                    mem::size_of::<GuestData>() as u64,
                ));
            }
        }

        if !self.guests.is_empty() {
            autoreleasepool(|| {
                let cmd_buf = self.queue.new_command_buffer();
                let encoder =
                    cmd_buf.compute_command_encoder_with_dispatch_type(MTLDispatchType::Serial);
                encoder.set_compute_pipeline_state(&self.compute_pipeline.state);
                encoder.set_bytes(
                    0,
                    mem::size_of::<f32>() as u64,
                    &delta_time as *const _ as *const _,
                );
                encoder.set_buffer(1, Some(&self.guests_buffer), 0);
                encoder.dispatch_threads(
                    MTLSize::new(self.guests.len() as u64, 1, 1),
                    MTLSize::new(1, 1, 1),
                );
                encoder.end_encoding();
                cmd_buf.commit();
                cmd_buf.wait_until_completed();
            });
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

                encoder.set_vertex_buffer(3, Some(&self.guests_buffer), 0);

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

            if !self.guests.is_empty() {
                encoder.set_render_pipeline_state(&self.pipeline.state);
                encoder.set_vertex_bytes(
                    1,
                    mem::size_of_val(&proj_view) as u64,
                    &proj_view as *const _ as *const _,
                );

                encoder.set_vertex_buffer(3, Some(&self.guests_buffer), 0);

                for (i, (_, static_mesh)) in self.guests.iter().enumerate() {
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
                    let i = i as u16;
                    encoder.set_vertex_bytes(
                        4,
                        mem::size_of::<u16>() as u64,
                        &i as *const _ as *const _,
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

    fn spawn_static_mesh(&mut self) -> StaticMesh {
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

        StaticMesh {
            buffer,
            locations_offset: locations_offset as u64,
            location: Vec3::zeros(),
        }
    }
}
