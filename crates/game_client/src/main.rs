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
                    ..
                } => *control_flow = ControlFlow::Exit,
                Event::DeviceEvent { event, .. } => {
                    engine.handle_device_event(event);
                }
                Event::WindowEvent { event, .. } => {
                    engine.handle_window_event(event);
                }
                Event::MainEventsCleared => {
                    engine.frame();
                }
                _ => (),
            }
        });
    }
}
