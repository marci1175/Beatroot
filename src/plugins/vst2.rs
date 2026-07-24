use crate::plugins::api::vst2::{AudioMasterOpcode, VstFileSelect, VstOpcode};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::{
    ffi::{c_str, c_void},
    sync::LazyLock,
};
use vst::api::AEffect;

/// A queue for the plugin to request save changes in the DAW.
/// These "requests" contain which plugin requested and what.
pub static PARAMETER_CHANNEL: LazyLock<(Sender<Parameter>, Receiver<Parameter>)> =
    LazyLock::new(unbounded);

#[derive(Debug, Clone, Copy)]
pub struct Parameter {
    /// The pointer to the plugin's handler
    /// The underlying type may change from plugin to plugin.
    pub plugin_pointer: usize,

    /// The index of the parameter.
    pub index: i32,

    /// The value of the parameter.
    pub value: f32,
}

/// Main host callback passed to every plugin.
pub extern "C" fn host_callback(
    effect: *mut AEffect,
    opcode: i32,
    index: i32,
    _value: isize,
    ptr: *mut c_void,
    opt: f32,
) -> isize {
    // Check if the opcode is supported.
    if let Ok(opcode) = AudioMasterOpcode::try_from(opcode) {
        match opcode {
            // This opcode is called when the vst2 plugin is modified via its ui.
            AudioMasterOpcode::Automate => {
                // Store the parameter change along with which plugin was called
                PARAMETER_CHANNEL.0.send(Parameter {
                    plugin_pointer: effect as usize,
                    index,
                    value: opt,
                });

                0
            }
            AudioMasterOpcode::Version => 2400,
            AudioMasterOpcode::CurrentId => 0,
            AudioMasterOpcode::Idle => {
                // Handle GUI update
                unsafe {
                    if !effect.is_null() {
                        let eff = &*effect;

                        (eff.dispatcher)(
                            effect as *mut AEffect,
                            VstOpcode::EditIdle.as_i32(),
                            0,
                            0,
                            std::ptr::null_mut(),
                            0.0,
                        );
                    }
                }

                0
            }
            AudioMasterOpcode::PinConnected => 0,
            AudioMasterOpcode::WantMidi => 1,
            AudioMasterOpcode::GetTime => {
                // let time_info = VstTimeInfo {
                //     sample_pos: todo!(),
                //     sample_rate: todo!(),
                //     nano_seconds: todo!(),
                //     ppq_pos: todo!(),
                //     tempo: todo!(),
                //     bar_start_pos: todo!(),
                //     cycle_start_pos: todo!(),
                //     cycle_end_pos: todo!(),
                //     time_sig_numerator: todo!(),
                //     time_sig_denominator: todo!(),
                //     smpte_offset: todo!(),
                //     smpte_frame_rate: todo!(),
                //     samples_to_next_clock: todo!(),
                //     flags: todo!(),
                // };

                // return &time_info as *const _ as isize;

                0
            }
            AudioMasterOpcode::ProcessEvents => {
                // asd
                0
            }
            AudioMasterOpcode::SetTime => 0,
            AudioMasterOpcode::TempoAt => 0,
            AudioMasterOpcode::GetNumAutomatableParameters => 0,
            AudioMasterOpcode::GetParameterQuantization => 0,
            AudioMasterOpcode::IOChanged => {
                // asd
                0
            }
            AudioMasterOpcode::NeedIdle => 1,
            AudioMasterOpcode::SizeWindow => {
                // asd
                0
            }
            AudioMasterOpcode::GetSampleRate => {
                // asd
                0
            }
            AudioMasterOpcode::GetBlockSize => {
                // asd
                0
            }
            AudioMasterOpcode::GetInputLatency => 0,
            AudioMasterOpcode::GetOutputLatency => 0,
            AudioMasterOpcode::GetPreviousPlug => 0,
            AudioMasterOpcode::GetNextPlug => 0,
            AudioMasterOpcode::WillReplaceOrAccumulate => 1,
            AudioMasterOpcode::GetCurrentProcessLevel => {
                // asd
                0
            }
            AudioMasterOpcode::GetAutomationState => 0,
            AudioMasterOpcode::OfflineStart => 0,
            AudioMasterOpcode::OfflineRead => 0,
            AudioMasterOpcode::OfflineWrite => 0,
            AudioMasterOpcode::OfflineGetCurrentPass => 0,
            AudioMasterOpcode::OfflineGetCurrentMetaPass => 0,
            AudioMasterOpcode::SetOutputSampleRate => 0,
            AudioMasterOpcode::GetOutputSpeakerArrangement => 0,
            AudioMasterOpcode::GetVendorString => {
                if !ptr.is_null() {
                    let vendor = c_str::CString::new("marci").unwrap();
                    let vendor = vendor.as_bytes_with_nul();

                    for byte_idx in 0..vendor.len() {
                        unsafe { (ptr as *mut u8).add(byte_idx).write(vendor[byte_idx]) };
                    }

                    1
                } else {
                    0
                }
            }
            AudioMasterOpcode::GetProductString => {
                if !ptr.is_null() {
                    let vendor = c_str::CString::new("beatroot").unwrap();
                    let vendor = vendor.as_bytes_with_nul();

                    for byte_idx in 0..vendor.len() {
                        unsafe { (ptr as *mut u8).add(byte_idx).write(vendor[byte_idx]) };
                    }

                    1
                } else {
                    0
                }
            }
            AudioMasterOpcode::GetVendorVersion => 1,
            AudioMasterOpcode::VendorSpecific => 1,
            AudioMasterOpcode::SetIcon => 0,
            AudioMasterOpcode::CanDo => {
                /*
                    Plugin asks "can you do X" (a string in ptr, e.g. "sendVstEvents", "sendVstMidiEvent", "receiveVstTimeInfo").
                    Return 1 (yes), 0 (no/don't know), or -1 (explicitly no).
                */

                0
            }
            AudioMasterOpcode::GetLanguage => {
                // Locale id for English
                1
            }
            AudioMasterOpcode::OpenWindow => 0,
            AudioMasterOpcode::CloseWindow => 0,
            AudioMasterOpcode::GetDirectory => 0,
            AudioMasterOpcode::UpdateDisplay => {
                // TODO: refresh cached parameter info if I keep any
                0
            }
            AudioMasterOpcode::BeginEdit => {
                // TODO: if recording automation, start capturing changes for `index`
                1
            }
            AudioMasterOpcode::EndEdit => {
                // TODO: if recording automation, stop/finalize capture for `index`
                1
            }
            AudioMasterOpcode::OpenFileSelector => {
                let file_select_ptr = unsafe { &mut *(ptr as *mut VstFileSelect) };

                if let Some(path) = {
                    let file_dialog = rfd::FileDialog::new();

                    match file_select_ptr.command {
                        super::api::vst2::VstFileSelectCommand::FileLoad => file_dialog.pick_file(),
                        super::api::vst2::VstFileSelectCommand::FileSave => file_dialog.save_file(),
                        // Research API for this for now itll just pick one file instead.
                        super::api::vst2::VstFileSelectCommand::MultipleFilesLoad => {
                            file_dialog.pick_file()
                        }
                        super::api::vst2::VstFileSelectCommand::DirectorySelect => {
                            file_dialog.pick_folder()
                        }
                    }
                } {
                    let path_str = path.to_string_lossy();
                    let bytes = path_str.as_bytes();
                    let max_len = (file_select_ptr.size_return_path as usize).saturating_sub(1); // room for null term
                    let len = bytes.len().min(max_len);

                    if !file_select_ptr.return_path.is_null() {
                        unsafe {
                            std::ptr::copy_nonoverlapping(
                                bytes.as_ptr(),
                                file_select_ptr.return_path as *mut u8,
                                len,
                            );
                            *(file_select_ptr.return_path as *mut u8).add(len) = 0; // null terminator
                        }

                        // Return success if everything was ok
                        return 1;
                    }
                }

                0
            }
            AudioMasterOpcode::CloseFileSelector => 0,
            AudioMasterOpcode::EditFile => 0,
            AudioMasterOpcode::GetChunkFile => 0,
            AudioMasterOpcode::GetInputSpeakerArrangement => 0,
        }
    }
    // If its an unsupported opcode just return 1
    else {
        1
    }
}

const EFF_FLAGS_PROGRAM_CHUNKS: i32 = 1 << 5;

pub unsafe fn save_state(effect: *mut AEffect) -> Vec<u8> {
    let flags = (unsafe { &*effect }).flags;
    if flags & EFF_FLAGS_PROGRAM_CHUNKS != 0 {
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let size = ((unsafe { &*effect }).dispatcher)(
            effect,
            VstOpcode::GetChunk.as_i32(),
            0, /* bank, not just current program */
            0,
            &mut ptr as *mut _ as *mut c_void,
            0.0,
        );
        // IMPORTANT: ptr is owned by the plugin. Copy it NOW —
        // it may be invalidated by literally any other dispatcher call.
        unsafe { std::slice::from_raw_parts(ptr as *const u8, size as usize).to_vec() }
    } else {
        // fall back to raw f32 param dump — not portable across plugin versions,
        // but works for your own save/restore within the same session
        let n = (unsafe { &*effect }).numParams;
        let mut buf = Vec::with_capacity(n as usize * 4);
        for i in 0..n {
            let v = ((unsafe { &*effect }).getParameter)(effect, i);
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf
    }
}

pub unsafe fn set_parameter(effect: *mut AEffect, index: i32, value: f32) {
    ((unsafe { &*effect }).setParameter)(effect, index, value)
}

pub fn set_parameter_in_state(state: &mut [u8], index: usize, value: f32) {
    let offset = index * 4;
    state[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub unsafe fn restore_state(effect: *mut AEffect, data: &[u8]) {
    let flags = (unsafe { &*effect }).flags;
    if flags & EFF_FLAGS_PROGRAM_CHUNKS != 0 {
        ((unsafe { &*effect }).dispatcher)(
            effect,
            VstOpcode::SetChunk.as_i32(),
            0,
            data.len() as isize,
            data.as_ptr() as *mut c_void,
            0.0,
        );
    } else {
        for (i, chunk) in data.chunks_exact(4).enumerate() {
            let v = f32::from_le_bytes(chunk.try_into().unwrap());
            unsafe { set_parameter(effect, i as i32, v) };
        }
    }
}
