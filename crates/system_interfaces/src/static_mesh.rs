use game_data::SharedData;
use game_entity::{EntityId, EntityMap};
use nalgebra_glm::Vec3;

pub type Data = SharedData<DataSingle>;

#[derive(Default)]
pub struct DataSingle {
    pub locations: EntityMap<Vec3>,
}

pub struct Interface {
    data: Data,
}

impl From<Data> for Interface {
    fn from(data: Data) -> Self {
        Self { data }
    }
}

impl Interface {
    pub async fn location(&self, entity_id: EntityId) -> Option<Vec3> {
        let data = self.data.read_single().await;
        data.locations.get(entity_id).copied()
    }
}
