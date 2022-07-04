use std::{mem, slice};

use game_entity::EntityId;
use gpu_compute_data::GuestGpuComputeData;
use metal::{Buffer, Device, MTLResourceOptions};
use nalgebra_glm::{Vec3, Vec4};

pub struct GpuComputeData {
    guest_data: GuestData,
    index: bool,
}

impl GpuComputeData {
    pub(crate) fn new(device: &Device) -> Self {
        Self {
            guest_data: GuestData::new(device),
            index: false,
        }
    }

    pub fn swap(&mut self) {
        self.index = !self.index;
    }

    #[inline]
    pub fn guest(&self) -> Guest {
        Guest {
            guest_data: &self.guest_data,
            index: self.index,
        }
    }
}

pub struct Guest<'a> {
    guest_data: &'a GuestData,
    index: bool,
}

impl Guest<'_> {
    pub(crate) fn frame_updates(&self) -> &Buffer {
        &self.guest_data.frame_updates[self.index as usize]
    }

    pub(crate) fn locations(&self) -> &Buffer {
        &self.guest_data.locations[self.index as usize]
    }

    pub(crate) fn gpu_locations(&self) -> &Buffer {
        &self.guest_data.locations[!self.index as usize]
    }

    pub(crate) fn velocities(&self) -> &Buffer {
        &self.guest_data.velocities
    }

    pub(crate) unsafe fn set_velocity(
        &self,
        update_index: usize,
        instance_index: usize,
        goal: Vec3,
        speed: f32,
    ) {
        assert!(update_index < GUEST_BUFFER_LEN);
        let data = self.frame_updates().contents() as *mut GuestFrameUpdate;
        let update = data.add(update_index).as_mut().unwrap_unchecked();
        let goal = goal.normalize();
        update.velocity = goal * speed;
        update.index = instance_index.try_into().unwrap();
    }
}

impl GuestGpuComputeData for Guest<'_> {
    #[inline]
    fn location(&self, entity_id: EntityId) -> Vec3 {
        let index = entity_id.get() as usize - 1; // TEMP
        let data = self.locations().contents() as *const Vec4;
        let location = unsafe { *data.add(index) };
        location.xyz()
    }
}

const GUEST_BUFFER_LEN: usize = 128;

#[repr(C)]
struct GuestFrameUpdate {
    velocity: Vec3,
    index: u32,
}

struct GuestData {
    frame_updates: [Buffer; 2],
    locations: [Buffer; 2],
    velocities: Buffer,
}

unsafe impl Send for GuestData {}
unsafe impl Sync for GuestData {}

impl GuestData {
    fn new(device: &Device) -> Self {
        let make_updates_buffer = || {
            device.new_buffer(
                (GUEST_BUFFER_LEN * mem::size_of::<GuestFrameUpdate>()) as u64,
                MTLResourceOptions::StorageModeShared
                    | MTLResourceOptions::HazardTrackingModeUntracked,
            )
        };

        let make_locations_buffer = || {
            let locations_buffer = device.new_buffer(
                (GUEST_BUFFER_LEN * mem::size_of::<Vec4>()) as u64,
                MTLResourceOptions::StorageModeShared
                    | MTLResourceOptions::HazardTrackingModeUntracked,
            );

            unsafe {
                let locations_data = slice::from_raw_parts_mut(
                    locations_buffer.contents() as *mut Vec4,
                    GUEST_BUFFER_LEN,
                );
                locations_data.fill(Vec4::zeros());
            }

            locations_buffer
        };

        let velocities = device.new_buffer(
            (GUEST_BUFFER_LEN * mem::size_of::<Vec4>()) as u64,
            MTLResourceOptions::StorageModePrivate
                | MTLResourceOptions::HazardTrackingModeUntracked,
        );

        Self {
            frame_updates: [(); 2].map(|_| make_updates_buffer()),
            locations: [(); 2].map(|_| make_locations_buffer()),
            velocities,
        }
    }
}
