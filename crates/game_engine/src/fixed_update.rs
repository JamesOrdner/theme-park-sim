use futures::pin_mut;
use task_executor::{task::parallel, FixedTaskHandle, TaskExecutor};
use update_buffer::UpdateBuffer;

use crate::frame_update::FrameUpdate;

pub struct FixedUpdate {
    systems: Option<Box<FixedUpdateSystems>>,
    task_handle: Option<FixedTaskHandle<FixedUpdateSystems>>,
}

impl FixedUpdate {
    pub fn new(update_buffer: UpdateBuffer) -> Self {
        Self {
            systems: Some(Box::new(FixedUpdateSystems::new(update_buffer))),
            task_handle: None,
        }
    }

    pub async fn await_prev_update(&mut self) {
        if let Some(task_handle) = self.task_handle.take() {
            self.systems = Some(task_handle.await);
        }
    }

    pub async fn swap(&mut self, frame_systems: &mut FrameUpdate) {
        let fixed_systems = self.systems.as_mut().unwrap();

        let audio = fixed_systems.audio.swap(&mut frame_systems.audio);
        let static_mesh = fixed_systems
            .static_mesh
            .swap(&mut frame_systems.static_mesh);

        pin_mut!(audio);
        pin_mut!(static_mesh);

        parallel([audio, static_mesh]).await;
    }

    pub fn execute(&mut self, task_executor: &mut TaskExecutor) {
        let mut fixed_systems = self.systems.take().unwrap();

        let task = async move {
            fixed_systems.update_buffer.swap_buffers();

            let update_buffer = fixed_systems.update_buffer.borrow();

            {
                let audio = fixed_systems.audio.update();
                let network = fixed_systems.network.update(update_buffer);
                let static_mesh = fixed_systems.static_mesh.update(update_buffer);

                pin_mut!(audio);
                pin_mut!(network);
                pin_mut!(static_mesh);

                parallel([audio, network, static_mesh]).await;
            }

            fixed_systems
        };

        self.task_handle = Some(task_executor.execute_fixed(task));
    }
}

struct FixedUpdateSystems {
    update_buffer: UpdateBuffer,
    audio: system_audio::FixedData,
    network: system_network::FixedData,
    static_mesh: system_static_mesh::FixedData,
}

impl FixedUpdateSystems {
    fn new(update_buffer: UpdateBuffer) -> Self {
        Self {
            update_buffer,
            audio: Default::default(),
            network: Default::default(),
            static_mesh: Default::default(),
        }
    }
}
