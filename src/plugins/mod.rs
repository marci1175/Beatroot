use std::{collections::HashMap, ffi::c_void, path::PathBuf, sync::LazyLock};

use ::vst::api::{AEffect, PluginMain};
use parking_lot::Mutex;

use crate::internals::library::{get_fn_addr, load_library};

pub mod fx_chain;
pub mod vst2;

pub struct HostState {}

impl Default for HostState {
    fn default() -> Self {
        Self::new()
    }
}

impl HostState {
    pub fn new() -> Self {
        Self {}
    }
}

pub static HOST_STATE: LazyLock<Mutex<HostState>> = LazyLock::new(|| Mutex::new(HostState::new()));

/// Main host callback passed to every plugin.
extern "C" fn host_callback(
    _effect: *mut AEffect,
    opcode: i32,
    _index: i32,
    _value: isize,
    _ptr: *mut c_void,
    _opt: f32,
) -> isize {
    let Ok(opcode) = vst2::AudioMasterOpcode::try_from(opcode) else {
        return 0; // unknown/unsupported opcode number, safe default
    };

    match opcode {
        vst2::AudioMasterOpcode::Automate => {}
        vst2::AudioMasterOpcode::Version => {}
        vst2::AudioMasterOpcode::CurrentId => {}
        vst2::AudioMasterOpcode::Idle => {}
        vst2::AudioMasterOpcode::PinConnected => {}
        vst2::AudioMasterOpcode::WantMidi => {}
        vst2::AudioMasterOpcode::GetTime => {}
        vst2::AudioMasterOpcode::ProcessEvents => {}
        vst2::AudioMasterOpcode::SetTime => {}
        vst2::AudioMasterOpcode::TempoAt => {}
        vst2::AudioMasterOpcode::GetNumAutomatableParameters => {}
        vst2::AudioMasterOpcode::GetParameterQuantization => {}
        vst2::AudioMasterOpcode::IOChanged => {}
        vst2::AudioMasterOpcode::NeedIdle => {}
        vst2::AudioMasterOpcode::SizeWindow => {}
        vst2::AudioMasterOpcode::GetSampleRate => {}
        vst2::AudioMasterOpcode::GetBlockSize => {}
        vst2::AudioMasterOpcode::GetInputLatency => {}
        vst2::AudioMasterOpcode::GetOutputLatency => {}
        vst2::AudioMasterOpcode::GetPreviousPlug => {}
        vst2::AudioMasterOpcode::GetNextPlug => {}
        vst2::AudioMasterOpcode::WillReplaceOrAccumulate => {}
        vst2::AudioMasterOpcode::GetCurrentProcessLevel => {}
        vst2::AudioMasterOpcode::GetAutomationState => {}
        vst2::AudioMasterOpcode::OfflineStart => {}
        vst2::AudioMasterOpcode::OfflineRead => {}
        vst2::AudioMasterOpcode::OfflineWrite => {}
        vst2::AudioMasterOpcode::OfflineGetCurrentPass => {}
        vst2::AudioMasterOpcode::OfflineGetCurrentMetaPass => {}
        vst2::AudioMasterOpcode::SetOutputSampleRate => {}
        vst2::AudioMasterOpcode::GetOutputSpeakerArrangement => {}
        vst2::AudioMasterOpcode::GetVendorString => {}
        vst2::AudioMasterOpcode::GetProductString => {}
        vst2::AudioMasterOpcode::GetVendorVersion => {}
        vst2::AudioMasterOpcode::VendorSpecific => {}
        vst2::AudioMasterOpcode::SetIcon => {}
        vst2::AudioMasterOpcode::CanDo => {}
        vst2::AudioMasterOpcode::GetLanguage => {}
        vst2::AudioMasterOpcode::OpenWindow => {}
        vst2::AudioMasterOpcode::CloseWindow => {}
        vst2::AudioMasterOpcode::GetDirectory => {}
        vst2::AudioMasterOpcode::UpdateDisplay => {}
        vst2::AudioMasterOpcode::BeginEdit => {}
        vst2::AudioMasterOpcode::EndEdit => {}
        vst2::AudioMasterOpcode::OpenFileSelector => {}
        vst2::AudioMasterOpcode::CloseFileSelector => {}
        vst2::AudioMasterOpcode::EditFile => {}
        vst2::AudioMasterOpcode::GetChunkFile => {}
        vst2::AudioMasterOpcode::GetInputSpeakerArrangement => {}
        _ => (),
    };
    0
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PluginInformation {
    pub path: PathBuf,
    pub name: String,
    pub plugin_type: PluginType,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum PluginType {
    /// Vst2.4 implemented for legacy plugin support.
    Vst2,

    /// Vst 3.x, the latest vst edition.
    Vst3,

    /// Special rust based plugin format.
    Clap,

    /// The application's own extension format.
    /// These plugin dont have to be audio related they could just provide something extra in the application itself.
    Lua,
}

#[derive(Debug, Clone)]
pub struct PluginHandle {}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct PluginManager {
    /// The saved path to the plugins we want to load in at startup or during runtime.
    pub plugins_path: Vec<PathBuf>,

    #[serde(skip)]
    /// This field should get reinitalized at every startup since the libraries are dynamically resolved.
    pub plugins: HashMap<PluginInformation, PluginHandle>,
}

impl PluginManager {
    ///
    /// Initalizes the PluginManager by loading all of the plugins present in the `plugins_path` field.
    ///
    pub fn init(&mut self) -> anyhow::Result<()> {
        // Load plugins from path and retrive basic information
        for path in &self.plugins_path {
            // Load plugin into memory
            let module_handle = load_library(path)?;

            // Fetch the main function of the plugin from which we can set up the plugin.
            // Search for the "VSTPluginMain" entrypoint.
            // This is not the real signature of the function, we have to transmute it.
            if let Some(function) = get_fn_addr(module_handle, "VSTPluginMain") {
                // SAFETY: This function signature is transmuted based on the official SDK of VST 2.x.
                let plugin_entry: PluginMain = unsafe { std::mem::transmute(function) };

                // Call the main plugin entry passing the host callback
                let _plugin_call_res = (plugin_entry)(host_callback);
            }
        }

        Ok(())
    }
}
