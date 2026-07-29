use std::sync::{Arc, LazyLock};

use arc_swap::ArcSwap;
use rodio::cpal::traits::HostTrait;

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
        Self { sample_rate, channel_count }
    }
}

/// 
/// Initiaize HostState with default parameters which will get overwritten at application startup anyway.
/// The reason im using an ArcSwap here is because the data is rarely updated but read relatively frequently. (Every ingest)
///  
pub static HOST_STATE: LazyLock<ArcSwap<HostInformation>> = LazyLock::new(|| ArcSwap::new(Arc::new(HostInformation::new(48000, 2))));

fn list_output_devices() -> Vec<rodio::cpal::Device> {
    let host = rodio::cpal::default_host();

    match host.output_devices() {
        Ok(devices) => devices.collect(),
        Err(_) => Vec::new(),
    }
}
