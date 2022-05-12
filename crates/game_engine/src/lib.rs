use std::time::Instant;

use event::EventManager;
use frame_buffer::FrameBufferManager;
use futures::pin_mut;
use game_controller::GameController;
use game_input::GameInput;
use game_system::FIXED_TIMESTEP;
use game_vr::GameVr;
use system_interfaces::SystemData;
use task_executor::{task::parallel, TaskExecutor};
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
    vr: Option<GameVr>,

    #[cfg(target_vendor = "apple")]
    graphics: Metal,

    #[cfg(not(target_vendor = "apple"))]
    graphics: Vulkan,
}

impl GameEngine {
    pub fn new(window: &Window) -> Self {
        let thread_count = TaskExecutor::available_parallelism();

        let event_manager = EventManager::new(thread_count);

        let frame_buffer_manager = FrameBufferManager::new(thread_count);

        let input = GameInput::new(window.inner_size());

        let task_executor = TaskExecutor::new(thread_count, &|thread_index| {
            event_manager.assign_thread_event_buffer(thread_index);
            frame_buffer_manager.assign_thread_frame_buffer(thread_index);
        });

        #[cfg(target_vendor = "apple")]
        let (vr, graphics) = (None, Metal::new(window).unwrap());

        #[cfg(not(target_vendor = "apple"))]
        let (vr, graphics) = if let Ok((vr, graphics)) = GameVr::new(window) {
            (Some(vr), graphics)
        } else {
            (None, Vulkan::new(window).unwrap())
        };

        let system_data = system_data();

        Self {
            task_executor,
            event_manager,
            frame_update: FrameUpdate::new(&system_data),
            fixed_update: FixedUpdate::new(thread_count),
            frame_buffer_manager,
            game_controller: GameController::default(),
            input,
            last_fixed_update_instant: Instant::now(),
            last_frame_update_instant: Instant::now(),
            vr,
            graphics,
        }
    }
}

fn system_data() -> SystemData {
    SystemData {
        navigation: system_navigation::shared_data(),
        static_mesh: system_static_mesh::shared_data(),
    }
}

impl GameEngine {
    pub fn handle_device_event(&mut self, event: DeviceEvent) {
        self.input.handle_raw_input(event);
    }

    pub fn handle_window_event(&mut self, event: WindowEvent) {
        if let WindowEvent::Resized(size) = event {
            self.graphics.window_resized(size);
        }

        self.input.handle_input(event);
    }

    pub fn frame(&mut self) {
        self.event_manager.swap();
        self.frame_buffer_manager.swap();

        self.update_fixed();

        self.update_game_state();

        self.update_and_render_frame();
    }

    fn update_fixed(&mut self) {
        let now = Instant::now();

        while now.duration_since(self.last_fixed_update_instant) >= FIXED_TIMESTEP {
            self.last_fixed_update_instant += FIXED_TIMESTEP;

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

    fn update_game_state(&mut self) {
        let mut event_delegate = self.event_manager.sync_delegate();
        let mut frame_buffer_delegate = self.frame_buffer_manager.sync_delegate();

        self.input.update(&mut event_delegate);
        self.game_controller
            .update(&mut event_delegate, &mut frame_buffer_delegate);
    }

    fn update_and_render_frame(&mut self) {
        let frame_buffer_delegate = self.frame_buffer_manager.async_delegate();
        let frame_buffer_reader = frame_buffer_delegate.reader();
        let frame_buffer_writer = frame_buffer_delegate.writer();
        let event_delegate = self.event_manager.async_delegate();

        let now = Instant::now();
        let delta_time = now
            .duration_since(self.last_frame_update_instant)
            .as_secs_f32();
        self.last_frame_update_instant = now;

        let frame_update_task =
            self.frame_update
                .update(&event_delegate, &frame_buffer_writer, delta_time);

        if let Some(vr) = &mut self.vr {
            let frame_task = async {
                let graphics_task = vr.frame(&mut self.graphics);

                pin_mut!(frame_update_task);
                pin_mut!(graphics_task);

                parallel([frame_update_task, graphics_task]).await;
            };

            pin_mut!(frame_task);
            self.task_executor.execute_blocking(frame_task);
        } else {
            let frame_task = async {
                let graphics_task = self.graphics.frame(&frame_buffer_reader);

                pin_mut!(frame_update_task);
                pin_mut!(graphics_task);

                parallel([frame_update_task, graphics_task]).await;
            };

            pin_mut!(frame_task);
            self.task_executor.execute_blocking(frame_task);
        };
    }
}
