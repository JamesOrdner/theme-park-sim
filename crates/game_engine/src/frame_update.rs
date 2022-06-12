use event::{AsyncEventDelegate, SyncEventDelegate};
use frame_buffer::{AsyncFrameBufferDelegate, SyncFrameBufferDelegate};
use futures::pin_mut;
use system_interfaces::SystemData;
use task_executor::task::parallel;
use winit::window::Window;

pub struct FrameUpdate {
    pub audio: system_audio::FrameData,
    pub camera: system_camera::FrameData,
    pub navigation: system_navigation::FrameData,
    pub network: system_network::FrameData,
    pub static_mesh: system_static_mesh::FrameData,
}

impl FrameUpdate {
    pub fn new(system_data: &SystemData, window: &Window) -> Self {
        let size = window.inner_size();

        let camera = system_camera::FrameData::new(
            size.width,
            size.height,
            system_data.physics.clone().into(),
        );
        let navigation = system_navigation::FrameData::new(
            system_data.navigation.clone(),
            system_data.static_mesh.clone().into(),
        );
        let static_mesh = system_static_mesh::FrameData::new(system_data.static_mesh.clone());

        Self {
            audio: Default::default(),
            camera,
            navigation,
            network: Default::default(),
            static_mesh,
        }
    }

    /// Update systems which must update synchronously, before the game state update
    pub fn update_sync(
        &mut self,
        event_delegate: &SyncEventDelegate<'_>,
        frame_buffer: &mut SyncFrameBufferDelegate<'_>,
        delta_time: f32,
    ) {
        self.camera.update(event_delegate, frame_buffer, delta_time);
    }

    /// Update systems which may update asynchronously, in parallel with frame rendering
    pub async fn update_async(
        &mut self,
        event_delegate: &AsyncEventDelegate<'_>,
        frame_buffer: &AsyncFrameBufferDelegate<'_>,
    ) {
        let audio = self.audio.update(frame_buffer);
        let navigation = self.navigation.update(event_delegate);
        let network = self.network.update(event_delegate);
        let static_mesh = self.static_mesh.update(event_delegate, frame_buffer);

        pin_mut!(audio);
        pin_mut!(navigation);
        pin_mut!(network);
        pin_mut!(static_mesh);

        parallel([audio, navigation, network, static_mesh]).await;
    }
}
