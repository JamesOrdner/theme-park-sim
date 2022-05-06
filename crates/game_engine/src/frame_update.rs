use event::AsyncEventDelegate;
use frame_buffer::FrameBufferWriter;
use futures::pin_mut;
use system_interfaces::SystemData;
use task_executor::parallel;

pub struct FrameUpdate {
    pub audio: system_audio::FrameData,
    pub camera: system_camera::FrameData,
    pub static_mesh: system_static_mesh::FrameData,
}

impl FrameUpdate {
    pub fn new(system_data: &SystemData) -> Self {
        Self {
            audio: Default::default(),
            camera: system_camera::FrameData::new(system_data.static_mesh.clone().into()),
            static_mesh: system_static_mesh::FrameData::new(system_data.static_mesh.clone()),
        }
    }

    pub async fn update(
        &mut self,
        event_delegate: &AsyncEventDelegate<'_>,
        frame_buffer: &FrameBufferWriter<'_>,
        delta_time: f32,
    ) {
        let audio_task = self.audio.update(event_delegate);
        let camera_task = self.camera.update(event_delegate, frame_buffer, delta_time);
        let static_mesh_task = self.static_mesh.update(event_delegate, frame_buffer);

        pin_mut!(audio_task);
        pin_mut!(camera_task);
        pin_mut!(static_mesh_task);

        parallel([audio_task, camera_task, static_mesh_task]).await;
    }
}
