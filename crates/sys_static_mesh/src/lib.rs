use event::{EventReader, EventWriter};
use frame_buffer::FrameBufferWriter;
use system_interfaces::SystemInterfaces;

#[derive(Default)]
pub struct System {
    frame_data: FrameData,
}

impl System {
    pub async fn update_frame(
        &mut self,
        event_reader: EventReader<'_>,
        event_writer: EventWriter<'_>,
        frame_buffer_writer: FrameBufferWriter,
        system_interfaces: SystemInterfaces<'_>,
    ) {
        self.frame_data
            .update(
                event_reader,
                event_writer,
                frame_buffer_writer,
                system_interfaces,
            )
            .await;
    }
}

#[derive(Default)]
struct FrameData;

impl FrameData {
    async fn update(
        &mut self,
        _event_reader: EventReader<'_>,
        _event_writer: EventWriter<'_>,
        _frame_buffer_writer: FrameBufferWriter,
        _system_interfaces: SystemInterfaces<'_>,
    ) {
    }
}
