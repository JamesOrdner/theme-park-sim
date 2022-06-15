use event::{AsyncEventDelegate, FrameEvent};
use system_interfaces::{
    navigation::Data as SharedData, static_mesh::Interface as StaticMeshInterface,
};
use task_executor::async_task::{execute_async, AsyncTaskHandle};

pub fn shared_data() -> SharedData {
    Default::default()
}

pub struct FrameData {
    task_data: Option<TaskData>,
    task_handle: Option<AsyncTaskHandle<TaskData>>,
}

struct TaskData {
    _shared_data: SharedData,
    _static_mesh_interface: StaticMeshInterface,
}

impl FrameData {
    pub fn new(shared_data: SharedData, static_mesh_interface: StaticMeshInterface) -> Self {
        let task_data = TaskData {
            _shared_data: shared_data,
            _static_mesh_interface: static_mesh_interface,
        };

        Self {
            task_data: Some(task_data),
            task_handle: None,
        }
    }

    pub async fn update(&mut self, event_delegate: &AsyncEventDelegate<'_>) {
        // TODO: do as part of fixed update

        // check completion of rebuild task
        if let Some(task_handle) = self.task_handle.take() {
            match task_handle.result() {
                Ok(task_data) => self.task_data = Some(task_data),
                Err(task_handle) => self.task_handle = Some(task_handle),
            }
        }

        if event_delegate
            .frame_events()
            .any(|event| matches!(event, FrameEvent::Location(_)))
        {
            let mut task_data = self.task_data.take().unwrap();

            let task = async move {
                rebuild_navmesh(&mut task_data);
                task_data
            };

            self.task_handle = Some(execute_async(task));
        }
    }
}

fn rebuild_navmesh(_task_data: &mut TaskData) {}
