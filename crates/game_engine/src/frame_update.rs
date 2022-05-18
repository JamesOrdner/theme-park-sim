use event::AsyncEventDelegate;
use frame_buffer::AsyncFrameBufferDelegate;
use futures::pin_mut;
use system_interfaces::SystemData;
use task_executor::task::parallel;

pub struct FrameUpdate {
    pub audio: system_audio::FrameData,
    pub camera: system_camera::FrameData,
    pub navigation: system_navigation::FrameData,
    pub static_mesh: system_static_mesh::FrameData,
}

impl FrameUpdate {
    pub fn new(system_data: &SystemData) -> Self {
        let camera = system_camera::FrameData::new(system_data.static_mesh.clone().into());
        let navigation = system_navigation::FrameData::new(
            system_data.navigation.clone(),
            system_data.static_mesh.clone().into(),
        );
        let static_mesh = system_static_mesh::FrameData::new(system_data.static_mesh.clone());

        Self {
            audio: Default::default(),
            camera,
            navigation,
            static_mesh,
        }
    }

    pub async fn update(
        &mut self,
        event_delegate: &AsyncEventDelegate<'_>,
        frame_buffer: &AsyncFrameBufferDelegate<'_>,
        delta_time: f32,
    ) {
        let audio = self.audio.update(frame_buffer);
        let camera = self.camera.update(event_delegate, frame_buffer, delta_time);
        let navigation = self.navigation.update(event_delegate);
        let static_mesh = self.static_mesh.update(event_delegate, frame_buffer);

        pin_mut!(audio);
        pin_mut!(camera);
        pin_mut!(navigation);
        pin_mut!(static_mesh);

        parallel([audio, camera, navigation, static_mesh]).await;
    }
}
