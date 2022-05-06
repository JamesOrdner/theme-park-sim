use game_data::SharedData;
use game_entity::EntityId;
use nalgebra_glm::Vec3;

pub type Data = SharedData<DataSingle>;

#[derive(Default)]
pub struct DataSingle {
    pub locations: Vec<Vec3>,
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
    pub async fn location(&self, _entity_id: EntityId) -> Option<Vec3> {
        let data = self.data.read_single().await;
        data.locations.first().copied()
    }

    pub async fn raycast(&self, _origin: &Vec3, _direction: &Vec3) -> Option<Vec3> {
        let _ = self.data.read_single().await;
        None
    }
}
