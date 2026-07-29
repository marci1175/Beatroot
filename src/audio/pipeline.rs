use std::{num::NonZero, sync::Arc};

use arc_swap::Guard;
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use rayon::{
    ThreadPool,
    iter::{IndexedParallelIterator, IntoParallelIterator, Map, ParallelIterator},
    vec::IntoIter,
};
use rodio::source::Zero;
use rubato::{
    Async, Resampler, SincInterpolationParameters, audioadapter::Adapter, audioadapter_buffers::owned::InterleavedOwned,
};
use vst::api::AEffect;

use crate::{
    audio::{host::HostInformation, playback::SampleBuffer},
    plugins::PluginManager,
    ui::fx_map::NodeMap,
};

pub const RESAMPLER_CHUNK_SIZE: usize = 1024;

/// Processes samples - this means that this function ensures that all samples match the host's sample rate and desired output.
pub fn process_samples(
    workers: &ThreadPool,
    original_samples: Vec<SampleBuffer>,
    host_info: &HostInformation,
    resampler_params: &SincInterpolationParameters,
    processed_samples: &mut Vec<SampleBuffer>,
    resamplers: Arc<DashMap<u32, Mutex<Async<f32>>>>,
    effects_map: Arc<DashMap<usize, NodeMap>>,
    plugin_manager: Arc<RwLock<PluginManager>>,
) -> anyhow::Result<()> {
    // Clear processed sample buffer, this does not reallocate (doesnt create a new vector, so this is pretty quick)
    processed_samples.clear();

    // Make the list of processed samples big enough for the samples to fit
    // We shouldnt really need this
    processed_samples.reserve(
        original_samples
            .len()
            .saturating_sub(processed_samples.len()),
    );

    // Iter over all the samples and make sure we have a resampler for every sample rate.
    add_resamplers(&original_samples, host_info, resampler_params, &resamplers)?;

    // Resample samples if sample rates mismatch
    // Load the resampled samples into the original samples vector
    resample(workers, original_samples, host_info, resamplers).collect_into_vec(processed_samples);

    // Apply effects to each sample
    apply_effects(processed_samples, effects_map.clone());

    // Return ok if we could do all of our actions without issue
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
    workers: &ThreadPool,
    original_samples: Vec<SampleBuffer>,
    host_info: &HostInformation,
    resamplers: Arc<DashMap<u32, parking_lot::lock_api::Mutex<parking_lot::RawMutex, Async<f32>>>>,
) -> Map<IntoIter<SampleBuffer>, impl Fn(SampleBuffer) -> SampleBuffer> {
    // Run on worker threads specifically created for this.
    workers.install(|| {
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

/// TODO: Revisit these to remove all reallocs from buffers
fn apply_effects(
    samples: &mut Vec<SampleBuffer>,
    effects_map: Arc<DashMap<usize, NodeMap>>,
) {
    for sample in samples {
        // Lookup the fx chain for the sample if there is one
        if let Some(entry) = effects_map.get(&sample.origin_id()) {
            // Get the effects chain
            let fx = entry.value();

            // If there is one create an output buffer and convert the sample to planar
            // We have to de-interleave the sample we are applying the effects to (convert to planar)
            let planar = sample.deinterleave();

            // Create outputs buffer (this may be resized later but always made bigger if needed)
            let mut outputs: Vec<Vec<f32>> = vec![vec![0.0; sample.frames()]; sample.channels() as usize];
            
            // Check if the current fx sequence is valid
            if let Ok(fx_chain) = fx.create_effect_sequence() {
                'effect_loop: for effect_id in fx_chain {
                    // Get the node of the effect from its id
                    let node = &fx.nodes()[effect_id];

                    // Match the node type so that we can apply the effect appropriately
                    // This match statement will have a side effect on the samples.
                    match node.node_type() {
                        // Output and input nodes do not do anything
                        crate::ui::fx_map::NodeType::In | crate::ui::fx_map::NodeType::Out => (),
                        // Apply the effect from the external plugin, apply it appropirately to the effect type.
                        crate::ui::fx_map::NodeType::ExternalPlugin {
                            plugin_instance, ..
                        } => {
                            // If any error occured while handling the plugin just skip the entire plugin.
                            if let Ok(plugin_instance) = plugin_instance.get() {
                                // If the plugin is invalid, just skip the plugin
                                if plugin_instance.is_invalid.load(std::sync::atomic::Ordering::Relaxed) {
                                    continue 'effect_loop;
                                }

                                // Raw pointer to the plugin
                                let raw_aeffect = plugin_instance.plugin_instance_ptr as *mut AEffect;
                                let aeffect = unsafe { &*raw_aeffect };

                                // Get the number of inputs and outputs its expecting
                                let input_count = aeffect.numInputs as usize;
                                let output_count = aeffect.numOutputs as usize;

                                // Track the current chunk we are at
                                let mut chunk_idx = 0;

                                // Iter over the entirety of the sample buffer
                                while chunk_idx < sample.frames() {
                                    // Calculate the current chunk size to avoid reading out of bounds
                                    let current_chunk_size = (sample.frames() - chunk_idx).min(RESAMPLER_CHUNK_SIZE);

                                    // Allocate the buffers for both the input and output
                                    let mut inputs: Vec<Vec<f32>> = planar.iter().map(|channel| channel[chunk_idx..chunk_idx + current_chunk_size].to_vec()).collect();
                                    
                                    // Resize both buffers into desired size, if they are smaller (to avoid sample loss if a plugin needs less channels)
                                    if inputs.len() < input_count {
                                        inputs.resize(input_count, vec![0.0; current_chunk_size]);
                                    }
                                    if outputs.len() < output_count {
                                        outputs.resize(output_count, vec![0.0; current_chunk_size]);
                                    }

                                    // Create a list of pointers for both, this will get passed into the plugin
                                    let input_ptrs: Vec<*const f32> = inputs.iter_mut().map(|c| c.as_ptr()).collect();
                                    let mut output_ptrs: Vec<*mut f32> = outputs[chunk_idx..chunk_idx + current_chunk_size].iter_mut().map(|c| c.as_mut_ptr()).collect();

                                    // Apply effects
                                    (aeffect.processReplacing)(raw_aeffect, input_ptrs.as_ptr(), output_ptrs.as_mut_ptr(), current_chunk_size as i32);

                                    // Increment index by current chunk size
                                    chunk_idx += current_chunk_size;
                                }
                            }
                        }
                        crate::ui::fx_map::NodeType::InternalCustom(_plugin_node_properties) => {}
                    }
                }
            }
            
            // Replace sample with the output of the effects chain
            sample.replace_from_planar(&planar);
        }
    }
}

pub fn mix_samples(mixer_buffer: &mut Vec<f32>, samples: &Vec<SampleBuffer>) {
    for sample_buf in samples {
        if mixer_buffer.len() < sample_buf.sample_count() {
            mixer_buffer.resize(sample_buf.sample_count(), 0.0);
        }

        for (pos, sample) in sample_buf.samples().iter().enumerate() {
            mixer_buffer[pos] += *sample;
        }
    }
}
