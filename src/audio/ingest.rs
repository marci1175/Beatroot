use std::{
    collections::HashMap,
    fs::File,
    sync::{Arc, atomic::AtomicU64},
};

use indexmap::IndexMap;
use parking_lot::{Mutex, RwLock};
use rodio::{Decoder, Source};

use crate::{
    audio::{
        host::HOST_STATE,
        playback::{SampleBuffer, calculate_playback_chunk_size},
    },
    internals::utils::Stopper,
    ui::panels::playlist::{Position, SampleInstance},
};

#[derive(Debug, Clone)]
/// The struct hold every relevant information for an ingest thread to correctly send the desired sample packets to the playback thread.
pub struct SampleIngestManager {
    /// Samples are provided from a set amount of tracks (cpu core count) in pre-determined buffer sizes.
    /// For example the samples are ingested from every 10 tracks. So we have to ingest those 10 tracks worth of samples before moving on to the 2nd set of 10 and so forth.
    /// If there are less than 10 tracks available the remainder of worker threads will be idle.
    pub ingest_channel: flume::Sender<Vec<SampleBuffer>>,

    /// This is used to mark where the ingest thread should offset sample index to fetch the next sample packet.
    /// When a sample is received it is immediately incremented (but is only made available after the stopper unlocks)
    pub sample_ingest_tracker: Arc<AtomicU64>,

    /// Basically a limiter for the ingest thread, this is controlled by the playback thread to manage the amount of
    pub should_ingest: Stopper,
}

pub fn create_ingest_thread(
    ingest_manager: SampleIngestManager,
    bpm: Arc<Mutex<f32>>,
    playlist: Arc<RwLock<IndexMap<Position, Vec<SampleInstance>>>>,
) {
    std::thread::spawn(move || {
        let mut ingested_samples: Vec<SampleBuffer> = Vec::with_capacity(32);

        // A cache for samples that are needed in quick succession. (This should be automaitcally cleaned up and managed by the loop below)
        let mut cache: HashMap<usize, SampleBuffer> = HashMap::new();

        loop {
            // Get latest host info
            let host_info = HOST_STATE.load();

            // Fetch the ingestable samples, before acutally waiting for the signal to send
            for (pos, samples) in &*playlist.read() {
                for sample in samples {
                    // Calculate ingest chunk size
                    let chunk_size = calculate_playback_chunk_size(&host_info);

                    // Calculate the sample's absolute position (sample index compared to the entirety of the playlist)
                    let sample_pos_absolute =
                        calculate_sample_pos(*bpm.lock(), pos.beat, host_info.sample_rate as usize);

                    // Fetch from which sample position should we be ingesting from
                    let chunk_start_absolute = ingest_manager
                        .sample_ingest_tracker
                        .load(std::sync::atomic::Ordering::Relaxed)
                        as usize;

                    // NOTICE: `chunk_end_absolute` is guaranteed to be bigger than `chunk_start_absolute`
                    let chunk_end_absolute = chunk_start_absolute + chunk_size;

                    // If the sample is out of range
                    if sample_pos_absolute > chunk_end_absolute {
                        continue;
                    }
                    // If the sample is only partially in range
                    else if sample_pos_absolute > chunk_start_absolute
                        && sample_pos_absolute < chunk_end_absolute
                    {
                        // Until which sample position should we fetch samples (from the start)
                        let until_sample = (chunk_end_absolute) - sample_pos_absolute;

                        // Read and cache the sample
                        if let Ok(file) = File::open(&sample.path) {
                            if let Ok(source) = Decoder::try_from(file) {
                                let sample_rate = source.sample_rate();
                                let channels = source.channels();

                                // Collect raw samples
                                let samples: Vec<f32> = source.collect();

                                // Create cached sample
                                let cached_sample = SampleBuffer::new(
                                    samples,
                                    sample.id,
                                    sample_rate.into(),
                                    channels.into(),
                                );

                                // Where in the chunk the real audio should start (rest stays silent)
                                let offset_in_chunk = chunk_size - until_sample;

                                // Build a full chunk-sized buffer, zero-padded at the front
                                let mut padded = vec![0.0f32; chunk_size];
                                let real = cached_sample.clone_sample_range(0..until_sample);
                                padded[offset_in_chunk..].copy_from_slice(real.samples());

                                ingested_samples.push(SampleBuffer::new(
                                    padded,
                                    sample.id,
                                    sample_rate.into(),
                                    channels.into(),
                                ));

                                // Store in cache (unpadded, full decode)
                                cache.insert(sample.id, cached_sample);
                            }
                        }
                    }
                    // If the sample is fully in range
                    else if sample_pos_absolute < chunk_start_absolute {
                        // Ensure that the sample is cached
                        if !cache.contains_key(&sample.id) {
                            // Read and cache the sample
                            // We can ignore the errors here since they wouldve been caught when importing them, but we still dont want to panic nevertheless
                            if let Ok(file) = File::open(&sample.path) {
                                if let Ok(source) = Decoder::try_from(file) {
                                    let sample_rate = source.sample_rate();
                                    let channels = source.channels();

                                    // Collect raw samples
                                    let samples: Vec<f32> = source.collect();

                                    // Check if we should cache the sample
                                    if samples.len() + sample_pos_absolute < chunk_start_absolute {
                                        continue;
                                    }

                                    // Create cached sample
                                    let cached_sample = SampleBuffer::new(
                                        samples,
                                        sample.id,
                                        sample_rate.into(),
                                        channels.into(),
                                    );

                                    // Store in cache
                                    cache.insert(sample.id, cached_sample);
                                }
                            }
                        }

                        let mut should_be_removed = false;

                        // The sample should be cached by this time if valid
                        // If the sample still isnt present in the cache we can ignore that since it probably encountered some sort of an error or was deleted
                        if let Some(cached) = cache.get(&sample.id) {
                            let sample_end_pos_absolute =
                                cached.sample_count() + sample_pos_absolute;

                            if !(sample_end_pos_absolute < chunk_start_absolute) {
                                // Convert absolute playlist positions into indices local to this buffer
                                let local_start =
                                    chunk_start_absolute.saturating_sub(sample_pos_absolute);

                                if sample_end_pos_absolute > chunk_end_absolute {
                                    let local_end = chunk_end_absolute - sample_pos_absolute;
                                    let ingest = cached.clone_sample_range(local_start..local_end);
                                    ingested_samples.push(ingest);
                                } else {
                                    // sample ends inside (or right at) this chunk — take it to the end of the buffer
                                    let local_end = cached.sample_count();
                                    let ingest = cached.clone_sample_range(local_start..local_end);
                                    ingested_samples.push(ingest);
                                    should_be_removed = true;
                                }
                            } else {
                                should_be_removed = true;
                            }
                        }

                        // If its flagged for deletion, just delete it.
                        if should_be_removed {
                            // Remove cached sample
                            cache.remove(&sample.id);
                        }
                    }
                }
            }

            // Wait until we are cleared to send the ingested data to the playback thread
            // We should only send one packet once instructed
            ingest_manager.should_ingest.should_wait_once();

            // Send ingest buffer to master playback thread
            ingest_manager
                .ingest_channel
                .send(std::mem::take(&mut ingested_samples))
                .expect("Failed to send ingest to master playback.");
        }
    });
}

fn samples_per_beat(bpm: f32, sample_rate: usize) -> f32 {
    (60.0 / bpm) * sample_rate as f32
}

pub fn calculate_sample_pos(bpm: f32, beat: usize, sample_rate: usize) -> usize {
    (beat as f32 * samples_per_beat(bpm, sample_rate)).round() as usize
}

pub fn calculate_beat_pos(bpm: f32, sample_pos: usize, sample_rate: usize) -> f32 {
    sample_pos as f32 / samples_per_beat(bpm, sample_rate)
}
