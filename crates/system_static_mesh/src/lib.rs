use std::{future::Future, mem, pin::Pin};

use event::{EventReader, EventWriter};
use frame_buffer::FrameBufferWriter;
use nalgebra_glm::Vec3;
use system_interfaces::SystemInterfaces;
use task::FixedUpdateTask;
use task_executor::{FixedUpdateExecutor, FixedUpdateTaskHandle};
use update_buffer::UpdateBufferRef;

pub struct System {
    frame_data: FrameUpdateData,
    fixed_data: Option<Pin<Box<FixedUpdateData>>>,
    fixed_update_task_handle: Option<FixedUpdateTaskHandle<FixedUpdateData>>,
}

impl Default for System {
    fn default() -> Self {
        Self {
            frame_data: Default::default(),
            fixed_data: Some(Box::pin(Default::default())),
            fixed_update_task_handle: None,
        }
    }
}

impl System {
    pub async fn update_frame(
        &mut self,
        event_reader: EventReader<'_>,
        event_writer: EventWriter<'_>,
        frame_buffer_writer: FrameBufferWriter,
        system_interfaces: SystemInterfaces<'_>,
    ) {
        self.frame_data
            .update(
                event_reader,
                event_writer,
                frame_buffer_writer,
                system_interfaces,
            )
            .await;
    }

    pub async fn await_fixed(&mut self) {
        if let Some(task_handle) = self.fixed_update_task_handle.take() {
            self.fixed_data = Some(task_handle.await);
        }
    }

    pub fn update_fixed(&mut self, executor: &FixedUpdateExecutor<'_>) {
        let mut fixed = self.fixed_data.take().unwrap();

        fixed.prepare_update(&mut self.frame_data);

        self.fixed_update_task_handle = Some(executor.execute_async(fixed));
    }
}

#[derive(Default)]
struct FrameUpdateData {
    modified_entities: Vec<Vec3>,
}

impl FrameUpdateData {
    async fn update(
        &mut self,
        _event_reader: EventReader<'_>,
        _event_writer: EventWriter<'_>,
        _frame_buffer_writer: FrameBufferWriter,
        _system_interfaces: SystemInterfaces<'_>,
    ) {
        if let Some(entity) = self.modified_entities.first() {
            println!("{:?}", entity);
        } else {
            self.modified_entities.push(Vec3::zeros());
        }
    }
}

#[derive(Default)]
struct FixedUpdateData {
    modified_entities: Vec<Vec3>,
}

impl FixedUpdateData {
    fn prepare_update(&mut self, frame_data: &mut FrameUpdateData) {
        // swap network updates to frame update, and local changes to fixed update thread
        mem::swap(
            &mut self.modified_entities,
            &mut frame_data.modified_entities,
        );
    }
}

impl FixedUpdateTask for FixedUpdateData {
    fn task<'a>(
        mut self: Pin<&'a mut Self>,
        update_buffer: &UpdateBufferRef,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        let update_buffer = update_buffer.clone();

        Box::pin(async move {
            if let Some(entity) = self.modified_entities.first_mut() {
                entity.x += 1.0;
                entity.y += 1.0;
                entity.z += 1.0;
            }

            // placeholder
            update_buffer.read();
        })
    }
}
