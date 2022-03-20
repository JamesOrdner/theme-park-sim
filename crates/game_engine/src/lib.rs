use event::{EventManager, EventReader, EventWriter};
use frame_buffer::{FrameBuffer, FrameBufferWriter};
use game_controller::GameController;
use game_input::{GameInput, GameInputInterface};
use system_interfaces::SystemInterfaces;
use task_executor::{FixedUpdateExecutor, TaskExecutor};
use update_buffer::UpdateBuffer;
use winit::{event::WindowEvent, window::Window};

#[cfg(target_vendor = "apple")]
use metal::Metal;

#[cfg(not(target_vendor = "apple"))]
use vulkan::Vulkan;

pub struct GameEngine {
    event_manager: EventManager,
    game_controller: GameController,
    input: GameInput,
    systems: Systems,
    task_executor: TaskExecutor,
    update_buffer: UpdateBuffer,

    #[cfg(target_vendor = "apple")]
    graphics: Metal,

    #[cfg(not(target_vendor = "apple"))]
    graphics: Vulkan,
}

impl GameEngine {
    pub fn new(window: &Window) -> Self {
        let thread_count = TaskExecutor::available_parallelism();

        let mut event_manager = EventManager::new(thread_count);
        event_manager.assign_thread_event_buffer(0);

        let input = GameInput::new(window.inner_size());

        let update_buffer = UpdateBuffer::new(thread_count);

        #[cfg(target_vendor = "apple")]
        let graphics = Metal::new(window);

        #[cfg(not(target_vendor = "apple"))]
        let graphics = Vulkan::new(window);

        Self {
            event_manager,
            game_controller: GameController,
            input,
            systems: Systems::new(),
            task_executor: TaskExecutor,
            update_buffer,
            graphics,
        }
    }

    pub fn handle_input(&mut self, event: WindowEvent) {
        // writes to previous frame event buffers
        self.input.handle_input(event);
    }

    pub fn frame(&mut self) {
        // fixed-timestep systems
        for _ in 0..3 {
            {
                let mut await_task = self.systems.await_fixed();
                self.task_executor.execute_blocking(&mut await_task);
            }

            self.update_buffer.swap_buffers();

            let update_buffer = self.update_buffer.borrow();

            let fixed_update_executor = self.task_executor.fixed_update_executor(update_buffer);
            self.systems.update_fixed(&fixed_update_executor);
        }

        let event_reader = self.event_manager.event_reader();
        let event_writer = self.event_manager.event_writer();

        self.input.update(event_writer);
        self.game_controller.update(event_reader);

        self.event_manager.swap_buffers();

        let input_interface = GameInputInterface::new(&self.input);
        let event_reader = self.event_manager.event_reader();
        let event_writer = self.event_manager.event_writer();

        let mut frame_task = async {
            self.systems
                .update_frame(
                    event_reader,
                    event_writer,
                    FrameBufferWriter,
                    input_interface,
                )
                .await;
            self.graphics.frame(&FrameBuffer).await;
        };

        self.task_executor.execute_blocking(&mut frame_task);
    }
}

#[derive(Default)]
struct Systems {
    static_mesh: system_static_mesh::System,
}

impl Systems {
    fn new() -> Self {
        Default::default()
    }

    async fn await_fixed(&mut self) {
        let static_mesh_task = self.static_mesh.await_fixed();

        static_mesh_task.await;
    }

    fn update_fixed(&mut self, task_executor: &FixedUpdateExecutor<'_>) {
        self.static_mesh.update_fixed(task_executor);
    }

    async fn update_frame(
        &mut self,
        event_reader: EventReader<'_>,
        event_writer: EventWriter<'_>,
        frame_buffer_writer: FrameBufferWriter,
        input_interface: GameInputInterface<'_>,
    ) {
        let system_interfaces = SystemInterfaces {
            input: input_interface,
        };

        let static_mesh_task = self.static_mesh.update_frame(
            event_reader,
            event_writer,
            frame_buffer_writer,
            system_interfaces,
        );

        static_mesh_task.await;
    }
}
