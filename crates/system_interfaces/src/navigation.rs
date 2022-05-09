use game_data::SharedData;
use nalgebra_glm::Vec3;

pub type Data = SharedData<DataSingle>;

#[derive(Default)]
pub struct DataSingle {}

pub struct Interface {
    data: Data,
}

impl From<Data> for Interface {
    fn from(data: Data) -> Self {
        Self { data }
    }
}

impl Interface {
    pub async fn path(&self, _origin: &Vec3, _dest: &Vec3) -> Option<Vec3> {
        let _ = self.data.read_single().await;
        None
    }
}
