use event::{GameEvent, InputEvent, SyncEventDelegate};
use game_entity::EntityId;

pub struct GameController;

impl GameController {
    pub fn update(&mut self, event_delegate: &mut SyncEventDelegate) {
        let spawn_object = event_delegate
            .input_events()
            .any(|event| matches!(event, InputEvent::MouseButton(true)));

        if spawn_object {
            event_delegate.push_game_event(GameEvent::Spawn(EntityId::new(1)));
        }
    }
}
