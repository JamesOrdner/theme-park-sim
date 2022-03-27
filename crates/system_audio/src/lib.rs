use std::{cmp::min, f32::consts::PI};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, SampleFormat, SampleRate, Stream, StreamError, SupportedBufferSize,
};
use event::AsyncEventDelegate;
use game_system::FIXED_TIMESTEP;
use ringbuf::{Consumer, Producer, RingBuffer};

#[derive(Default)]
pub struct FrameData {
    play_click: bool,
}

impl FrameData {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn update(&mut self, event_delegate: &AsyncEventDelegate<'_>) {
        self.play_click |= event_delegate
            .input_events()
            .any(|event| matches!(event, event::InputEvent::MouseButton(true)));
    }
}

/// Wrapper to allow cpal::Stream to be Send
struct SendStream(Stream);

/// SAFETY: Android's AAudio isn't threadsafe
#[cfg(not(target_os = "android"))]
unsafe impl Send for SendStream {}

pub struct FixedData {
    sample_rate: u32,
    audio_producer: Producer<f32>,
    play_click: u32,
    _stream: SendStream,
}

fn err_fn(err: StreamError) {
    eprintln!("an error occurred on the output audio stream: {err}");
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
        let stereo_ringbuf_len = ringbuf_len as usize * 2;
        let (audio_producer, audio_consumer) = RingBuffer::new(stereo_ringbuf_len).split();

        let mut audio_player = AudioPlayer::new(audio_consumer);

        let mut config = config.with_sample_rate(SampleRate(sample_rate)).config();
        config.buffer_size = BufferSize::Fixed(buffer_size);
        let stream = device
            .build_output_stream(
                &config,
                move |data, _| audio_player.data_callback(data),
                err_fn,
            )
            .unwrap();

        stream.play().unwrap();

        Self {
            sample_rate,
            audio_producer,
            play_click: 0,
            _stream: SendStream(stream),
        }
    }
}

impl FixedData {
    pub async fn swap(&mut self, frame_data: &mut FrameData) {
        if frame_data.play_click {
            frame_data.play_click = false;
            self.play_click = self.sample_rate / 10;
        }
    }

    pub async fn update(&mut self) {
        for _ in 0..self.audio_producer.remaining() / 2 {
            if self.play_click > 0 {
                self.play_click -= 1;

                let phase = 2.0 * PI * self.play_click as f32 / self.sample_rate as f32;
                let val = (880.0 * phase).sin();
                self.audio_producer.push(val).unwrap();
                self.audio_producer.push(val).unwrap();
            } else {
                self.audio_producer.push(0.0).unwrap();
                self.audio_producer.push(0.0).unwrap();
            }
        }
    }
}

struct AudioPlayer {
    audio_consumer: Consumer<f32>,
}

impl AudioPlayer {
    fn new(audio_consumer: Consumer<f32>) -> Self {
        Self { audio_consumer }
    }

    fn data_callback(&mut self, buffer: &mut [f32]) {
        let num_read = self.audio_consumer.pop_slice(buffer);
        buffer[num_read..].fill(0.0);

        if num_read != buffer.len() {
            log::warn!(
                "audio running {} frames behind",
                (buffer.len() - num_read) / 2
            );
        }
    }
}
