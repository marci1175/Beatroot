use std::{
    num::{NonZero, NonZeroU32},
    sync::{Arc, atomic::AtomicU64},
    time::{Duration, Instant},
};

use dashmap::DashMap;
use parking_lot::Mutex;
use rayon::ThreadPoolBuilder;
use rodio::{Player, SampleRate, Source, mixer::Mixer};
use rubato::{
    Async, SincInterpolationParameters, SincInterpolationType, WindowFunction,
    audioadapter::Adapter,
};

use crate::{
    audio::pipeline::process_samples,
    ui::{
        fx_chain::NodeMap,
        panels::playlist::{PlaybackState, Position},
    },
};

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

#[derive(Debug, Clone, Copy)]
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
///
/// All samples are interleaved by default.
#[derive(Debug, Clone)]
pub struct SampleBuffer {
    /// The raw samples of the buffer.
    samples: Vec<f32>,
    /// The sample rate of the sample.
    sample_rate: u32,
    /// The count of channels present in the sample.
    channels: u16,

    /// The id of the node that this sample is coming from. (The nodes which are present in the playlist.)
    /// This is going to be useful when looking up what effects to apply to this sample.
    origin_id: usize,

    /// This is for the internal iterator trait implementation.
    _iterator_idx: usize,
}

unsafe impl Adapter<'_, f32> for SampleBuffer {
    unsafe fn read_sample_unchecked(&self, channel: usize, frame: usize) -> f32 {
        let idx = frame * self.channels as usize + channel;

        *unsafe { self.samples.get_unchecked(idx) }
    }

    fn channels(&self) -> usize {
        self.channels as usize
    }

    fn frames(&self) -> usize {
        self.samples.len() / self.channels as usize
    }
}

impl SampleBuffer {
    pub fn new(samples: Vec<f32>, origin_id: usize, sample_rate: u32, channels: u16) -> Self {
        Self {
            samples,
            sample_rate,
            origin_id,
            channels,
            _iterator_idx: 0,
        }
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn origin_id(&self) -> usize {
        self.origin_id
    }
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

/// Wrapper around the type NodeMap.
/// Key is the position of the sample and its index in that position (Every key is forced to have a unique key due to how vectors work.), nodemap is the effects chain to the sample.
pub type FXMap = Arc<DashMap<(Position, usize), NodeMap>>;

/// Time since starting the playback in nanos.
/// When paused this field stops being updated.
pub static GLOBAL_PLAYBACK_TIMER: AtomicU64 = AtomicU64::new(0);

/// This represents the main playback manager in the application.
/// It used for playing back the playlist's samples.
/// This handles the main workflow of the raw samples.
pub struct MasterPlaybackThread {
    /// Main playback state. This controls to entire playback thread.
    playback_state: Arc<PlaybackState>,

    playback_start_ts: Instant,

    /// Samples are provided from a set amount of tracks (cpu core count) in pre-determined buffer sizes.
    /// For example the samples are ingested from every 10 tracks. So we have to ingest those 10 tracks worth of samples before moving on to the 2nd set of 10 and so forth.
    /// If there are less than 10 tracks available the remainder of worker threads will be idle.
    sample_ingest: flume::Sender<Vec<SampleBuffer>>,

    /// Mixer handle of the host. This is used to append samples to the host's output.
    host_mixer: Mixer,

    /// Contains each sample's effects that should be applied to them.
    /// They key is the original identifier for that specific node in the playlist.
    /// When reading the samples from the playlist each sample read from a different node will have a different id.
    /// This id is generated in the ui and later supplied to this map, if the user wants to apply any effects to that specific node.
    /// The audio thread will look up the sample's origin id - fetch the effects and their order and run the effects on the samples.
    fx_map: FXMap,
}

impl MasterPlaybackThread {
    pub fn new(host_info: HostInformation, host_mixer: Mixer) -> anyhow::Result<Self> {
        // Create a thread pool with the default settings
        // CPU core count equals thread count.
        let worker_thread_pool = ThreadPoolBuilder::new().build()?;

        // Create sample ingest channel, this serves as a way for the main thread to send information to the master playback thread.
        let (sender, receiver) = flume::unbounded::<Vec<SampleBuffer>>();
        let host_mixer_clone = host_mixer.clone();

        // Create a map of effects which the samples will be applied with.
        let fx_map: Arc<DashMap<(Position, usize), NodeMap>> = Arc::new(DashMap::new());
        let fx_map_clone = fx_map.clone();

        // Create a thread for handling incoming samples
        std::thread::spawn(move || {
            let host_mixer = host_mixer_clone.clone();
            let host_info = host_info;
            let effects_map = fx_map_clone.clone();

            // Create parameters for the resampler
            let params = SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: SincInterpolationType::Cubic,
                oversampling_factor: 256,
                window: WindowFunction::BlackmanHarris2,
            };

            // Create a buffer here so that it gets reused instead of reallocated every iteration.
            let mut processed_sample_buffer =
                Vec::with_capacity(worker_thread_pool.current_num_threads());

            // Resample input - all inputs could vary in length, however the output length doesnt really matter (input is going to be fixed cuz its easier to implement).
            let resamplers: Arc<DashMap<u32, Mutex<Async<f32>>>> = Arc::new(DashMap::new());

            loop {
                // Listen for an incoming sample packet
                match receiver.recv() {
                    Ok(samples) => {
                        // Handle samples by passing them into the pipeline
                        process_samples(
                            &worker_thread_pool,
                            samples,
                            host_info,
                            &params,
                            &mut processed_sample_buffer,
                            resamplers.clone(),
                        )
                        .expect("Error occured in master playback thread.");
                    }
                    Err(error) => {
                        // Print the error but we shouldnt stop execution
                        eprintln!("Error in Master Playback Thread: {error}");
                    }
                }
            }
        });

        Ok(Self {
            playback_state: PlaybackState::Stopped.into(),
            sample_ingest: sender,
            host_mixer,
            fx_map,
            playback_start_ts: Instant::now(),
        })
    }

    pub fn fx_map(&self) -> Arc<DashMap<(Position, usize), NodeMap>> {
        self.fx_map.clone()
    }
}
