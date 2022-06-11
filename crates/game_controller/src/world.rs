use game_entity::EntityId;

pub struct World {
    entities: Vec<EntityId>,
    _entity_free_list: Vec<EntityId>,
    next_entity_id: EntityId,
    replicable_entity_free_list: Vec<EntityId>,
    next_replicable_entity_id: EntityId,
}

impl Default for World {
    fn default() -> Self {
        Self {
            entities: Vec::new(),
            _entity_free_list: Vec::new(),
            next_entity_id: EntityId::max(),
            replicable_entity_free_list: Vec::new(),
            next_replicable_entity_id: EntityId::min(),
        }
    }
}

impl World {
    /// Creates unique EntityIds starting from EntityId::MAX and shrinking
    pub fn _spawn(&mut self) -> EntityId {
        let entity_id = self._entity_free_list.pop().unwrap_or_else(|| {
            let entity_id = self.next_entity_id;
            self.next_entity_id.decrement();
            assert!(self.next_replicable_entity_id <= self.next_entity_id);
            entity_id
        });

        self.entities.push(entity_id);

        entity_id
    }

    /// Creates unique EntityIds starting from 1 and growing
    pub fn spawn_replicable(&mut self) -> EntityId {
        let entity_id = self.replicable_entity_free_list.pop().unwrap_or_else(|| {
            let entity_id = self.next_replicable_entity_id;
            self.next_replicable_entity_id.increment();
            assert!(self.next_replicable_entity_id <= self.next_entity_id);
            entity_id
        });

        self.entities.push(entity_id);

        entity_id
    }

    pub fn remote_spawn(&mut self, entity_id: EntityId) {
        debug_assert!(!self.entities.contains(&entity_id));
        self.entities.push(entity_id);

        self.replicable_entity_free_list
            .retain(|id| *id != entity_id);

        let next_entity_id = self
            .next_replicable_entity_id
            .get()
            .max(entity_id.get() + 1);

        self.next_replicable_entity_id = EntityId::new(next_entity_id);

        assert!(self.next_replicable_entity_id <= self.next_entity_id);
    }

    pub fn _despawn(&mut self, entity_id: EntityId) {
        debug_assert!(self.entities.contains(&entity_id));
        self.entities.retain(|id| *id != entity_id);

        if entity_id < self.next_entity_id {
            self._entity_free_list.push(entity_id);
        } else {
            debug_assert!(self.next_replicable_entity_id < entity_id);
            self.replicable_entity_free_list.push(entity_id);
        }
    }

    pub fn _contains(&self, entity_id: EntityId) -> bool {
        self.entities.contains(&entity_id)
    }
}
