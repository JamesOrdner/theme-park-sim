use event::EventManager;
use frame_buffer::{FrameBuffer, FrameBufferWriter};
use game_controller::GameController;
use game_input::{GameInput, GameInputInterface};
use task_executor::TaskExecutor;
use winit::{event::WindowEvent, window::Window};

use crate::{fixed_update::FixedUpdate, frame_update::FrameUpdateSystems};

mod fixed_update;
mod frame_update;

#[cfg(target_vendor = "apple")]
use metal::Metal;

#[cfg(not(target_vendor = "apple"))]
use vulkan::Vulkan;

pub struct GameEngine {
    event_manager: EventManager,
    fixed_update: FixedUpdate,
    frame_update_systems: FrameUpdateSystems,
    game_controller: GameController,
    input: GameInput,
    task_executor: TaskExecutor,

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

        #[cfg(target_vendor = "apple")]
        let graphics = Metal::new(window);

        #[cfg(not(target_vendor = "apple"))]
        let graphics = Vulkan::new(window);

        Self {
            event_manager,
            fixed_update: FixedUpdate::new(thread_count),
            frame_update_systems: FrameUpdateSystems::new(),
            game_controller: GameController,
            input,
            task_executor: TaskExecutor,
            graphics,
        }
    }

    pub fn handle_input(&mut self, event: WindowEvent) {
        // writes to previous frame event buffers
        self.input.handle_input(event);
    }

    pub fn frame(&mut self) {
        const NUM_FIXED_UPDATES: usize = 1;
        for i in 0..NUM_FIXED_UPDATES {
            {
                let mut await_task = self.fixed_update.await_prev_update();
                self.task_executor.execute_blocking(&mut await_task);
            }

            // if last iteration, swap with frame updates
            if i == NUM_FIXED_UPDATES - 1 {
                self.fixed_update.swap(&mut self.frame_update_systems);
            }

            self.fixed_update.execute(&mut self.task_executor);
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
            self.frame_update_systems
                .update(
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
