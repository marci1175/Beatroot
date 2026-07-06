use std::{
    collections::HashMap,
    num::{NonZero, NonZeroU32},
    sync::Arc,
    time::Duration,
};

use rayon::{ThreadPool, ThreadPoolBuilder};
use rodio::{Player, SampleRate, Source, mixer::Mixer};
use rubato::{Async, SincInterpolationParameters, SincInterpolationType, WindowFunction};

use crate::{audio::pipeline::process_samples, ui::panels::playlist::PlaybackState};

#[derive(Debug, Clone, Copy)]
/// This is for personalizing the sample previewers.
/// Only the most basic functionality, available in the players themselves.
pub struct PlayerPreferences {
    pub speed: f32,
    pub volume: f32,
}

impl Default for PlayerPreferences {
    fn default() -> Self {
        Self {
            speed: 1.0,
            volume: 1.0,
        }
    }
}

pub struct HostInformation {
    pub sample_rate: u32,
    pub channel_count: u16,
}

/// Used for playing back samples easily. This is the simpler form of playing back samples.
#[derive(Clone)]
pub struct SamplePlayer {
    /// The underlying player of the sample
    pub player: Arc<Player>,
    /// Total duration of the sample we are playing back
    pub total_duration: Option<Duration>,
    /// Preferences of this specific player.
    pub preferences: PlayerPreferences,
}

///
/// Used to manage playback in the playlist (timeline) of the application.
/// One buffer instance can only hold the data of one sample.
/// One instance of this buffer has to be pre-processed before acutally being able to play them back.
/// The workflow is as follows:
/// ```-
/// 1. Retrive sample buffer from playlist in chunks.
///         |
///         |
///         V
/// 2. Resample in order to fit the target sample rate.
///         |
///         |
///         V
/// 3. Pre-process with effects chain and or other plugins. (VST2, EQ or other)
///         |
///         |
///         V
/// 4. Apply with mixer fader (volume control + pan)
///         |
///         |
///         V
/// 5. Queue to device output.
/// ```
#[derive(Debug)]
pub struct SampleBuffer {
    /// The raw samples of the buffer.
    samples: Vec<f32>,
    /// The sample rate of the sample.
    sample_rate: u32,
    /// The count of channels present in the sample.
    channels: u16,

    /// This is for the internal iterator trait implementation.
    _iterator_idx: usize,
}

impl Iterator for SampleBuffer {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let result = self.samples.get(self._iterator_idx);

        self._iterator_idx += 1;

        result.copied()
    }
}

impl Source for SampleBuffer {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.samples.len())
    }

    fn channels(&self) -> rodio::ChannelCount {
        NonZero::new(self.channels).unwrap()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        SampleRate::from(NonZeroU32::new(self.sample_rate).unwrap())
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            self.samples.len() as f64 / (self.sample_rate * self.channels as u32) as f64,
        ))
    }
}

/// This represents the main playback manager in the application.
/// It used for playing back the playlist's samples.
/// This handles the main workflow of the raw samples.
pub struct MasterPlaybackThread {
    playback_state: PlaybackState,
    sample_ingest: flume::Sender<Vec<SampleBuffer>>,
    host_mixer: Mixer,
}

impl MasterPlaybackThread {
    pub fn new(host_info: Arc<HostInformation>, host_mixer: Mixer) -> anyhow::Result<Self> {
        // Create a thread pool with the default settings
        // CPU core count equals thread count.
        let worker_thread_pool = ThreadPoolBuilder::new().build()?;

        // Create sample ingest channel, this serves as a way for the main thread to send information to the master playback thread.
        let (sender, receiver) = flume::unbounded::<Vec<SampleBuffer>>();
        let host_mixer_clone = host_mixer.clone();

        // Create a thread for handling incoming samples
        std::thread::spawn(move || {
            let host_mixer = host_mixer_clone.clone();
            let host_info = host_info.clone();

            // Create parameters for the resampler
            let params = SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Cubic,
                oversampling_factor: 256,
                window: WindowFunction::BlackmanHarris2,
            };

            // Create a buffer here so that it gets reused instead of reallocated every iteration.
            let mut processed_sample_buffer = Vec::with_capacity(10);
            
            // Resample input - all inputs could vary in length, however the output length doesnt really matter (input is going to be fixed cuz its easier to implement).
            let mut resamplers: HashMap<u32, Async<f32>> = HashMap::new();

            loop {
                // Listen for an incoming sample packet
                match receiver.recv() {
                    Ok(samples) => {
                        // Handle samples by passing them into the pipeline
                        process_samples(
                            &worker_thread_pool,
                            &samples,
                            host_info.clone(),
                            &params,
                            &mut processed_sample_buffer,
                            &mut resamplers,
                        )
                        .expect("Error occured in master playback thread.");
                    }
                    Err(error) => {
                        // Print the error but we shouldnt stop execution
                        eprintln!("{error}");
                    }
                }
            }
        });

        Ok(Self {
            playback_state: PlaybackState::Stopped,
            sample_ingest: sender,
            host_mixer,
        })
    }
}
