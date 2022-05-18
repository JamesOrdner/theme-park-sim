use update_buffer::UpdateBufferRef;

pub enum Role {
    Client,
    Server,
}

#[derive(Default)]
pub struct FixedData {}

impl FixedData {
    pub fn set_role(&mut self, _role: Role) {}

    pub async fn update(&mut self, _update_buffer: UpdateBufferRef<'_>) {
        // take data from static mesh/other systems and update network

        // update static mesh/other systems from network
    }
}
