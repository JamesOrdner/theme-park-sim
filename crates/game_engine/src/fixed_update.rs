use std::num::NonZeroUsize;

use task_executor::{FixedUpdateTaskHandle, TaskExecutor};
use update_buffer::UpdateBuffer;

use crate::frame_update::FrameUpdateSystems;

pub struct FixedUpdate {
    systems: Option<Box<FixedUpdateSystems>>,
    task_handle: Option<FixedUpdateTaskHandle<FixedUpdateSystems>>,
}

impl FixedUpdate {
    pub fn new(thread_count: NonZeroUsize) -> Self {
        Self {
            systems: Some(Box::new(FixedUpdateSystems::new(thread_count))),
            task_handle: None,
        }
    }

    pub async fn await_prev_update(&mut self) {
        if let Some(task_handle) = self.task_handle.take() {
            self.systems = Some(task_handle.await);
        }
    }

    pub async fn swap(&mut self, frame_systems: &mut FrameUpdateSystems) {
        let fixed_systems = self.systems.as_mut().unwrap();

        let static_mesh_task = fixed_systems
            .static_mesh
            .swap(&mut frame_systems.static_mesh);

        static_mesh_task.await;
    }

    pub fn execute(&mut self, task_executor: &mut TaskExecutor) {
        let mut fixed_systems = self.systems.take().unwrap();

        let task = async move {
            fixed_systems.update_buffer.swap_buffers();

            let update_buffer = fixed_systems.update_buffer.borrow();

            let static_mesh_task = fixed_systems.static_mesh.update(update_buffer);

            static_mesh_task.await;

            fixed_systems
        };

        self.task_handle = Some(task_executor.execute_async(task));
    }
}

struct FixedUpdateSystems {
    update_buffer: UpdateBuffer,
    static_mesh: system_static_mesh::FixedData,
}

impl FixedUpdateSystems {
    fn new(thread_count: NonZeroUsize) -> Self {
        Self {
            update_buffer: UpdateBuffer::new(thread_count),
            static_mesh: Default::default(),
        }
    }
}
