use event::{EventManager, EventReader, EventWriter};
use frame_buffer::{FrameBuffer, FrameBufferWriter};
use game_controller::GameController;
use game_input::{GameInput, GameInputInterface};
use system_interfaces::SystemInterfaces;
use task_executor::TaskExecutor;
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

    #[cfg(target_vendor = "apple")]
    graphics: Metal,

    #[cfg(not(target_vendor = "apple"))]
    graphics: Vulkan,
}

impl GameEngine {
    pub fn new(window: &Window) -> Self {
        let thread_count = TaskExecutor::max_parallelism();

        let mut event_manager = EventManager::new(thread_count);
        event_manager.assign_thread_event_buffer(0);

        #[cfg(target_vendor = "apple")]
        let graphics = Metal::new(window);

        #[cfg(not(target_vendor = "apple"))]
        let graphics = Vulkan::new(window);

        Self {
            event_manager,
            game_controller: GameController,
            input: GameInput::new(),
            systems: Systems::new(),
            task_executor: TaskExecutor,
            graphics,
        }
    }

    pub fn handle_input(&mut self, event: WindowEvent) {
        // writes to previous frame event buffers
        self.input.handle_input(event);
    }

    pub fn frame(&mut self) {
        // fixed-timestep systems
        //   await previous update if time for next update
        //   write event changes
        //   begin processing next update

        let event_reader = self.event_manager.event_reader();
        let event_writer = self.event_manager.event_writer();

        self.input.update(event_writer);
        self.game_controller.update(event_reader);

        self.event_manager.step();

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
    static_mesh: sys_static_mesh::System,
}

impl Systems {
    fn new() -> Self {
        Default::default()
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
