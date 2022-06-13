use game_entity::EntityId;

pub struct World {
    entities: Vec<EntityId>,
    entity_free_list: Vec<EntityId>,
    next_entity_id: EntityId,
}

impl Default for World {
    fn default() -> Self {
        Self {
            entities: Vec::new(),
            entity_free_list: Vec::new(),
            next_entity_id: EntityId::new(1),
        }
    }
}

impl World {
    pub fn spawn(&mut self) -> EntityId {
        let entity_id = self.entity_free_list.pop().unwrap_or_else(|| {
            let entity_id = self.next_entity_id;
            self.next_entity_id.increment();
            entity_id
        });

        self.entities.push(entity_id);

        entity_id
    }

    pub fn despawn(&mut self, entity_id: EntityId) {
        debug_assert!(self.entities.contains(&entity_id));
        self.entities.retain(|id| *id != entity_id);
        self.entity_free_list.push(entity_id);
    }

    pub fn contains(&self, entity_id: EntityId) -> bool {
        self.entities.contains(&entity_id)
    }
}
