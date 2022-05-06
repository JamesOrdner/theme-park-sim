use std::{cmp::min, f32::consts::TAU, slice};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, SampleFormat, SampleRate, Stream, SupportedBufferSize,
};
use event::{AsyncEventDelegate, FrameEvent};
use game_system::FIXED_TIMESTEP;
use nalgebra_glm::Vec3;
use ringbuf::{Consumer, Producer, RingBuffer};

#[derive(Default)]
pub struct FrameData {
    camera_location: Vec3,
    camera_orientation: Vec3,
}

impl FrameData {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn update(&mut self, event_delegate: &AsyncEventDelegate<'_>) {
        for event in event_delegate.frame_events() {
            match event {
                FrameEvent::CameraLocation(location) => self.camera_location = *location,
                FrameEvent::CameraOrientation(orientation) => {
                    self.camera_orientation = *orientation
                }
                _ => {}
            }
        }
    }
}

/// Wrapper to allow cpal::Stream to be Send
struct SendStream(Stream);

// SAFETY: Android's AAudio isn't threadsafe
#[cfg(not(target_os = "android"))]
unsafe impl Send for SendStream {}

pub struct FixedData {
    sample_rate: u32,
    audio_producer: Producer<[f32; 2]>,
    phase: f32,
    swap_index: bool,
    camera_location: [Vec3; 2],
    camera_orientation: [Vec3; 2],
    _stream: SendStream,
}

impl Default for FixedData {
    fn default() -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().expect("no audio device");
        let config = device
            .supported_output_configs()
            .expect("no audio configs")
            .find(|config| {
                matches!(config.sample_format(), SampleFormat::F32) && config.channels() == 2
            })
            .expect("no supported audio device");

        let sample_rate = min(config.max_sample_rate().0, 48000);
        let buffer_size = match config.buffer_size() {
            SupportedBufferSize::Range { min, max } => 2048.clamp(*min, *max),
            _ => panic!("unable to determine audio buffer size"),
        };

        // we always want to have FIXED_TIMESTEP * buffer_size ready to play
        let ringbuf_len = FIXED_TIMESTEP.as_millis() as u32 * sample_rate / 1000 + buffer_size;
        let (audio_producer, audio_consumer) = RingBuffer::new(ringbuf_len as usize).split();

        let mut audio_player = AudioPlayer::new(audio_consumer);

        let mut config = config.with_sample_rate(SampleRate(sample_rate)).config();
        config.buffer_size = BufferSize::Fixed(buffer_size);
        let stream = device
            .build_output_stream(
                &config,
                move |data, _| audio_player.data_callback(data),
                |err| eprintln!("an error occurred on the output audio stream: {err}"),
            )
            .unwrap();

        stream.play().unwrap();

        Self {
            sample_rate,
            audio_producer,
            phase: 0.0,
            swap_index: false,
            camera_location: Default::default(),
            camera_orientation: Default::default(),
            _stream: SendStream(stream),
        }
    }
}

impl FixedData {
    pub async fn swap(&mut self, frame_data: &mut FrameData) {
        self.swap_index = !self.swap_index;

        let index = self.swap_index as usize;
        self.camera_location[index] = frame_data.camera_location;
        self.camera_orientation[index] = frame_data.camera_orientation;
    }

    pub async fn update(&mut self) {
        let index_start = !self.swap_index as usize;
        let index_end = self.swap_index as usize;

        let gain = [index_start, index_end].map(|i| {
            let dist = 0.5_f32.powf(self.camera_location[i].norm());
            let pan = self.camera_orientation[i]
                .cross(&self.camera_location[i].normalize())
                .y;

            [dist * (pan + 1.0) * 0.5, dist * (-pan + 1.0) * 0.5]
        });

        let phase_delta = 880.0 * TAU / self.sample_rate as f32;
        let num_frames = self.audio_producer.remaining();

        for i in 0..num_frames {
            let alpha = i as f32 / num_frames as f32;
            let val = self.phase.sin();
            let frame = [0, 1]
                .map(|i| gain[0][i] * (1.0 - alpha) + gain[1][i] * alpha)
                .map(|gain| val * gain);
            self.audio_producer.push(frame).unwrap();

            self.phase += phase_delta;
        }

        self.phase %= TAU;
    }
}

struct AudioPlayer {
    audio_consumer: Consumer<[f32; 2]>,
}

impl AudioPlayer {
    fn new(audio_consumer: Consumer<[f32; 2]>) -> Self {
        Self { audio_consumer }
    }

    fn data_callback(&mut self, buffer: &mut [f32]) {
        let buffer = as_chunks_mut(buffer);

        let num_read = self.audio_consumer.pop_slice(buffer);
        buffer[num_read..].fill([0.0; 2]);

        if num_read != buffer.len() {
            log::warn!("audio running {} frames behind", buffer.len() - num_read);
        }
    }
}

fn as_chunks_mut(slice: &mut [f32]) -> &mut [[f32; 2]] {
    debug_assert_eq!(slice.len() % 2, 0);
    let stereo_len = slice.len() / 2;
    // SAFETY: stereo_len * 2 is guaranteed to not exceed original slice len
    unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr().cast(), stereo_len) }
}
