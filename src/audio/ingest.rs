use std::sync::{Arc, atomic::AtomicU64};

use indexmap::IndexMap;
use parking_lot::{Mutex, RwLock};

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

        loop {
            let host_info = HOST_STATE.load();

            // Fetch the ingestable samples
            for (pos, samples) in &*playlist.read() {
                for sample in samples {
                    // Calculate ingest chunk size
                    let chunk_size = calculate_playback_chunk_size(&host_info);

                    // Calculate the sample's absolute position (sample index compared to the entirety of the playlist)
                    let sample_pos_absolute =
                        calculate_sample_pos(*bpm.lock(), pos.beat, host_info.sample_rate as usize);

                    // Fetch from which sample position should we be ingesting from
                    let sample_ingest_offset = ingest_manager
                        .sample_ingest_tracker
                        .load(std::sync::atomic::Ordering::Relaxed)
                        as usize;

                    // If the sample is out of range
                    if sample_pos_absolute > sample_ingest_offset + chunk_size {
                        continue;
                    }
                    // If the sample is only partially in range
                    else if sample_pos_absolute > sample_ingest_offset {
                        // Until which sample position should we fetch samples (from the start)
                        let until_sample =
                            (sample_ingest_offset + chunk_size) - sample_pos_absolute;

                        // ingested_samples.push(sample);
                    }
                }
            }

            // Wait until we are cleared to send the ingested data to the playback thread
            ingest_manager.should_ingest.should_wait();

            ingest_manager
                .ingest_channel
                .send(std::mem::take(&mut ingested_samples))
                .expect("Failed to send ingest to master playback.");
        }
    });
}

pub fn calculate_sample_pos(bpm: f32, beat: usize, sample_rate: usize) -> usize {
    let seconds_per_beat = 60.0 / bpm;
    let samples_per_beat = seconds_per_beat * sample_rate as f32;

    (beat as f32 * samples_per_beat).round() as usize
}
