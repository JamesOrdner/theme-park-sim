use std::{
    cmp::min,
    f32::consts::TAU,
    iter::zip,
    slice,
    sync::{atomic::Ordering, Arc},
};

use atomic_float::AtomicF32;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, FrameCount, SampleFormat, SampleRate, Stream, SupportedBufferSize,
};
use frame_buffer::AsyncFrameBufferDelegate;

const PREFERRED_SAMPLE_RATE: u32 = 48000;
const PREFERRED_BUFFER_LEN: FrameCount = 512;

/// Wrapper to allow cpal::Stream to be Send
struct SendStream(Stream);

// SAFETY: Android's AAudio isn't threadsafe
#[cfg(not(target_os = "android"))]
unsafe impl Send for SendStream {}

#[derive(Default)]
struct SharedAudioData {
    channel_gains: [AtomicF32; 2],
}

pub struct FrameData {
    audio_data: Arc<SharedAudioData>,
    _stream: SendStream,
}

impl Default for FrameData {
    fn default() -> Self {
        let host = cpal::default_host();
        let device = host.default_output_device().expect("no audio device");
        let config = device
            .supported_output_configs()
            .expect("no audio configs")
            .find(|config| {
                matches!(config.sample_format(), SampleFormat::F32) && config.channels() >= 2
            })
            .expect("no supported audio device");

        let sample_rate = min(config.max_sample_rate().0, PREFERRED_SAMPLE_RATE);

        let buffer_size = match config.buffer_size() {
            SupportedBufferSize::Range { min, max } => {
                BufferSize::Fixed(PREFERRED_BUFFER_LEN.clamp(*min, *max))
            }
            SupportedBufferSize::Unknown => BufferSize::Default,
        };

        let mut config = config.with_sample_rate(SampleRate(sample_rate)).config();
        config.channels = 2;
        config.buffer_size = buffer_size;

        let audio_data = Arc::new(SharedAudioData::default());
        let mut audio_player = AudioPlayer::new(audio_data.clone(), sample_rate as f32);

        let stream = device
            .build_output_stream(
                &config,
                move |data, _| audio_player.data_callback(data),
                |err| eprintln!("an error occurred on the output audio stream: {err}"),
            )
            .unwrap();

        stream.play().unwrap();

        Self {
            audio_data,
            _stream: SendStream(stream),
        }
    }
}

impl FrameData {
    pub async fn update(&mut self, frame_buffer: &AsyncFrameBufferDelegate<'_>) {
        let frame_buffer = frame_buffer.reader();
        let camera_info = frame_buffer.camera_info();
        let camera_orientation = (camera_info.focus - camera_info.location).normalize();

        let dist = 0.5_f32.powf(camera_info.location.norm());
        let pan = camera_orientation
            .cross(&camera_info.location.normalize())
            .y;

        self.audio_data.channel_gains[0].store(dist * (pan + 1.0) * 0.5, Ordering::Relaxed);
        self.audio_data.channel_gains[1].store(dist * (-pan + 1.0) * 0.5, Ordering::Relaxed);
    }
}

struct AudioPlayer {
    audio_data: Arc<SharedAudioData>,
    channel_gains: [f32; 2],
    target_channel_gains: [f32; 2],
    sample_rate: f32,
    phase: f32,
}

impl AudioPlayer {
    fn new(audio_data: Arc<SharedAudioData>, sample_rate: f32) -> Self {
        Self {
            audio_data,
            channel_gains: Default::default(),
            target_channel_gains: Default::default(),
            sample_rate,
            phase: 0.0,
        }
    }

    fn data_callback(&mut self, buffer: &mut [f32]) {
        for (local_target, atomic_target) in zip(
            &mut self.target_channel_gains,
            &self.audio_data.channel_gains,
        ) {
            *local_target = atomic_target.load(Ordering::Relaxed);
        }

        let buffer = as_stereo_mut(buffer);

        let phase_delta = 880.0 * TAU / self.sample_rate;

        for frame in buffer {
            for (current, target) in zip(&mut self.channel_gains, &self.target_channel_gains) {
                *current += (*target - *current) * 0.001;
            }

            let val = self.phase.sin();
            self.phase += phase_delta;

            frame[0] = val * self.channel_gains[0];
            frame[1] = val * self.channel_gains[1];
        }
    }
}

fn as_stereo_mut(slice: &mut [f32]) -> &mut [[f32; 2]] {
    debug_assert_eq!(slice.len() % 2, 0);
    let stereo_len = slice.len() / 2;
    // SAFETY: stereo_len * 2 is guaranteed to not exceed original slice len
    unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr().cast(), stereo_len) }
}
