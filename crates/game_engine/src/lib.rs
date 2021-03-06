use std::time::Instant;

use event::EventManager;
use frame_buffer::FrameBufferManager;
use futures::pin_mut;
use game_controller::GameController;
use game_input::GameInput;
use game_system::FIXED_TIMESTEP;
use system_interfaces::SystemData;
use task_executor::{task::parallel, TaskExecutor};
use update_buffer::UpdateBuffer;
use winit::{
    event::{DeviceEvent, WindowEvent},
    window::Window,
};

use crate::{fixed_update::FixedUpdate, frame_update::FrameUpdate};

mod fixed_update;
mod frame_update;

#[cfg(target_vendor = "apple")]
use metal::Metal;

#[cfg(not(target_vendor = "apple"))]
use vulkan::Vulkan;

pub struct GameEngine {
    task_executor: TaskExecutor,
    event_manager: EventManager,
    frame_update: FrameUpdate,
    fixed_update: FixedUpdate,
    frame_buffer_manager: FrameBufferManager,
    game_controller: GameController,
    input: GameInput,
    last_fixed_update_instant: Instant,
    last_frame_update_instant: Instant,

    #[cfg(target_vendor = "apple")]
    graphics: Metal,

    #[cfg(not(target_vendor = "apple"))]
    graphics: std::mem::ManuallyDrop<Vulkan>,
}

impl GameEngine {
    pub fn new(window: &Window) -> Self {
        let thread_count = TaskExecutor::available_parallelism();

        let event_manager = EventManager::new(thread_count);
        let update_buffer = UpdateBuffer::new(thread_count);
        let frame_buffer_manager = FrameBufferManager::new(thread_count);

        let task_executor = TaskExecutor::new(thread_count, &|thread_index| {
            event_manager.assign_thread_event_buffer(thread_index);
            update_buffer.assign_thread_event_buffer(thread_index);
            frame_buffer_manager.assign_thread_frame_buffer(thread_index);
        });

        let system_data = system_data();
        let frame_update = FrameUpdate::new(&system_data, window);
        let fixed_update = FixedUpdate::new(update_buffer);
        let game_controller = GameController::new(system_data.physics.into());

        let input = GameInput::new(window.inner_size());

        #[cfg(target_vendor = "apple")]
        let graphics = Metal::new(window).unwrap();

        #[cfg(not(target_vendor = "apple"))]
        let graphics = std::mem::ManuallyDrop::new(Vulkan::new(window).unwrap());

        Self {
            task_executor,
            event_manager,
            frame_update,
            fixed_update,
            frame_buffer_manager,
            game_controller,
            input,
            last_fixed_update_instant: Instant::now(),
            last_frame_update_instant: Instant::now(),
            graphics,
        }
    }
}

fn system_data() -> SystemData {
    SystemData {
        navigation: system_navigation::shared_data(),
        physics: system_physics::shared_data(),
        static_mesh: system_static_mesh::shared_data(),
    }
}

#[cfg(not(target_vendor = "apple"))]
impl Drop for GameEngine {
    fn drop(&mut self) {
        let graphics = unsafe { std::mem::ManuallyDrop::take(&mut self.graphics) };
        graphics.destroy();
    }
}

impl GameEngine {
    pub fn handle_device_event(&mut self, event: DeviceEvent) {
        self.input.handle_raw_input(event);
    }

    pub fn handle_window_event(&mut self, event: WindowEvent) {
        if let WindowEvent::Resized(size) = event {
            self.frame_update
                .camera
                .window_resized(size.width, size.height);
            self.graphics.window_resized(size);
        }

        self.input.handle_input(event);
    }

    pub fn frame(&mut self) {
        let now = Instant::now();
        let delta_time = now
            .duration_since(self.last_frame_update_instant)
            .as_secs_f32();
        self.last_frame_update_instant = now;

        self.event_manager.swap();
        self.frame_buffer_manager.swap();

        self.update_fixed();

        self.input.update(&mut self.event_manager.sync_delegate());

        self.update_sync_systems(delta_time);

        self.update_game_state();

        self.event_manager.clear_system_game_events();

        self.update_and_render_frame(delta_time);
    }

    fn update_fixed(&mut self) {
        let now = Instant::now();

        while now.duration_since(self.last_fixed_update_instant) >= FIXED_TIMESTEP {
            self.last_fixed_update_instant += FIXED_TIMESTEP;

            // ensure previous update is complete
            {
                let await_task = self.fixed_update.await_prev_update();
                pin_mut!(await_task);
                self.task_executor.execute_blocking(await_task);
            }

            // if last iteration, swap with frame updates
            if now.duration_since(self.last_fixed_update_instant) < FIXED_TIMESTEP {
                let swap_task = self.fixed_update.swap(&mut self.frame_update);
                pin_mut!(swap_task);
                self.task_executor.execute_blocking(swap_task);
            }

            self.fixed_update.execute(&mut self.task_executor);
        }
    }

    fn update_sync_systems(&mut self, delta_time: f32) {
        let event_delegate = self.event_manager.sync_delegate();
        let mut frame_buffer_delegate = self.frame_buffer_manager.sync_delegate();

        self.frame_update
            .update_sync(&event_delegate, &mut frame_buffer_delegate, delta_time);
    }

    fn update_game_state(&mut self) {
        let mut event_delegate = self.event_manager.sync_delegate();
        let mut frame_buffer = self.frame_buffer_manager.sync_delegate();

        self.game_controller.update(
            &mut event_delegate,
            &mut frame_buffer,
            self.input.interface(),
            self.frame_update.camera.interface(),
        );
    }

    fn update_and_render_frame(&mut self, delta_time: f32) {
        let frame_buffer_delegate = self.frame_buffer_manager.async_delegate();
        let frame_buffer_reader = frame_buffer_delegate.reader();
        let event_delegate = self.event_manager.async_delegate();

        let frame_task = async {
            let frame_update_task =
                self.frame_update
                    .update_async(&event_delegate, &frame_buffer_delegate, delta_time);

            let graphics_task = self.graphics.frame(&frame_buffer_reader);

            pin_mut!(frame_update_task);
            pin_mut!(graphics_task);

            parallel([frame_update_task, graphics_task]).await;
        };

        pin_mut!(frame_task);
        self.task_executor.execute_blocking(frame_task);
    }
}
