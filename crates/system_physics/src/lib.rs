use system_interfaces::physics::Data as SharedData;

pub fn shared_data() -> SharedData {
    Default::default()
}

pub struct FrameData {
    _shared_data: SharedData,
}

impl FrameData {
    pub fn new(shared_data: SharedData) -> Self {
        Self {
            _shared_data: shared_data,
        }
    }

    pub async fn update(&mut self) {}
}

pub struct FixedData {
    _shared_data: SharedData,
}

impl FixedData {
    pub async fn swap(&mut self, _frame_data: &mut FrameData) {}

    pub async fn update(&mut self) {}
}
