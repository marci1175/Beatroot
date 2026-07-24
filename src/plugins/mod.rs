use std::{
    collections::HashMap,
    ffi::c_void,
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use ::vst::api::{AEffect, PluginMain};
use indexmap::IndexMap;
use parking_lot::{Mutex, RwLock};
use strum::Display;
use windows::Win32::{
    Foundation::{HMODULE, HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE},
};

use crate::{
    internals::{
        library::{get_fn_addr, load_library, unload_library},
        mem::str_to_pcwstr,
        windowing::{create_window, register_class},
    }, plugins::{
        api::vst2::{AEffectOpcode, ERect}, vst2::{host_callback, restore_state, save_state},
    },
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

#[derive(Debug, Clone)]
pub struct PluginHandle {
    /// Pointer to the handler struct of this plugin.
    /// The type of the plugin decides how this pointer is worked with.
    ///
    /// SAFETY: Ensure that the memory is not deallocated where this pointer is pointing to.
    ///
    /// PluginType casts:
    /// - VST2: ```*mut AEffect```
    pub plugin_handle_ptr: *mut usize,

    /// The type of the plugin
    pub plugin_type: PluginType,
    
    /// Handle to the loaded library in memory.
    pub library_handle: HMODULE,
    
    /// Every plugin has its memory snapshotted at startup to know what should a valid "default" paramter list should look like.
    /// This is used as a default setting for the plugin.
    pub startup_memory_snapshot: Vec<u8>,

    /// The window's handle if the plugin is being displayed.
    /// This is used to prevent opening up multiple windows to the same plugin and when removing the plugin.
    /// The underlying usize is actual a raw pointer casted to usize so that it can be Sent between threads.
    /// When handling this usize make sure to cast it to a `HWND(*mut c_void)`.
    pub displayed_window_handle: Arc<Mutex<Option<usize>>>,
}

impl PluginHandle {
    pub fn load_state(&self, state: &[u8]) {
        match self.plugin_type {
            PluginType::Vst2 => {
                unsafe { restore_state(self.plugin_handle_ptr as *mut _, state); }
            },
            PluginType::Vst3 => todo!(),
            PluginType::Clap => todo!(),
            PluginType::Lua => todo!(),
        }
    }

    pub fn save_state(&self) -> Vec<u8> {
        match self.plugin_type {
            PluginType::Vst2 => {
                unsafe { save_state(self.plugin_handle_ptr as *mut _) }
            },
            PluginType::Vst3 => todo!(),
            PluginType::Clap => todo!(),
            PluginType::Lua => todo!(),
        }
    }
}

impl PartialEq for PluginHandle {
    fn eq(&self, other: &Self) -> bool {
        self.plugin_handle_ptr == other.plugin_handle_ptr
            && self.plugin_type == other.plugin_type
            && self.library_handle == other.library_handle
    }
}

///
/// The set of callbacks the windows callback calls when the window has some sort of interaction.
///
pub struct PluginWindowState {
    /// This callback is called when the window is signaled to close.
    pub on_close: Box<dyn Fn()>,
    /// This callback is called when the actual window is destroyed where the plugin was displayed.
    pub on_destroy: Box<dyn Fn()>,
    /// The plugin's handle that this window is for.
    pub plugin_handle: PluginHandle,
    /// The handle of the state buffer for the plugin.
    /// The reason this is atomic is that multiple threads can write and read this entry.
    pub state_handle: Arc<RwLock<Vec<u8>>>,
}

impl PluginHandle {
    /// Closes the plugin's window.
    pub fn destroy(&self) -> anyhow::Result<()> {
        // Close based on plugin type.
        match self.plugin_type {
            PluginType::Vst2 => {
                let effect = self.plugin_handle_ptr as *mut AEffect;
                let dispatcher = unsafe { effect.read() }.dispatcher;

                // Destroy window if open
                if let Some(window_handle) = *self.displayed_window_handle.lock() {
                    // Recast the usize to a hwnd
                    let hwnd = HWND(window_handle as *mut c_void);

                    // Close the window of the plugin
                    unsafe {
                        PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0))?;
                    }
                }

                // Close window in plugin
                (dispatcher)(
                    effect,
                    AEffectOpcode::EditClose as i32,
                    0,
                    0,
                    std::ptr::null_mut(),
                    0.0,
                );

                // Close plugin
                (dispatcher)(
                    effect,
                    AEffectOpcode::Close as i32,
                    0,
                    0,
                    std::ptr::null_mut(),
                    0.0,
                );
            }
            PluginType::Vst3 => {}
            PluginType::Clap => {}
            PluginType::Lua => {}
        }

        // Free library from memory
        // Make sure we are only doing this if the plugin is safe to deallocate
        unload_library(self.library_handle)?;

        Ok(())
    }

    pub fn open(&self, state: Arc<RwLock<Vec<u8>>>) -> anyhow::Result<()> {
        // Clone the window handle so that it can be modified from the other thread
        let window_hwnd = self.displayed_window_handle.clone();

        // Load the plugin's state
        self.load_state(&*state.read());

        // Match the pulgin type and display appropriately
        match self.plugin_type {
            PluginType::Vst2 => {
                // We cast to usize because a *mut pointer does not implement Send.
                let plugin_handle_ptr = self.plugin_handle_ptr as usize;
                let effect = plugin_handle_ptr as *mut AEffect;
                let dispatcher = unsafe { effect.read().dispatcher };

                let mut rect_ptr: *mut ERect = std::ptr::null_mut();

                // Get size of plugin
                (dispatcher)(
                    effect,
                    AEffectOpcode::EditGetRect as i32,
                    0,
                    0,
                    &mut rect_ptr as *mut _ as *mut c_void,
                    0.0,
                );

                // Get Height and Width of window (of plugin)
                let (width, height) = unsafe {
                    (
                        (*rect_ptr).right - (*rect_ptr).left,
                        (*rect_ptr).bottom - (*rect_ptr).top,
                    )
                };

                // VST2 spec guarantees max 32 chars including null terminator
                let mut name_buf = [0u8; api::vst2::VSTNAMEMAXLEN];

                // Request effect name from plugin
                (dispatcher)(
                    effect,
                    AEffectOpcode::GetEffectName as i32,
                    0,
                    0,
                    name_buf.as_mut_ptr() as *mut c_void,
                    0.0,
                );

                // Read effect name
                let name = unsafe {
                    std::ffi::CStr::from_ptr(name_buf.as_ptr() as *const i8).to_string_lossy()
                };

                // Create PCWSTR from effect name string
                let (name, _bytes) = str_to_pcwstr(&name);

                // Create class for window
                let class_name = register_class(name).unwrap();

                // Clone the handle to the window
                let window_handle_clone = window_hwnd.clone();

                // Create a state for the window
                let window_state = PluginWindowState {
                    // Register the callback for when the window is destroyed
                    on_close: Box::new(move || {
                        // Signal the plugin to close
                        (dispatcher)(
                            effect,
                            AEffectOpcode::EditClose as i32,
                            0,
                            0,
                            std::ptr::null_mut(),
                            0.0,
                        );
                    }),
                    on_destroy: Box::new(move || {
                        // Signal that no window is open for this plugin.
                        *window_handle_clone.lock() = None;
                    }),
                    plugin_handle: self.clone(),
                    state_handle: state.clone(),
                };

                // Leak the state so that it wont get deallocated when this scope ends
                let state_ptr = Box::into_raw(Box::new(window_state));

                // Create window
                let hwnd = create_window(
                    class_name,
                    width as i32,
                    height as i32,
                    state_ptr as *mut c_void,
                )
                .unwrap();

                *window_hwnd.lock() = Some(hwnd.0 as usize);

                // The plugin to paint in the window handle
                (dispatcher)(
                    effect,
                    AEffectOpcode::EditOpen as i32,
                    0,
                    0,
                    hwnd.0 as *mut c_void,
                    0.0,
                );
            }
            PluginType::Vst3 => {}
            PluginType::Clap => {}
            PluginType::Lua => {}
        }

        Ok(())
    }
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
                plugin_type,
                status: crate::plugins::PluginStatus::Ok,
            },
        );

        // Loads plugin into memory and stores it as loaded
        self.load_plugin(&path);
    }

    /// Load plugin into memory and store it as loaded.
    /// This does not display the plugin itself only loads the plugin into memory.
    fn load_plugin(&mut self, path: &PathBuf) {
        let loader = self
            .plugin_loaders
            .get_mut(path)
            .expect("Plugin expected to be stored in `PluginManager->plugin_loaders`");

        // Try loading in the plugin into memory
        if let Ok(module_handle) = load_library(path) {
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
                                // The pointer to the plugin's handler
                                plugin_handle_ptr: plugin_callback as *mut _,
                                
                                // The plugins type
                                plugin_type: loader.plugin_type,
                                
                                // The raw dll module handle
                                library_handle: module_handle,

                                // Indicates whether a window is opened to the plugin
                                displayed_window_handle: Arc::new(Mutex::new(None)),
                                
                                // When loading up the plugin make sure to snapshot its settings memory so that we know whats a "default" paramater list to the plugin.
                                startup_memory_snapshot: unsafe { save_state(plugin_callback) },
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
