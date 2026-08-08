use std::sync::Arc;

use arc_swap::ArcSwapAny;
use dashmap::DashMap;
use parking_lot::{Mutex, RwLock};
use rayon::{
    ThreadPool,
    iter::{
        IndexedParallelIterator, IntoParallelIterator, IntoParallelRefMutIterator, Map,
        ParallelIterator,
    },
    vec::IntoIter,
};
use rubato::{
    Async, Resampler, SincInterpolationParameters, audioadapter::Adapter,
    audioadapter_buffers::owned::InterleavedOwned,
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
    resamplers: Arc<DashMap<u32, Mutex<ResamplerState>>>,
    effects_map: Arc<ArcSwapAny<Arc<DashMap<usize, NodeMap>>>>,
    _plugin_manager: Arc<RwLock<PluginManager>>,
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
    apply_effects(workers, processed_samples, effects_map.clone());

    // Return ok if we could do all of our actions without issue
    Ok(())
}

fn add_resamplers(
    original_samples: &Vec<SampleBuffer>,
    host_info: &HostInformation,
    resampler_params: &SincInterpolationParameters,
    resamplers: &Arc<DashMap<u32, Mutex<ResamplerState>>>,
) -> Result<(), anyhow::Error> {
    for sample in original_samples {
        let sample_rate = sample.sample_rate();
        let origin_id = sample.origin_id() as u32;

        if !resamplers.contains_key(&origin_id) {
            let resampler = Async::<f32>::new_sinc(
                host_info.sample_rate as f64 / sample_rate as f64,
                2.0,
                resampler_params,
                RESAMPLER_CHUNK_SIZE,
                host_info.channel_count as usize,
                rubato::FixedAsync::Input,
            )?;

            resamplers.insert(
                origin_id,
                Mutex::new(ResamplerState {
                    resampler,
                    input_carry: Vec::new(),
                    output_leftover: Vec::new(),
                }),
            );
        }
    }
    Ok(())
}
use crate::audio::playback::calculate_playback_chunk_size;

pub struct ResamplerState {
    resampler: Async<f32>,
    /// Leftover input not yet enough to feed the resampler.
    input_carry: Vec<f32>,
    /// Resampled output not yet consumed into a chunk.
    output_leftover: Vec<f32>,
}

fn resample(
    workers: &ThreadPool,
    original_samples: Vec<SampleBuffer>,
    host_info: &HostInformation,
    resamplers: Arc<DashMap<u32, Mutex<ResamplerState>>>,
) -> Map<IntoIter<SampleBuffer>, impl Fn(SampleBuffer) -> SampleBuffer> {
    workers.install(|| {
        original_samples.into_par_iter().map(move |sample| {
            if sample.sample_rate() != host_info.sample_rate {
                let state_guard = resamplers.get_mut(&(sample.origin_id() as u32)).unwrap();
                let mut state = state_guard.lock();
                let ResamplerState {
                    resampler,
                    input_carry,
                    output_leftover,
                } = &mut *state;

                let channels = sample.channels() as usize;
                input_carry.extend_from_slice(sample.samples());

                // Feed the resampler exactly the fixed frame count it wants, as many times as we currently have input for.
                loop {
                    let needed_frames = resampler.input_frames_next();
                    let needed_len = needed_frames * channels;
                    if input_carry.len() < needed_len {
                        break;
                    }

                    let input_chunk: Vec<f32> = input_carry.drain(..needed_len).collect();

                    // Wrap in a SampleBuffer instead of InterleavedOwned — SampleBuffer already has
                    // a working manual Adapter impl, so no ExactSizeBuf trait wrangling needed.
                    let input_sample = SampleBuffer::new(
                        input_chunk,
                        sample.origin_id(),
                        sample.sample_rate(),
                        sample.channels(),
                    );

                    let out_frames = resampler.output_frames_next();
                    let mut output_buffer = InterleavedOwned::new(0.0, channels, out_frames);

                    let (_consumed, produced) = resampler
                        .process_into_buffer(&input_sample, &mut output_buffer, None)
                        .unwrap();

                    let mut produced_samples = output_buffer.take_data();
                    produced_samples.truncate(produced * channels);
                    output_leftover.extend_from_slice(&produced_samples);
                }

                let target_frames =
                    calculate_playback_chunk_size(host_info) / host_info.channel_count as usize;
                let target_len = target_frames * channels;

                if output_leftover.len() >= target_len {
                    let out: Vec<f32> = output_leftover.drain(..target_len).collect();
                    SampleBuffer::new(
                        out,
                        sample.origin_id(),
                        host_info.sample_rate,
                        sample.channels(),
                    )
                } else {
                    // Only the very first chunk or two, while the resampler's one-time
                    // startup delay is still draining — steady state fills every time after.
                    let mut out = output_leftover.clone();
                    out.resize(target_len, 0.0);
                    output_leftover.clear();
                    SampleBuffer::new(
                        out,
                        sample.origin_id(),
                        host_info.sample_rate,
                        sample.channels(),
                    )
                }
            } else {
                sample
            }
        })
    })
}

/// TODO: Revisit these to remove all reallocs from buffers
fn apply_effects(
    workers: &ThreadPool,
    samples: &mut Vec<SampleBuffer>,
    effects_map: Arc<ArcSwapAny<Arc<DashMap<usize, NodeMap>>>>,
) {
    workers.install(|| {
        let fx_map = effects_map.load();
        samples.par_iter_mut().for_each(|sample| {
            // Lookup the fx chain for the sample if there is one
            if let Some(entry) = fx_map.get(&sample.origin_id()) {
                // Get the effects chain
                let fx = entry.value();

                // If there is one create an output buffer and convert the sample to planar
                // We have to de-interleave the sample we are applying the effects to (convert to planar)
                let planar = sample.deinterleave();

                // Create outputs buffer (this may be resized later but always made bigger if needed)
                let mut outputs: Vec<Vec<f32>> = planar.clone();

                // Check if the current fx sequence is valid
                if let Ok(fx_chain) = fx.create_effect_sequence() {
                    'effect_loop: for effect_id in fx_chain {
                        // Get the node of the effect from its id
                        let node = &fx.nodes()[effect_id];

                        // Match the node type so that we can apply the effect appropriately
                        // This match statement will have a side effect on the samples.
                        match node.node_type() {
                            // Output and input nodes do not do anything
                            crate::ui::fx_map::NodeType::In | crate::ui::fx_map::NodeType::Out => {}
                            // Apply the effect from the external plugin, apply it appropirately to the effect type.
                            crate::ui::fx_map::NodeType::ExternalPlugin {
                                plugin_instance, ..
                            } => {
                                // If any error occured while handling the plugin just skip the entire plugin.
                                if let Ok(plugin_instance) = plugin_instance.get() {
                                    // If the plugin is invalid, just skip the plugin
                                    if plugin_instance
                                        .is_invalid
                                        .load(std::sync::atomic::Ordering::Relaxed)
                                    {
                                        continue 'effect_loop;
                                    }

                                    // Raw pointer to the plugin
                                    let raw_aeffect =
                                        plugin_instance.plugin_instance_ptr as *mut AEffect;
                                    let aeffect = unsafe { &*raw_aeffect };

                                    // Get the number of inputs and outputs its expecting
                                    let input_count = aeffect.numInputs as usize;
                                    let output_count = aeffect.numOutputs as usize;

                                    // Track the current chunk we are at
                                    let mut chunk_idx = 0;

                                    // Iter over the entirety of the sample buffer
                                    while chunk_idx < sample.frames() {
                                        // Calculate the current chunk size to avoid reading out of bounds
                                        let current_chunk_size =
                                            (sample.frames() - chunk_idx).min(RESAMPLER_CHUNK_SIZE);

                                        // Allocate the buffers for both the input and output
                                        let mut inputs: Vec<Vec<f32>> = outputs
                                            .iter()
                                            .map(|channel| {
                                                channel[chunk_idx..chunk_idx + current_chunk_size]
                                                    .to_vec()
                                            })
                                            .collect();

                                        // Resize both buffers into desired size, if they are smaller (to avoid sample loss if a plugin needs less channels)
                                        if inputs.len() < input_count {
                                            inputs
                                                .resize(input_count, vec![0.0; current_chunk_size]);
                                        }
                                        if outputs.len() < output_count {
                                            outputs.resize(
                                                output_count,
                                                vec![0.0; current_chunk_size],
                                            );
                                        }

                                        // Create a list of pointers for both, this will get passed into the plugin
                                        let input_ptrs: Vec<*const f32> =
                                            inputs.iter_mut().map(|c| c.as_ptr()).collect();

                                        let mut output_ptrs: Vec<*mut f32> = outputs
                                            .iter_mut()
                                            .map(|c| unsafe { c.as_mut_ptr().add(chunk_idx) })
                                            .collect();

                                        // Apply effects
                                        (aeffect.processReplacing)(
                                            raw_aeffect,
                                            input_ptrs.as_ptr(),
                                            output_ptrs.as_mut_ptr(),
                                            current_chunk_size as i32,
                                        );

                                        // Increment index by current chunk size
                                        chunk_idx += current_chunk_size;
                                    }
                                }
                            }
                            crate::ui::fx_map::NodeType::InternalCustom(
                                _plugin_node_properties,
                            ) => {}
                        }
                    }
                }

                // Replace sample with the output of the effects chain
                sample.replace_from_planar(&outputs);
            }
        })
    });
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
