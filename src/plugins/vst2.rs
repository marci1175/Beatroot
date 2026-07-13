use std::ffi::c_void;

use vst::api::AEffect;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, num_enum::TryFromPrimitive)]
pub enum AudioMasterOpcode {
    Automate = 0,
    Version = 1,
    CurrentId = 2,
    Idle = 3,
    PinConnected = 4, // deprecated
    // 5 = gap
    WantMidi = 6, // deprecated
    GetTime = 7,
    ProcessEvents = 8,
    SetTime = 9,                      // deprecated
    TempoAt = 10,                     // deprecated
    GetNumAutomatableParameters = 11, // deprecated
    GetParameterQuantization = 12,    // deprecated
    IOChanged = 13,
    NeedIdle = 14, // deprecated
    SizeWindow = 15,
    GetSampleRate = 16,
    GetBlockSize = 17,
    GetInputLatency = 18,
    GetOutputLatency = 19,
    GetPreviousPlug = 20,         // deprecated
    GetNextPlug = 21,             // deprecated
    WillReplaceOrAccumulate = 22, // deprecated
    GetCurrentProcessLevel = 23,
    GetAutomationState = 24,
    OfflineStart = 25,
    OfflineRead = 26,
    OfflineWrite = 27,
    OfflineGetCurrentPass = 28,
    OfflineGetCurrentMetaPass = 29,
    SetOutputSampleRate = 30,         // deprecated
    GetOutputSpeakerArrangement = 31, // deprecated
    GetVendorString = 32,
    GetProductString = 33,
    GetVendorVersion = 34,
    VendorSpecific = 35,
    SetIcon = 36, // deprecated
    CanDo = 37,
    GetLanguage = 38,
    OpenWindow = 39,  // deprecated
    CloseWindow = 40, // deprecated
    GetDirectory = 41,
    UpdateDisplay = 42,
    BeginEdit = 43,
    EndEdit = 44,
    OpenFileSelector = 45,
    CloseFileSelector = 46,
    EditFile = 47,                   // deprecated
    GetChunkFile = 48,               // deprecated
    GetInputSpeakerArrangement = 49, // deprecated
}


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
            AudioMasterOpcode::Automate => {
                0
            }
            AudioMasterOpcode::Version => {
                2400
            }
            AudioMasterOpcode::CurrentId => {
                0
            }
            AudioMasterOpcode::Idle => {}
            AudioMasterOpcode::PinConnected => {}
            AudioMasterOpcode::WantMidi => {}
            AudioMasterOpcode::GetTime => {}
            AudioMasterOpcode::ProcessEvents => {}
            AudioMasterOpcode::SetTime => {}
            AudioMasterOpcode::TempoAt => {}
            AudioMasterOpcode::GetNumAutomatableParameters => {}
            AudioMasterOpcode::GetParameterQuantization => {}
            AudioMasterOpcode::IOChanged => {}
            AudioMasterOpcode::NeedIdle => {}
            AudioMasterOpcode::SizeWindow => {}
            AudioMasterOpcode::GetSampleRate => {}
            AudioMasterOpcode::GetBlockSize => {}
            AudioMasterOpcode::GetInputLatency => {}
            AudioMasterOpcode::GetOutputLatency => {}
            AudioMasterOpcode::GetPreviousPlug => {}
            AudioMasterOpcode::GetNextPlug => {}
            AudioMasterOpcode::WillReplaceOrAccumulate => {}
            AudioMasterOpcode::GetCurrentProcessLevel => {}
            AudioMasterOpcode::GetAutomationState => {}
            AudioMasterOpcode::OfflineStart => {}
            AudioMasterOpcode::OfflineRead => {}
            AudioMasterOpcode::OfflineWrite => {}
            AudioMasterOpcode::OfflineGetCurrentPass => {}
            AudioMasterOpcode::OfflineGetCurrentMetaPass => {}
            AudioMasterOpcode::SetOutputSampleRate => {}
            AudioMasterOpcode::GetOutputSpeakerArrangement => {}
            AudioMasterOpcode::GetVendorString => {}
            AudioMasterOpcode::GetProductString => {}
            AudioMasterOpcode::GetVendorVersion => {}
            AudioMasterOpcode::VendorSpecific => {}
            AudioMasterOpcode::SetIcon => {}
            AudioMasterOpcode::CanDo => {}
            AudioMasterOpcode::GetLanguage => {
                // Locale id for English
                1
            }
            AudioMasterOpcode::OpenWindow => {}
            AudioMasterOpcode::CloseWindow => {}
            AudioMasterOpcode::GetDirectory => {}
            AudioMasterOpcode::UpdateDisplay => {}
            AudioMasterOpcode::BeginEdit => {}
            AudioMasterOpcode::EndEdit => {}
            AudioMasterOpcode::OpenFileSelector => {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    1
                }
                else { 
                    0
                }
            }
            AudioMasterOpcode::CloseFileSelector => {
                0
            }
            AudioMasterOpcode::EditFile => {}
            AudioMasterOpcode::GetChunkFile => {}
            AudioMasterOpcode::GetInputSpeakerArrangement => {}
            _ => (),
        };

        return_val
    };
    else {
        0
    }
}