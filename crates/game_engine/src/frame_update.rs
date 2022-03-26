use event::EventDelegate;
use frame_buffer::FrameBufferWriter;
use game_input::GameInputInterface;
use system_interfaces::SystemInterfaces;

#[derive(Default)]
pub struct FrameUpdateSystems {
    pub static_mesh: system_static_mesh::FrameData,
}

impl FrameUpdateSystems {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn update(
        &mut self,
        event_delegate: EventDelegate<'_>,
        frame_buffer_writer: FrameBufferWriter,
        input_interface: GameInputInterface<'_>,
    ) {
        let system_interfaces = SystemInterfaces {
            input: input_interface,
        };

        let static_mesh_task =
            self.static_mesh
                .update(event_delegate, frame_buffer_writer, system_interfaces);

        static_mesh_task.await;
    }
}
