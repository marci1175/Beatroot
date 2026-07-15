use std::os::raw::{c_char, c_void};

use num_enum::TryFromPrimitive;

// ===================================================================
// Base integer types
// ===================================================================

pub type VstInt32 = i32;
pub type VstInt16 = i16;
pub type VstInt64 = i64;
/// Pointer-width integer - i64 on 64-bit targets, i32 on 32-bit targets.
/// `isize` tracks this automatically per compile target.
pub type VstIntPtr = isize;

pub const K_VST_VERSION: VstInt32 = 2400;

/// AEffect magic number ('VstP' packed into an i32, big-endian byte order:
/// ('V'<<24)|('s'<<16)|('t'<<8)|'P')
pub const K_EFFECT_MAGIC: VstInt32 = 0x56737450; // 'V','s','t','P'
pub const K_EFFECT_IDENTIFY: VstInt32 = 0x4E764566; // 'N','v','E','f'

// ===================================================================
// Function pointer typedefs
// ===================================================================

pub type AudioMasterCallback = unsafe extern "system" fn(
    effect: *mut AEffect,
    opcode: VstInt32,
    index: VstInt32,
    value: VstIntPtr,
    ptr: *mut c_void,
    opt: f32,
) -> VstIntPtr;

pub type AEffectDispatcherProc = unsafe extern "system" fn(
    effect: *mut AEffect,
    opcode: VstInt32,
    index: VstInt32,
    value: VstIntPtr,
    ptr: *mut c_void,
    opt: f32,
) -> VstIntPtr;

pub type AEffectProcessProc = unsafe extern "system" fn(
    effect: *mut AEffect,
    inputs: *mut *mut f32,
    outputs: *mut *mut f32,
    sample_frames: VstInt32,
);

pub type AEffectProcessDoubleProc = unsafe extern "system" fn(
    effect: *mut AEffect,
    inputs: *mut *mut f64,
    outputs: *mut *mut f64,
    sample_frames: VstInt32,
);

pub type AEffectSetParameterProc =
    unsafe extern "system" fn(effect: *mut AEffect, index: VstInt32, parameter: f32);

pub type AEffectGetParameterProc =
    unsafe extern "system" fn(effect: *mut AEffect, index: VstInt32) -> f32;

pub type VstPluginMainProc =
    unsafe extern "system" fn(audio_master: AudioMasterCallback) -> *mut AEffect;

// ===================================================================
// AEffect - the core plugin interface struct
// Field order matches vst2.h exactly (lines ~2275-2415). Do not reorder.
// ===================================================================

#[repr(C)]
pub struct AEffect {
    /// Must equal K_EFFECT_MAGIC ('VstP')
    pub magic: VstInt32,
    /// Host-to-plugin event dispatcher
    pub dispatcher: AEffectDispatcherProc,
    /// Deprecated since VST 2.4 - accumulating process mode
    pub process: AEffectProcessProc,
    /// Set an automatable parameter, value in [0.0, 1.0]
    pub set_parameter: AEffectSetParameterProc,
    /// Get an automatable parameter, returns value in [0.0, 1.0]
    pub get_parameter: AEffectGetParameterProc,
    /// Number of programs
    pub num_programs: VstInt32,
    /// Number of parameters (same across all programs)
    pub num_params: VstInt32,
    /// Number of audio inputs
    pub num_inputs: VstInt32,
    /// Number of audio outputs
    pub num_outputs: VstInt32,
    /// See VstAEffectFlags below
    pub flags: VstInt32,
    /// Reserved for host, should be zeroed
    pub __pad1: VstIntPtr,
    /// Reserved for host, should be zeroed
    pub __pad2: VstIntPtr,
    /// Latency introduced by plugin, in samples
    pub initial_delay: VstInt32,
    /// Deprecated/unused
    pub real_qualities: VstInt32,
    /// Deprecated/unused
    pub off_qualities: VstInt32,
    /// Deprecated/unused
    pub io_ratio: f32,
    /// Pointer to wrapper object (host-defined, opaque to plugin)
    pub object: *mut c_void,
    /// User-defined pointer - common place to stash per-instance host state
    pub user: *mut c_void,
    /// Unique plugin identifier
    pub unique_id: VstInt32,
    /// Plugin version, e.g. 1.1.0.0 encoded as 1100
    pub version: VstInt32,
    /// Main audio processing method (replacing mode, single precision)
    pub process_replacing: AEffectProcessProc,
    /// Same as process_replacing but double precision (VST 2.4+)
    pub process_double_replacing: AEffectProcessDoubleProc,
    /// Reserved for future use, should be zeroed
    pub reserved: [u8; 56],
}

/// VstAEffectFlags - bits in AEffect.flags
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VstAEffectFlags {
    HasEditor = 1 << 0,
    CanReplacing = 1 << 4,
    ProgramChunks = 1 << 5,
    IsSynth = 1 << 8,
    NoSoundInStop = 1 << 9,
    CanDoubleReplacing = 1 << 12,
}

// ===================================================================
// eff* opcodes (host calls plugin's dispatcher)
// Order extracted directly from `enum AEffectOpcodes` in vst2.h -
// values are the auto-incremented C enum positions (effOpen = 0).
// ===================================================================

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AEffectOpcode {
    Open = 0,
    Close,
    SetProgram,
    GetProgram,
    SetProgramName,
    GetProgramName,
    GetParamLabel,
    GetParamDisplay,
    GetParamName,
    GetVu, // deprecated
    SetSampleRate,
    SetBlockSize,
    MainsChanged,
    EditGetRect,
    EditOpen,
    EditClose,
    EditDraw,  // deprecated
    EditMouse, // deprecated
    EditKey,   // deprecated
    EditIdle,
    EditTop,   // deprecated
    EditSleep, // deprecated
    Identify,  // deprecated
    GetChunk,
    SetChunk,
    ProcessEvents,
    CanBeAutomated,
    String2Parameter,
    GetNumProgramCategories, // deprecated
    GetProgramNameIndexed,
    CopyProgram,   // deprecated
    ConnectInput,  // deprecated
    ConnectOutput, // deprecated
    GetInputProperties,
    GetOutputProperties,
    GetPlugCategory,
    GetCurrentPosition,   // deprecated
    GetDestinationBuffer, // deprecated
    OfflineNotify,
    OfflinePrepare,
    OfflineRun,
    ProcessVarIo,
    SetSpeakerArrangement,
    SetBlockSizeAndSampleRate,
    SetBypass,
    GetEffectName,
    GetErrorText, // deprecated
    GetVendorString,
    GetProductString,
    GetVendorVersion,
    VendorSpecific,
    CanDo,
    GetTailSize,
    Idle,            // deprecated
    GetIcon,         // deprecated
    SetViewPosition, // deprecated
    GetParameterProperties,
    KeysRequired, // deprecated
    GetVstVersion,
    EditKeyDown,
    EditKeyUp,
    SetEditKnobMode,
    GetMidiProgramName,
    GetCurrentMidiProgram,
    GetMidiProgramCategory,
    HasMidiProgramsChanged,
    GetMidiKeyName,
    BeginSetProgram,
    EndSetProgram,
    GetSpeakerArrangement, // deprecated
    ShellGetNextPlugin,
    StartProcess,
    StopProcess,
    SetTotalSampleToProcess,
    SetPanLaw, // deprecated
    BeginLoadBank,
    BeginLoadProgram,
    SetProcessPrecision,
    GetNumMidiInputChannels,
    GetNumMidiOutputChannels,
}

// ===================================================================
// audioMaster* opcodes (plugin calls host callback)
// Order extracted directly from `enum AudioMasterOpcodes` in vst2.h -
// values are auto-incremented C enum positions (Automate = 0).
// ===================================================================

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum AudioMasterOpcode {
    Automate = 0,
    Version,
    CurrentId,
    Idle,         // deprecated
    PinConnected, // deprecated
    // slot 5 intentionally has no name in the C enum (a comment-only gap)
    WantMidi = 6, // deprecated
    GetTime,
    ProcessEvents,
    SetTime,                     // deprecated
    TempoAt,                     // deprecated
    GetNumAutomatableParameters, // deprecated
    GetParameterQuantization,    // deprecated
    IOChanged,
    NeedIdle, // deprecated
    SizeWindow,
    GetSampleRate,
    GetBlockSize,
    GetInputLatency,
    GetOutputLatency,
    GetPreviousPlug,         // deprecated
    GetNextPlug,             // deprecated
    WillReplaceOrAccumulate, // deprecated
    GetCurrentProcessLevel,
    GetAutomationState,
    OfflineStart,
    OfflineRead,
    OfflineWrite,
    OfflineGetCurrentPass,
    OfflineGetCurrentMetaPass,
    SetOutputSampleRate,         // deprecated
    GetOutputSpeakerArrangement, // deprecated
    GetVendorString,
    GetProductString,
    GetVendorVersion,
    VendorSpecific,
    SetIcon, // deprecated
    CanDo,
    GetLanguage,
    OpenWindow,  // deprecated
    CloseWindow, // deprecated
    GetDirectory,
    UpdateDisplay,
    BeginEdit,
    EndEdit,
    OpenFileSelector,
    CloseFileSelector,
    EditFile,                   // deprecated
    GetChunkFile,               // deprecated
    GetInputSpeakerArrangement, // deprecated
}

// ===================================================================
// VstTimeInfo - returned (as a pointer) for AudioMasterOpcode::GetTime
// Field order matches vst2.h lines ~3635-3709.
// ===================================================================

#[repr(C)]
pub struct VstTimeInfo {
    /// Current position in audio samples (always valid)
    pub sample_pos: f64,
    /// Current sample rate in Hz (always valid)
    pub sample_rate: f64,
    /// System time in nanoseconds
    pub nano_seconds: f64,
    /// Musical position in quarter notes (1.0 = 1 quarter note)
    pub ppq_pos: f64,
    /// Current tempo in BPM
    pub tempo: f64,
    /// Last bar start position, in quarter notes
    pub bar_start_pos: f64,
    /// Cycle start (left locator), in quarter notes
    pub cycle_start_pos: f64,
    /// Cycle end (right locator), in quarter notes
    pub cycle_end_pos: f64,
    /// Time signature numerator (e.g. 3 for 3/4)
    pub time_sig_numerator: VstInt32,
    /// Time signature denominator (e.g. 4 for 3/4)
    pub time_sig_denominator: VstInt32,
    /// SMPTE offset in SMPTE subframes (1/80 of a frame)
    pub smpte_offset: VstInt32,
    /// SMPTE frame rate - see VstSmpteFrameRate
    pub smpte_frame_rate: VstInt32,
    /// MIDI clock resolution (24 per quarter note), can be negative
    pub samples_to_next_clock: VstInt32,
    /// See VstTimeInfoFlags below
    pub flags: VstInt32,
}

/// VstTimeInfoFlags - bits in VstTimeInfo.flags and the `value` argument
/// of AudioMasterOpcode::GetTime (which bits the plugin is requesting).
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VstTimeInfoFlags {
    TransportChanged = 1 << 0,
    TransportPlaying = 1 << 1,
    TransportCycleActive = 1 << 2,
    TransportRecording = 1 << 3,
    AutomationWriting = 1 << 6,
    AutomationReading = 1 << 7,
    NanosValid = 1 << 8,
    PpqPosValid = 1 << 9,
    TempoValid = 1 << 10,
    BarsValid = 1 << 11,
    CyclePosValid = 1 << 12,
    TimeSigValid = 1 << 13,
    SmpteValid = 1 << 14,
    ClockValid = 1 << 15,
}

// ===================================================================
// VstEvent / VstMidiEvent / VstMidiSysexEvent / VstEvents
// Used with AEffectOpcode::ProcessEvents / AudioMasterOpcode::ProcessEvents
// Field order matches vst2.h lines ~2677-2833.
// ===================================================================

/// Generic timestamped VST event - the common header shared by all
/// specific event types (VstMidiEvent, VstMidiSysexEvent, etc). You
/// inspect `event_type` and then reinterpret the same memory as the
/// specific event struct.
#[repr(C)]
pub struct VstEvent {
    /// See VstEventTypes
    pub event_type: VstInt32,
    /// Size of event data, excluding type and byte_size fields
    pub byte_size: VstInt32,
    /// Offset from start of current block, in samples
    pub delta_frames: VstInt32,
    /// Generic flags, none defined yet
    pub flags: VstInt32,
    /// Padding bytes, used by specific event types
    pub data: [u8; 16],
}

/// VstEventTypes - values for VstEvent.event_type
pub const K_VST_MIDI_TYPE: VstInt32 = 1;
pub const K_VST_AUDIO_TYPE: VstInt32 = 2; // deprecated
pub const K_VST_VIDEO_TYPE: VstInt32 = 3; // deprecated
pub const K_VST_PARAMETER_TYPE: VstInt32 = 4; // deprecated
pub const K_VST_TRIGGER_TYPE: VstInt32 = 5; // deprecated
pub const K_VST_SYSEX_TYPE: VstInt32 = 6;

#[repr(C)]
pub struct VstMidiEvent {
    /// Should be K_VST_MIDI_TYPE
    pub event_type: VstInt32,
    /// sizeof(VstMidiEvent)
    pub byte_size: VstInt32,
    /// Offset from start of current block, in samples
    pub delta_frames: VstInt32,
    /// See VstMidiEventFlags
    pub flags: VstInt32,
    /// Length of entire note in samples, if known, else 0
    pub note_length: VstInt32,
    /// Offset in samples into note from note start, if known, else 0
    pub note_offset: VstInt32,
    /// 1-3 raw MIDI packet bytes; midi_data[3] reserved, must be zero
    pub midi_data: [c_char; 4],
    /// Detune in cents, -64 to +63
    pub detune: c_char,
    /// Note-off velocity, 0-127
    pub note_off_velocity: c_char,
    /// Reserved, should be zero
    pub reserved1: c_char,
    /// Reserved, should be zero
    pub reserved2: c_char,
}

/// VstMidiEventFlags - bits in VstMidiEvent.flags
pub const K_VST_MIDI_EVENT_IS_REALTIME: VstInt32 = 1 << 0;

#[repr(C)]
pub struct VstMidiSysexEvent {
    /// Should be K_VST_SYSEX_TYPE
    pub event_type: VstInt32,
    /// sizeof(VstMidiSysexEvent)
    pub byte_size: VstInt32,
    /// Offset from start of current block, in samples
    pub delta_frames: VstInt32,
    /// No flags defined, should be zero
    pub flags: VstInt32,
    /// Size of sysex_dump, in bytes
    pub dump_bytes: VstInt32,
    /// Reserved, should be zero
    pub resvd1: VstIntPtr,
    /// Pointer to the sysex dump data
    pub sysex_dump: *mut c_char,
    /// Reserved, should be zero
    pub resvd2: VstIntPtr,
}

/// Array of VST events, passed via a pointer for ProcessEvents opcodes.
/// NOTE: `events` is a C flexible/variable-length array in the original
/// header (`VstEvent* events[2]`, but actually sized to `num_events`
/// entries at runtime by the allocator). Do NOT treat this as a fixed
/// 2-element array - allocate/read `num_events` pointers starting at
/// this field's address. See "Working with VstEvents" note at the
/// bottom of this file for how to do that safely in Rust.
#[repr(C)]
pub struct VstEvents {
    /// Number of events in the array
    pub num_events: VstInt32,
    /// Reserved, should be zero
    pub reserved: VstIntPtr,
    /// First 2 slots of the variable-length events array (see note above)
    pub events: [*mut VstEvent; 2],
}

// ===================================================================
// ERect - editor window bounds, used with AEffectOpcode::EditGetRect
// Field order matches vst2.h lines ~3590-3611.
// ===================================================================

#[repr(C)]
pub struct ERect {
    pub top: i16,
    pub left: i16,
    pub bottom: i16,
    pub right: i16,
}

// ===================================================================
// VstFileType - file filter entry used inside VstFileSelect
// Field order matches vst2.h lines ~3359-3395.
// ===================================================================

#[repr(C)]
pub struct VstFileType {
    pub name: [c_char; 128],
    pub mac_type: [c_char; 8],
    pub dos_type: [c_char; 8],
    pub unix_type: [c_char; 8],
    pub mime_type1: [c_char; 128],
    pub mime_type2: [c_char; 128],
}

// ===================================================================
// VstFileSelect - used with AudioMasterOpcode::OpenFileSelector /
// CloseFileSelector. Field order matches vst2.h lines ~3401-3472.
// ===================================================================

#[repr(C)]
pub struct VstFileSelect {
    /// See VstFileSelectCommand
    pub command: VstFileSelectCommand,
    /// See VstFileSelectType. Named `type` in C; renamed here since
    /// `type` is a Rust keyword.
    pub file_type: VstInt32,
    /// Optional Mac creator code, 0 = none
    pub mac_creator: VstInt32,
    /// Number of entries in file_types
    pub nb_file_types: VstInt32,
    /// Pointer to an array of file type filters
    pub file_types: *mut VstFileType,
    /// Title text for the selector dialog
    pub title: [c_char; 1024],
    /// Initial path to show
    pub initial_path: *mut c_char,
    /// Output: selected path. NULL on input for single-file-load /
    /// directory-select commands - host allocates, plugin must call
    /// CloseFileSelector to free it afterward.
    pub return_path: *mut c_char,
    /// Size of the allocated return_path buffer
    pub size_return_path: VstInt32,
    /// Output: selected paths for multi-file-load. NULL on input -
    /// host allocates, plugin must call CloseFileSelector to free it.
    pub return_multiple_paths: *mut *mut c_char,
    /// Number of paths in return_multiple_paths
    pub nb_return_path: VstInt32,
    /// Reserved for host use
    pub reserved: VstIntPtr,
    /// Reserved for future use
    pub future: [u8; 116],
}

/// VstFileSelectCommand - values for VstFileSelect.command
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VstFileSelectCommand {
    FileLoad = 0,
    FileSave = 1,
    MultipleFilesLoad = 2,
    DirectorySelect = 3,
}

/// VstFileSelectType - values for VstFileSelect.file_type
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VstFileSelectType {
    Simple = 0,
}

#[repr(C)]
pub struct VstPatchChunkInfo {
    /// Format version, should be 1
    pub version: VstInt32,
    /// Unique plugin identifier
    pub plugin_unique_id: VstInt32,
    /// Plugin version
    pub plugin_version: VstInt32,
    /// Number of programs (bank) or parameters (program)
    pub num_elements: VstInt32,
    /// Reserved for future use
    pub reserved: [u8; 48],
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VstOpcode {
    // Lifecycle & state
    Open = 0,
    Close = 1,
    SetProgram = 2,
    GetProgram = 3,
    SetProgramName = 4,
    GetProgramName = 5,
    GetParamLabel = 6,
    GetParamDisplay = 7,
    GetParamName = 8,
    SetSampleRate = 10,
    SetBlockSize = 11,
    MainsChanged = 12,

    // Editor / GUI
    EditGetRect = 13,
    EditOpen = 14,
    EditClose = 15,
    EditIdle = 19,
    EditTop = 20,

    // Chunk-based state
    GetChunk = 23,
    SetChunk = 24,

    // Processing / events
    ProcessEvents = 25,
    CanBeAutomated = 26,
    String2Parameter = 27,
    GetProgramNameIndexed = 29,

    // Plugin category & properties
    GetPlugCategory = 35,
    GetEffectName = 45,
    GetVendorString = 47,
    GetProductString = 48,
    GetVendorVersion = 49,
    CanDo = 51,
    GetTailSize = 52,
    GetVstVersion = 58,

    // Speaker arrangement / IO
    SetSpeakerArrangement = 42,
    GetInputProperties = 33,
    GetOutputProperties = 34,
    GetSpeakerArrangement = 69,
}

impl VstOpcode {
    /// Convert to the raw i32 expected by the dispatcher function.
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

impl From<VstOpcode> for i32 {
    fn from(op: VstOpcode) -> Self {
        op as i32
    }
}
