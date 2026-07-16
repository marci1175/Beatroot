use crate::plugins::api::vst2::{AudioMasterOpcode, VstFileSelect, VstOpcode, VstTimeInfo};
use std::ffi::{CString, c_str, c_void};
use vst::api::AEffect;

/// Main host callback passed to every plugin.
pub extern "C" fn host_callback(
    effect: *mut AEffect,
    opcode: i32,
    index: i32,
    value: isize,
    ptr: *mut c_void,
    opt: f32,
) -> isize {
    // Check if the opcode is supported.
    if let Ok(opcode) = AudioMasterOpcode::try_from(opcode) {
        let return_val = match opcode {
            AudioMasterOpcode::Automate => 0,
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
        };

        return_val
    }
    // If its an unsupported opcode just return 1
    else {
        1
    }
}
