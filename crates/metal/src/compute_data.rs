use std::{mem, slice};

use game_entity::EntityId;
use gpu_compute_data::GuestGpuComputeData;
use metal::{Buffer, Device, MTLResourceOptions, NSRange};
use nalgebra_glm::{Vec3, Vec4};

pub struct GpuComputeData {
    guest_data: [GuestData; 2],
    index: bool,
}

impl GpuComputeData {
    pub(crate) fn new(device: &Device) -> Self {
        Self {
            guest_data: [GuestData::new(device), GuestData::new(device)],
            index: false,
        }
    }

    pub fn swap(&mut self) {
        self.index = !self.index;
    }

    pub fn guest(&self) -> Guest {
        Guest {
            guest_data: &self.guest_data,
            index: self.index,
        }
    }
}

pub struct Guest<'a> {
    guest_data: &'a [GuestData; 2],
    index: bool,
}

impl Guest<'_> {
    pub(crate) fn locations(&self) -> &Buffer {
        &self.guest_data[self.index as usize].locations_buffer
    }

    pub(crate) fn host_velocities_mut(&self) -> &Buffer {
        &self.guest_data[self.index as usize].movement_buffer
    }

    pub(crate) fn gpu_velocities(&self) -> &Buffer {
        &self.guest_data[!self.index as usize].movement_buffer
    }

    pub(crate) fn gpu_locations_mut(&self) -> &Buffer {
        &self.guest_data[!self.index as usize].locations_buffer
    }

    pub(crate) unsafe fn set_velocity(&self, index: usize, goal: Vec3, speed: f32) {
        let data = self.host_velocities_mut().contents() as *mut Vec4;
        let movement = data.add(index).as_mut().unwrap_unchecked();
        let goal = goal.normalize();
        movement.x = goal.x * speed;
        movement.y = goal.y * speed;
        movement.z = goal.z * speed;

        self.host_velocities_mut().did_modify_range(NSRange::new(
            (index * mem::size_of::<Vec4>()) as u64,
            mem::size_of::<Vec4>() as u64,
        ));

        // TEMP

        let data = self.gpu_velocities().contents() as *mut Vec4;
        let movement = data.add(index).as_mut().unwrap_unchecked();
        movement.x = goal.x * speed;
        movement.y = goal.y * speed;
        movement.z = goal.z * speed;

        self.gpu_velocities().did_modify_range(NSRange::new(
            (index * mem::size_of::<Vec4>()) as u64,
            mem::size_of::<Vec4>() as u64,
        ));
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

struct GuestData {
    locations_buffer: Buffer,
    movement_buffer: Buffer,
}

unsafe impl Send for GuestData {}
unsafe impl Sync for GuestData {}

impl GuestData {
    fn new(device: &Device) -> Self {
        let locations_buffer = device.new_buffer(
            (GUEST_BUFFER_LEN * mem::size_of::<Vec4>()) as u64,
            MTLResourceOptions::StorageModeManaged,
        );

        unsafe {
            let locations_data = slice::from_raw_parts_mut(
                locations_buffer.contents() as *mut Vec4,
                GUEST_BUFFER_LEN,
            );
            locations_data.fill(Vec4::zeros());
        }

        locations_buffer.did_modify_range(NSRange::new(
            0,
            (GUEST_BUFFER_LEN * mem::size_of::<Vec4>()) as u64,
        ));

        let movement_buffer = device.new_buffer(
            (GUEST_BUFFER_LEN * mem::size_of::<Vec4>()) as u64,
            MTLResourceOptions::StorageModeManaged,
        );

        unsafe {
            let movement_data = slice::from_raw_parts_mut(
                movement_buffer.contents() as *mut Vec4,
                GUEST_BUFFER_LEN,
            );
            movement_data.fill(Default::default());
        }

        movement_buffer.did_modify_range(NSRange::new(
            0,
            (GUEST_BUFFER_LEN * mem::size_of::<Vec4>()) as u64,
        ));

        Self {
            locations_buffer,
            movement_buffer,
        }
    }
}
