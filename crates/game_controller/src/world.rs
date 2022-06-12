use game_entity::EntityId;

pub struct World {
    entities: Vec<EntityId>,
    entity_free_list: Vec<EntityId>,
    next_entity_id: EntityId,
    replicable_entity_free_list: Vec<EntityId>,
    next_replicable_entity_id: EntityId,
}

impl Default for World {
    fn default() -> Self {
        Self {
            entities: Vec::new(),
            entity_free_list: Vec::new(),
            next_entity_id: EntityId::max(),
            replicable_entity_free_list: Vec::new(),
            next_replicable_entity_id: EntityId::min(),
        }
    }
}

impl World {
    /// Creates unique EntityIds starting from EntityId::MAX and shrinking
    pub fn spawn(&mut self) -> EntityId {
        let entity_id = self.entity_free_list.pop().unwrap_or_else(|| {
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

    pub fn despawn(&mut self, entity_id: EntityId) {
        debug_assert!(self.entities.contains(&entity_id));
        self.entities.retain(|id| *id != entity_id);

        if entity_id < self.next_entity_id {
            self.entity_free_list.push(entity_id);
        } else {
            debug_assert!(self.next_replicable_entity_id < entity_id);
            self.replicable_entity_free_list.push(entity_id);
        }
    }

    /// Moves a locally-spawned EntityId to a replicable EntityId.
    pub fn local_to_replicable(&mut self, local_id: EntityId, replicable_id: EntityId) {
        self.entity_free_list.push(local_id);

        self.replicable_entity_free_list
            .retain(|id| *id != replicable_id);

        let next_entity_id = self
            .next_replicable_entity_id
            .get()
            .max(replicable_id.get() + 1);

        self.next_replicable_entity_id = EntityId::new(next_entity_id);
    }

    pub fn _contains(&self, entity_id: EntityId) -> bool {
        self.entities.contains(&entity_id)
    }
}
