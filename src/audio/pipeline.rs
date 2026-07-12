use std::sync::Arc;

use dashmap::DashMap;
use parking_lot::Mutex;
use rayon::{
    ThreadPool,
    iter::{IndexedParallelIterator, IntoParallelIterator, Map, ParallelIterator},
    vec::IntoIter,
};
use rubato::{
    Async, Resampler, SincInterpolationParameters, audioadapter_buffers::owned::InterleavedOwned,
};

use crate::audio::playback::{HostInformation, SampleBuffer};

pub const RESAMPLER_CHUNK_SIZE: usize = 1024;

/// Processes samples - this means that this function ensures that all samples match the host's sample rate and desired output.
pub fn process_samples(
    workers: &ThreadPool,
    original_samples: Vec<SampleBuffer>,
    host_info: HostInformation,
    resampler_params: &SincInterpolationParameters,
    processed_samples: &mut Vec<SampleBuffer>,
    resamplers: Arc<DashMap<u32, Mutex<Async<f32>>>>,
) -> anyhow::Result<()> {
    // Clear processed sample buffer
    processed_samples.clear();

    // Make the list of processed samples big enough for the samples to fit
    processed_samples.reserve(
        original_samples
            .len()
            .saturating_sub(processed_samples.len()),
    );

    // Iter over all the samples and make sure we have a resampler for every sample rate.
    add_resamplers(&original_samples, &host_info, resampler_params, &resamplers)?;

    // Resample samples if sample rates mismatch
    // Load the resampled samples into the original samples vector
    resample(workers, original_samples, &host_info, resamplers).collect_into_vec(processed_samples);

    // Apply effects to each sample
    apply_effects(processed_samples);

    Ok(())
}

fn add_resamplers(
    original_samples: &Vec<SampleBuffer>,
    host_info: &HostInformation,
    resampler_params: &SincInterpolationParameters,
    resamplers: &Arc<DashMap<u32, parking_lot::lock_api::Mutex<parking_lot::RawMutex, Async<f32>>>>,
) -> Result<(), anyhow::Error> {
    for sample in original_samples {
        // Get sample rate of sample
        let sample_rate = sample.sample_rate();

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

            resamplers.insert(sample_rate, Mutex::new(resampler));
        }
    }

    Ok(())
}

fn resample(
    _workers: &ThreadPool,
    original_samples: Vec<SampleBuffer>,
    host_info: &HostInformation,
    resamplers: Arc<DashMap<u32, parking_lot::lock_api::Mutex<parking_lot::RawMutex, Async<f32>>>>,
) -> Map<IntoIter<SampleBuffer>, impl Fn(SampleBuffer) -> SampleBuffer> {
    // Run on worker threads specifically created for this.
    _workers.install(|| {
        original_samples.into_par_iter().map(move |sample| {
            // Resample if needed
            if sample.sample_rate() != host_info.sample_rate {
                // Get the correct resampler
                // It is safe to unwrap here since sample rates are checked above.
                let resampler_guard = resamplers.get_mut(&sample.sample_rate()).unwrap();

                // Lock resampler for worker thread
                let mut resampler = resampler_guard.lock();

                // Calculate input length
                let input_len = sample.sample_count() / sample.channels() as usize;

                // Fetch minimal size of output buffer
                let output_length = resampler.process_all_needed_output_len(input_len);

                let mut output_buffer =
                    InterleavedOwned::new(0.0, sample.channels() as usize, output_length);

                // Resample all samples and load into output buffer.
                // This function takes all the samples in the desired chunk size and resamples them automatically.
                let (_input_len, actual_output_len) = resampler
                    .process_all_into_buffer(&sample, &mut output_buffer, input_len, None)
                    .unwrap();

                // Get raw samples of InterleavedOwned
                let mut raw_samples = output_buffer.take_data();

                // Truncate to size
                raw_samples.truncate(actual_output_len);

                SampleBuffer::new(
                    raw_samples,
                    sample.origin_id(),
                    sample.sample_rate(),
                    sample.channels(),
                )
            } else {
                sample
            }
        })
    })
}

fn apply_effects(_samples: &mut Vec<SampleBuffer>) {}
