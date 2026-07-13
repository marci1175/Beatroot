use std::{collections::HashMap, ffi::c_void, path::PathBuf, sync::LazyLock};

use ::vst::api::{AEffect, PluginMain};
use parking_lot::Mutex;

use crate::{internals::library::{get_fn_addr, load_library}, plugins::vst2::host_callback};

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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PluginLoader {
    pub path: PathBuf,
    pub plugin_type: PluginType,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct PluginManager {
    /// The saved path to the plugins we want to load in at startup or during runtime.
    pub plugin_loaders: Vec<PluginLoader>,

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
        for loader in &self.plugin_loaders {
            let path = &loader.path;

            // Load plugin into memory
            let module_handle = load_library(path)?;

            match loader.plugin_type {
                PluginType::Vst2 => {
                    // Fetch the main function of the plugin from which we can set up the plugin.
                    // Search for the "VSTPluginMain" entrypoint.
                    // This is not the real signature of the function, we have to transmute it.
                    if let Some(function) = get_fn_addr(module_handle, "VSTPluginMain") {
                        // SAFETY: This function signature is transmuted based on the official SDK of VST 2.x.
                        let plugin_entry: PluginMain = unsafe { std::mem::transmute(function) };

                        // Call the main plugin entry passing the host callback
                        let plugin_call_res = (plugin_entry)(host_callback);
                    }
                },
                PluginType::Vst3 => {
                    
                },
                PluginType::Clap => {
                    
                },
                PluginType::Lua => {
                    
                },
            }
        }

        Ok(())
    }
}
