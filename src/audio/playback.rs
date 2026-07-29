///
/// Decides how big should the buffers of the playback thread be and how large of a sample should the ingest thread send.
/// This number is given in milliseconds so ensure that we are calculating with the right metric later.
///  
pub const PLAYBACK_BUFFER_LEN_MS: usize = 50;

use std::{
    num::{NonZero, NonZeroU32},
    sync::{Arc, atomic::AtomicU64},
    time::{Duration, Instant},
};

use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use rayon::ThreadPoolBuilder;
use rodio::{Player, SampleRate, Source, mixer::Mixer, queue::queue};
use rubato::{
    Async, SincInterpolationParameters, SincInterpolationType, WindowFunction,
    audioadapter::Adapter,
};

use crate::{
    audio::{
        host::{HOST_STATE, Tracked},
        pipeline::{mix_samples, process_samples},
    },
    plugins::PluginManager,
    ui::{fx_map::NodeMap, panels::playlist::PlaybackState},
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
    ///
    ///  NOTICE: We can use 0 if we want to ignore this field.
    ///
    origin_id: usize,

    /// This is for the internal iterator trait implementation.
    _iterator_idx: usize,

    /// This field is for minimizing allocations in the main playback thread. It can be ignored otherwise.
    recycle_to: Option<flume::Sender<Vec<f32>>>,
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
            recycle_to: None,
        }
    }

    pub fn with_recycler(mut self, sender: flume::Sender<Vec<f32>>) -> Self {
        self.recycle_to = Some(sender);
        self
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

    /// Deinterleaves this buffer's samples into one `Vec<f32>` per channel.
    pub fn deinterleave(&self) -> Vec<Vec<f32>> {
        let channels = self.channels as usize;

        let frames = self.samples.len() / channels;
        let mut planar: Vec<Vec<f32>> = (0..channels)
            .map(|_| Vec::with_capacity(frames))
            .collect();

        for frame in self.samples.chunks_exact(channels) {
            for (ch, &s) in frame.iter().enumerate() {
                planar[ch].push(s);
            }
        }

        planar
    }

    pub fn replace_from_planar(&mut self, planar: &[Vec<f32>]) {
        let channels = planar.len();
        let frames = planar.get(0).map_or(0, |c| c.len());

        let mut samples = Vec::with_capacity(frames * channels);
        let mut iters: Vec<_> = planar.iter().map(|c| c.iter()).collect();

        for _ in 0..frames {
            for it in iters.iter_mut() {
                samples.push(*it.next().unwrap());
            }
        }

        self.samples = samples;
    }
}

impl Drop for SampleBuffer {
    fn drop(&mut self) {
        if let Some(sender) = self.recycle_to.take() {
            let mut buf = std::mem::take(&mut self.samples);

            // drops contents, keeps allocated capacity
            buf.clear();

            // non-blocking — if pool's full, buffer's just dropped
            let _ = sender.try_send(buf);
        }
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
/// Key is the unique id of the sample, value is the effects chain to the sample (nodemap for easier user management).
pub type FXMap = Arc<DashMap<usize, NodeMap>>;

/// Time since starting the playback in nanos.
/// When paused this field stops being updated.
pub static GLOBAL_PLAYBACK_TIMER: AtomicU64 = AtomicU64::new(0);

pub struct SampleIngestManager {
    /// Samples are provided from a set amount of tracks (cpu core count) in pre-determined buffer sizes.
    /// For example the samples are ingested from every 10 tracks. So we have to ingest those 10 tracks worth of samples before moving on to the 2nd set of 10 and so forth.
    /// If there are less than 10 tracks available the remainder of worker threads will be idle.
    pub ingest_channel: flume::Sender<Vec<SampleBuffer>>,
}

/// This represents the main playback manager in the application.
/// It used for playing back the playlist's samples.
/// This handles the main workflow of the raw samples.
pub struct MasterPlaybackThread {
    /// Contains everything that an ingest thread needs to manage the samples going in into the master playback thread.
    /// This is mostly timing related things and the channel which the other thread can use to provide information to the playback thread.  
    sample_ingest: SampleIngestManager,

    sample_tracker: Arc<AtomicU64>,

    playback_start_ts: Instant,

    /// Mixer handle of the host. This is used to append samples to the host's output.
    host_mixer: Mixer,
}

impl MasterPlaybackThread {
    pub fn new(
        host_mixer: Mixer,
        fx_map: FXMap,
        plugin_manager: Arc<RwLock<PluginManager>>,
    ) -> anyhow::Result<Self> {
        // Create a thread pool with the default settings
        // CPU core count equals thread count.
        let worker_thread_pool = ThreadPoolBuilder::new().build()?;
        let sample_tracker = Arc::new(AtomicU64::new(0));

        // This will be handed to the thread the other is returned
        let sample_tracker_clone = sample_tracker.clone();

        // Create sample ingest channel, this serves as a way for the main thread to send information to the master playback thread.
        let (sender, receiver) = flume::bounded::<Vec<SampleBuffer>>(1024);
        let host_mixer_clone = host_mixer.clone();

        // Create a map of effects which the samples will be applied with.
        let fx_map_clone = fx_map.clone();

        // Create a thread for handling incoming samples
        std::thread::spawn(move || {
            let host_mixer = host_mixer_clone.clone();

            // Never close the queue
            let (queue_in, queue_out) = queue(true);

            // Append queue to Mixer
            host_mixer.add(queue_out);

            // Clone a handle to the effects map so that it can be read later
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

            // Get a temp reference inot the host's state
            // This is only used for initialization
            let info = HOST_STATE.load();

            // Zero the mixer buffer
            let mut mixer_buffer: Vec<f32> = vec![0.0; (info.channel_count as usize * info.sample_rate as usize * PLAYBACK_BUFFER_LEN_MS)
                    / 1000];

            // Drop the guard with the host information
            drop(info);

            // Resample input - all inputs could vary in length, however the output length doesnt really matter (input is going to be fixed cuz its easier to implement).
            let resamplers: Arc<DashMap<u32, Mutex<Async<f32>>>> = Arc::new(DashMap::new());

            // A handful of spare buffers is enough — this isn't meant to hold everything,
            // just avoid allocating on every single iteration after a few iterations.
            let (buf_return_tx, buf_return_rx) = flume::bounded::<Vec<f32>>(4);

            loop {
                // Listen for an incoming sample packet
                match receiver.recv() {
                    Ok(samples) => {
                        // Get a reference to the host state for this sample packet
                        let host_info = HOST_STATE.load();

                        // Get the updated buffer size every sample packet
                        let buffer_size =
                            (host_info.channel_count as usize * host_info.sample_rate as usize * PLAYBACK_BUFFER_LEN_MS)
                                / 1000;

                        // Handle samples by passing them into the pipeline
                        // This function has a side effect on `processed_sample_buffer`.
                        process_samples(
                            &worker_thread_pool,
                            samples,
                            &host_info,
                            &params,
                            &mut processed_sample_buffer,
                            resamplers.clone(),
                            effects_map.clone(),
                            plugin_manager.clone(),
                        )
                        .expect("Failed to process sample in master playback thread.");

                        // Mix all of the samples into `mixer_buffer`
                        mix_samples(&mut mixer_buffer, &processed_sample_buffer);

                        // Hand off the filled buffer without copying it, grabbing a recycled (already-allocated) buffer to use next iteration if one's available.
                        let filled = std::mem::replace(
                            &mut mixer_buffer,
                            buf_return_rx
                                .try_recv()
                                .unwrap_or_else(|_| Vec::with_capacity(buffer_size)),
                        );

                        // Clear the buffer
                        mixer_buffer.clear();
                        // Zero buffer so that 
                        mixer_buffer.resize(buffer_size, 0.0);

                        // Created a tracked sample which basically dereferences (not actually but all trait function calls go to the inner value) to the inner value.
                        let tracked_sample = Tracked::new(
                            SampleBuffer::new(
                                filled,
                                0,
                                host_info.sample_rate,
                                host_info.channel_count,
                            )
                            // Set the recycler so that this allocation can be reused
                            .with_recycler(buf_return_tx.clone()),
                            sample_tracker_clone.clone(),
                        );

                        // Append to queue
                        queue_in.append(tracked_sample);
                    }
                    Err(error) => {
                        // Print the error but we shouldnt stop execution
                        eprintln!("Error in Master Playback Thread: {error}");
                    }
                }
            }
        });

        Ok(Self {
            sample_ingest: SampleIngestManager {
                ingest_channel: sender,
            },
            host_mixer,
            playback_start_ts: Instant::now(),
            sample_tracker,
        })
    }
}
