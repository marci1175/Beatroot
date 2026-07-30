use std::sync::{Arc, LazyLock, atomic::AtomicU64};

use arc_swap::ArcSwap;
use rodio::Source;

use crate::audio::playback::SampleBuffer;

///
/// Contains information about the host relevant to the playback of media.
///
#[derive(Debug, Clone, Copy)]
pub struct HostInformation {
    /// Chosen sample rate for the host
    pub sample_rate: u32,
    /// The number of playback channels selected by the host
    pub channel_count: u16,
}

impl HostInformation {
    pub fn new(sample_rate: u32, channel_count: u16) -> Self {
        Self {
            sample_rate,
            channel_count,
        }
    }
}

///
/// Initiaize HostState with default parameters which will get overwritten at application startup anyway.
/// The reason im using an ArcSwap here is because the data is rarely updated but read relatively frequently. (Every ingest)
///  
pub static HOST_STATE: LazyLock<ArcSwap<HostInformation>> =
    LazyLock::new(|| ArcSwap::new(Arc::new(HostInformation::new(48000, 2))));

// fn list_output_devices() -> Vec<rodio::cpal::Device> {
//     let host = rodio::cpal::default_host();

//     match host.output_devices() {
//         Ok(devices) => devices.collect(),
//         Err(_) => Vec::new(),
//     }
// }

/// Creates a SampleBuffer that increments the passed in tracker every time a sample is consumed.
/// In the application the playlist's playback position is calculated through the consumed number of samples ([f32]).
/// ```Duration(s) = consumed_sample_count / sample_rate / channels```
pub struct Tracked<T> {
    /// Theoritically this could be anything but im just implementing this for my own [`SampleBuffer`] type.
    pub inner: T,

    // A tracker that is implemented every time an f32 is consumed. (At every [`Iterator::next()`])
    pub tracker: Arc<AtomicU64>,
}

impl<T> Tracked<T> {
    pub fn new(inner: T, tracker: Arc<AtomicU64>) -> Self {
        Self { inner, tracker }
    }
}

impl Iterator for Tracked<SampleBuffer> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        // Add one to the tracker
        self.tracker
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        self.inner.next()
    }
}

impl Source for Tracked<SampleBuffer> {
    fn current_span_len(&self) -> Option<usize> {
        self.inner.current_span_len()
    }

    fn channels(&self) -> rodio::ChannelCount {
        Source::channels(&self.inner)
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        Source::sample_rate(&self.inner)
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        Source::total_duration(&self.inner)
    }
}
