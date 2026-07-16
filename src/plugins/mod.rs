use std::{collections::HashMap, ffi::c_void, path::PathBuf, sync::LazyLock};

use ::vst::api::{AEffect, PluginMain};
use indexmap::IndexMap;
use parking_lot::Mutex;
use strum::Display;
use windows::Win32::Foundation::HMODULE;

use crate::{
    internals::library::{get_fn_addr, load_library},
    plugins::vst2::host_callback,
};

pub mod api;
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

#[derive(PartialEq, Eq, Hash, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PluginInformation {
    /// The type of plugin this plugin is
    pub plugin_type: PluginType,

    /// Status of the plugin, this is used when loading in a library/plugin.
    pub status: PluginStatus,
}

#[derive(
    PartialEq, Eq, Hash, Debug, Copy, Clone, serde::Deserialize, serde::Serialize, Default, Display,
)]
pub enum PluginType {
    /// Vst2.4 implemented for legacy plugin support.
    #[default]
    Vst2,

    /// Vst 3.x, the latest vst edition.
    Vst3,

    /// Special modernized plugin format.
    Clap,

    /// The application's own extension format.
    /// These plugin dont have to be audio related they could just provide something extra in the application itself.
    Lua,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PluginHandle {
    /// Pointer to the handler struct of this plugin.
    /// The type of the plugin decides how this pointer is worked with.
    /// SAFETY: Ensure that the memory is not deallocated where this pointer is pointing to.
    pub plugin_handle_ptr: *mut usize,

    /// The type of the plugin
    pub plugin_type: PluginType,

    /// Handle to the loaded library in memory.
    pub library_handle: HMODULE,
}

unsafe impl Send for PluginHandle {}
unsafe impl Sync for PluginHandle {}

#[derive(
    Hash, Debug, Clone, Copy, serde::Deserialize, serde::Serialize, Default, PartialEq, Eq, Display,
)]
pub enum PluginStatus {
    #[default]
    Ok,
    FileNotFound,
    PluginEntryNotFound,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PluginLoader {
    pub path: PathBuf,
    pub plugin_type: PluginType,

    /// This status field gets reinitalized every time a plugin is loaded and an error occurs.
    /// Since all of the plugins are loaded into memory at startup this field will get updated every startup.
    #[serde(skip)]
    pub status: PluginStatus,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct PluginManager {
    /// The saved path to the plugins we want to load in at startup or during runtime.
    pub plugin_loaders: IndexMap<PathBuf, PluginInformation>,

    #[serde(skip)]
    /// This field should get reinitalized at every startup since the libraries are dynamically resolved.
    pub loaded_plugins: HashMap<PathBuf, PluginHandle>,
}

impl PluginManager {
    ///
    /// Initalizes the PluginManager by loading all of the plugins present in the `plugins_path` field.
    ///
    pub fn init(&mut self) {
        // Load plugins from path and retrive basic information
        for path in self.plugin_loaders.clone().keys() {
            self.load_plugin(path);
        }
    }

    ///
    /// Stores and initalizes a plugin.
    ///
    pub fn store_plugin(&mut self, path: PathBuf, plugin_type: PluginType) {
        // Store plugin entry to reload at startup
        self.plugin_loaders.insert(
            path.clone(),
            PluginInformation {
                plugin_type: plugin_type,
                status: crate::plugins::PluginStatus::Ok,
            },
        );

        // Loads plugin into memory and stores it as loaded
        self.load_plugin(&path);
    }

    /// Load plugin into memory and store it as loaded.
    fn load_plugin(&mut self, path: &PathBuf) {
        let loader = self
            .plugin_loaders
            .get_mut(path)
            .expect("Plugin expected to be stored in `PluginManager->plugin_loaders`");

        // Try loading in the plugin into memory
        if let Ok(module_handle) = dbg!(load_library(path)) {
            match loader.plugin_type {
                PluginType::Vst2 => {
                    // Fetch the main function of the plugin from which we can set up the plugin.
                    // Search for the "VSTPluginMain" entrypoint.
                    // This is not the real signature of the function, we have to transmute it.
                    if let Some(function) = get_fn_addr(module_handle, "VSTPluginMain")
                        .or_else(|| get_fn_addr(module_handle, "main"))
                    {
                        // SAFETY: This function signature is transmuted based on the official SDK of VST 2.4.
                        let plugin_entry: PluginMain = unsafe { std::mem::transmute(function) };

                        // Call the main plugin entry passing the host callback
                        let plugin_callback = (plugin_entry)(host_callback);

                        // Store plugin
                        self.loaded_plugins.insert(
                            path.clone(),
                            PluginHandle {
                                plugin_handle_ptr: plugin_callback as *mut _,
                                plugin_type: loader.plugin_type,
                                library_handle: module_handle,
                            },
                        );
                    } else {
                        loader.status = PluginStatus::PluginEntryNotFound;
                    }
                }
                PluginType::Vst3 => {}
                PluginType::Clap => {}
                PluginType::Lua => {}
            }
        } else {
            // Set the plugins state to not found.
            loader.status = PluginStatus::FileNotFound;
        }
    }
}
