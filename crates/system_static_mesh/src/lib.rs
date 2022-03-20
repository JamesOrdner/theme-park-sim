use std::{future::Future, mem, pin::Pin};

use event::{EventReader, EventWriter};
use frame_buffer::FrameBufferWriter;
use nalgebra_glm::Vec3;
use system_interfaces::SystemInterfaces;
use task::FixedUpdateTask;
use task_executor::{FixedTaskExecutor, FixedUpdateTaskHandle};

#[derive(Default)]
pub struct System {
    frame: FrameUpdateData,
    fixed_task_handle: Option<FixedUpdateTaskHandle<FixedUpdateData>>,
}

impl System {
    pub async fn update_frame(
        &mut self,
        event_reader: EventReader<'_>,
        event_writer: EventWriter<'_>,
        frame_buffer_writer: FrameBufferWriter,
        system_interfaces: SystemInterfaces<'_>,
    ) {
        self.frame
            .update(
                event_reader,
                event_writer,
                frame_buffer_writer,
                system_interfaces,
            )
            .await;
    }

    pub async fn update_fixed(&mut self, executor: &FixedTaskExecutor) {
        let mut fixed_data = match self.fixed_task_handle.take() {
            Some(task_handle) => task_handle.await,
            None => Box::pin(Default::default()),
        };

        fixed_data.prepare_update(&mut self.frame);

        self.fixed_task_handle = Some(executor.execute_async(fixed_data));
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
        self.modified_entities.push(Vec3::zeros());
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
    fn task<'a>(mut self: Pin<&'a mut Self>) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let Some(entity) = self.modified_entities.first_mut() {
                entity.x += 1.0;
                entity.y += 1.0;
                entity.z += 1.0;
                println!("{:?}", entity);
            }
        })
    }
}
