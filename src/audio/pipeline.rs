use std::{collections::HashMap, sync::Arc};

use rayon::ThreadPool;
use rodio::Source;
use rubato::{Async, SincInterpolationParameters, SincInterpolationType, WindowFunction};

use crate::audio::playback::{HostInformation, SampleBuffer};

pub const RESAMPLER_CHUNK_SIZE: usize = 1024;

/// Processes samples - this means that this function ensures that all samples match the host's sample rate and desired output.
pub fn process_samples(
    workers: &ThreadPool,
    original_samples: &[SampleBuffer],
    host_info: Arc<HostInformation>,
    resampler_params: &SincInterpolationParameters,
    processed_samples: &mut Vec<SampleBuffer>,
    resamplers: &mut HashMap<u32, Async<f32>>,
) -> anyhow::Result<()> {
    // Make the list of processed samples big enough for the samples to fit
    processed_samples.reserve(
        original_samples
            .len()
            .checked_sub(processed_samples.len())
            .unwrap_or_default(),
    );

    // Iter over all the samples and make sure we have a resampler for every sample rate.
    for sample in original_samples {
        // Get sample rate of sample
        let sample_rate = sample.sample_rate().get();

        // Only create a new resampler if it doesnt exist yet for our sample rate
        if !resamplers.contains_key(&sample_rate) {
            let resampler = Async::<f32>::new_sinc(
                host_info.sample_rate as f64 / sample_rate as f64,
                2.0,
                resampler_params,
                RESAMPLER_CHUNK_SIZE,
                host_info.channel_count as usize,
                rubato::FixedAsync::Input,
            )?;
            resamplers.insert(sample_rate, resampler);
        }
    }

    // Resample samples if sample rates mismatch
    for sample in original_samples {
        // Resample
        if sample.sample_rate().get() != host_info.sample_rate {
            // Get the correct resampler
            // It is safe to unwrap here since sample rates are checked above.
            let resampler = resamplers.get(&sample.sample_rate().get()).unwrap();
            
            
        }
    }

    Ok(())
}
