use event::AsyncEventDelegate;
use frame_buffer::FrameBufferWriter;
use futures::pin_mut;
use game_input::GameInputInterface;
use system_interfaces::SystemInterfaces;
use task_executor::parallel;

#[derive(Default)]
pub struct FrameUpdateSystems {
    pub audio: system_audio::FrameData,
    pub camera: system_camera::FrameData,
    pub static_mesh: system_static_mesh::FrameData,
}

impl FrameUpdateSystems {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn update(
        &mut self,
        event_delegate: &AsyncEventDelegate<'_>,
        frame_buffer: &FrameBufferWriter<'_>,
        input_interface: GameInputInterface<'_>,
        delta_time: f32,
    ) {
        let system_interfaces = SystemInterfaces {
            input: input_interface,
        };

        let audio_task = self.audio.update(event_delegate);
        let camera_task = self.camera.update(event_delegate, frame_buffer, delta_time);
        let static_mesh_task =
            self.static_mesh
                .update(event_delegate, frame_buffer, system_interfaces);

        pin_mut!(audio_task);
        pin_mut!(camera_task);
        pin_mut!(static_mesh_task);

        parallel([audio_task, camera_task, static_mesh_task]).await;
    }
}
