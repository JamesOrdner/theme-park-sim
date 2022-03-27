use game_engine::GameEngine;
use log::LevelFilter;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    GameClient::start();
}

pub struct GameClient;

impl GameClient {
    pub fn start() {
        #[cfg(debug_assertions)]
        env_logger::builder().filter_level(LevelFilter::Info).init();

        #[cfg(not(debug_assertions))]
        env_logger::builder().filter_level(LevelFilter::Warn).init();

        let event_loop = EventLoop::new();
        let window = WindowBuilder::new().build(&event_loop).unwrap();

        let mut engine = GameEngine::new(&window);

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            match event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    window_id: _,
                } => {
                    // no need to check window_id, we only have a single window
                    *control_flow = ControlFlow::Exit
                }
                Event::WindowEvent {
                    event,
                    window_id: _,
                } => {
                    engine.handle_input(event);
                }
                Event::MainEventsCleared => {
                    engine.frame();
                }
                _ => (),
            }
        });
    }
}
