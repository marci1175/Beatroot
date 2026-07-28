use std::{
    any,
    collections::HashMap,
    ffi::c_void,
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use ::vst::api::{AEffect, PluginMain};
use anyhow::anyhow;
use dashmap::DashMap;
use indexmap::IndexMap;
use parking_lot::{Mutex, RwLock};
use strum::Display;
use windows::Win32::{
    Foundation::{FreeLibrary, HMODULE, HWND, LPARAM, WPARAM},
    UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE},
};

use crate::{
    internals::{
        library::{get_fn_addr, load_library},
        mem::str_to_pcwstr,
        windowing::{PluginWindowInformation, create_window, register_class},
    },
    plugins::{
        api::vst2::{AEffectOpcode, ERect, VstOpcode, get_plugin_name, get_vendor_name},
        vst2::{
            host_callback, restore_state, save_state, set_parameter,
        },
    },
    ui::fx_map::NodeMap,
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
pub struct PluginLoadStatus {
    /// The type of plugin this plugin is
    pub plugin_type: PluginType,

    /// Status of the plugin, this is used when loading in a library/plugin.
    pub status: PluginHandleStatus,
}

#[derive(
    PartialEq, Eq, Hash, Debug, Copy, Clone, serde::Deserialize, serde::Serialize, Default, Display,
)]
///
/// The type of the plugin. This selects the logic the plugin is used with.
/// NOTICE: Verify the type of the plugin since it uses unsafe code, and if a plugin is misrepresented it will lead to UB.
///
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
///
/// A PluginHandle can be used to spawn miltiple instances of the same plugin. ([`PluginInstance`])
/// Only one plugin handle can exist of one plugin loaded. (So one plugin handle represents one .dll)
///
pub struct PluginHandle {
    /// A pointer to the main entrypoint of the plugin.
    /// This can be used to spawn new instances of the sample plugin.
    pub plugin_entry_fn_ptr: *mut usize,

    /// The type of the plugin
    pub plugin_type: PluginType,

    /// Handle to the loaded library in memory.
    pub library_handle: HMODULE,

    /// Every plugin has its memory snapshotted at startup to know what should a valid "default" paramter list should look like.
    /// This is used as a default setting for the plugin.
    pub startup_memory_snapshot: Vec<u8>,

    /// Plugin instances created from this [`PluginHandle`].
    pub tracked_instances: Arc<Mutex<Vec<PluginInstance>>>,

    pub info: Arc<PluginInformation>,
}

impl PluginHandle {
    ///
    /// Stops all tracked instances of the plugin and free the plugin from memory.
    ///
    pub fn destroy(self) -> anyhow::Result<()> {
        // Close all tracked instances of the plugin before deallocating.
        for inst in &*self.tracked_instances.lock() {
            // Close plugin
            inst.close()?;
        }

        // Free library after everything has been closed
        unsafe { FreeLibrary(self.library_handle)? };

        Ok(())
    }

    pub fn create_instance(&self) -> PluginInstance {
        match self.plugin_type {
            PluginType::Vst2 => {
                // SAFETY: This function signature is transmuted based on the official SDK of VST 2.4.
                let plugin_entry: PluginMain =
                    unsafe { std::mem::transmute(self.plugin_entry_fn_ptr) };

                // Call the main plugin entry passing the host callback
                // Create a temporary instance of the plugin to get the "default" parameters of the plugin
                let instance_callback = (plugin_entry)(host_callback);

                PluginInstance {
                    plugin_instance_ptr: instance_callback as *mut _,
                    plugin_type: self.plugin_type,
                    displayed_window_information: Arc::new(Mutex::new(None)),
                    info: self.info.clone(),
                }
            }
            PluginType::Vst3 => todo!(),
            PluginType::Clap => todo!(),
            PluginType::Lua => todo!(),
        }
    }
}

impl PartialEq for PluginHandle {
    fn eq(&self, other: &Self) -> bool {
        self.plugin_entry_fn_ptr == other.plugin_entry_fn_ptr
            && self.plugin_type == other.plugin_type
            && self.library_handle == other.library_handle
    }
}

#[derive(Debug)]
pub struct PluginInformation {
    pub name: String,
    pub vendor: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
///
/// A plugin instance is an instance created by a plugin handle. Multiple plugin instances can exist at the same time created by one [`PluginHandle`].
///
pub struct PluginInstance {
    /// Pointer to the handler struct of this plugin.
    /// The type of the plugin decides how this pointer is worked with.
    ///
    /// SAFETY: Ensure that the memory is not deallocated where this pointer is pointing to.
    ///
    /// PluginType casts:
    /// - VST2: ```*mut AEffect```
    pub plugin_instance_ptr: *mut usize,

    /// The type of the plugin
    pub plugin_type: PluginType,

    ///
    /// The window's handle and the node which was used to open it up if the plugin is being displayed.
    /// This is used to prevent opening up multiple windows to the same plugin and as handle to the window we need to destroy when removing the plugin.
    ///
    /// This also provides information to the host to know which sample's node's state to modify.
    ///
    pub displayed_window_information: Arc<Mutex<Option<PluginWindowInformation>>>,

    /// Information about the plugin itself. (This is created cloned from the plugin handle.)
    pub info: Arc<PluginInformation>,
}

impl PluginInstance {
    pub fn load_state(&self, state: &[u8]) {
        match self.plugin_type {
            PluginType::Vst2 => unsafe {
                restore_state(self.plugin_instance_ptr as *mut _, state);
            },
            PluginType::Vst3 => todo!(),
            PluginType::Clap => todo!(),
            PluginType::Lua => todo!(),
        }
    }

    pub fn save_state(&self) -> Vec<u8> {
        match self.plugin_type {
            PluginType::Vst2 => unsafe { save_state(self.plugin_instance_ptr as *mut _) },
            PluginType::Vst3 => todo!(),
            PluginType::Clap => todo!(),
            PluginType::Lua => todo!(),
        }
    }

    pub fn change_paramter(
        &self,
        param_index: usize,
        value: Box<dyn any::Any>,
    ) -> anyhow::Result<()> {
        match self.plugin_type {
            PluginType::Vst2 => {
                unsafe {
                    set_parameter(
                        self.plugin_instance_ptr as *mut _,
                        param_index as i32,
                        *(value
                            .downcast::<f32>()
                            .map_err(|_| anyhow!("Invalid parameter provided."))?),
                    )
                };
            }
            PluginType::Vst3 => todo!(),
            PluginType::Clap => todo!(),
            PluginType::Lua => todo!(),
        };

        Ok(())
    }

    /// Closes the plugin's window.
    /// This does not free the library in order to deallocate the whole library, [`PluginHandle::destroy()`] should be called instead.
    pub fn close(&self) -> anyhow::Result<()> {
        // Close based on plugin type.
        match self.plugin_type {
            PluginType::Vst2 => {
                let effect = self.plugin_instance_ptr as *mut AEffect;
                let dispatcher = unsafe { effect.read() }.dispatcher;

                // Destroy window if open
                if let Some(window_info) = *self.displayed_window_information.lock() {
                    // Recast the usize to a hwnd
                    let hwnd = HWND(window_info.window_hwnd as *mut c_void);

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

        Ok(())
    }

    ///
    /// Open a plugin to the GUI in the application.
    /// This creates a separate window from the original host window, which always stays on top.
    /// We need to provide additional information so that we know which node's state we are modifying exactly.
    ///
    pub fn open(
        &self,
        state: Arc<RwLock<Vec<u8>>>,
        node_id: usize,
        sample_id: usize,
    ) -> anyhow::Result<()> {
        // Clone the window handle so that it can be modified from the other thread
        let window_info = self.displayed_window_information.clone();

        // Load the plugin's state
        self.load_state(&state.read());

        // Match the pulgin type and display appropriately
        match self.plugin_type {
            PluginType::Vst2 => {
                // We cast to usize because a *mut pointer does not implement Send.
                let plugin_handle_ptr = self.plugin_instance_ptr as usize;
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

                // Create PCWSTR from effect name string
                let (name, _bytes) = str_to_pcwstr(&self.info.name);

                // Create class for window
                let class_name = register_class(name).unwrap();

                // Clone the handle to the window
                let window_handle_clone = window_info.clone();

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
                    plugin_instance: self.clone(),
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

                // Provide the window information
                *window_info.lock() = Some(PluginWindowInformation {
                    window_hwnd: hwnd.0 as usize,
                    node_id,
                    sample_id,
                });

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

#[derive(Debug, Clone)]
pub struct InstanceResult(Result<PluginInstance, PluginInstanceStatus>);

impl InstanceResult {
    pub fn get(&self) -> Result<&PluginInstance, &PluginInstanceStatus> {
        self.0.as_ref()
    }

    pub fn new(plugin_instance: PluginInstance) -> Self {
        Self(Ok(plugin_instance))
    }
}

impl Default for InstanceResult {
    fn default() -> Self {
        Self(Err(PluginInstanceStatus::Unloaded))
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
    /// The plugin instance's handle that this window is for.
    pub plugin_instance: PluginInstance,
    /// The handle of the state buffer for the plugin.
    /// The reason this is atomic is that multiple threads can write and read this entry.
    pub state_handle: Arc<RwLock<Vec<u8>>>,
}

unsafe impl Send for PluginHandle {}
unsafe impl Sync for PluginHandle {}
unsafe impl Send for PluginInstance {}
unsafe impl Sync for PluginInstance {}

#[derive(
    Hash, Debug, Clone, Copy, serde::Deserialize, serde::Serialize, Default, PartialEq, Eq, Display,
)]
pub enum PluginHandleStatus {
    #[default]
    Ok,
    FileNotFound,
    PluginEntryNotFound,
}

#[derive(
    Hash, Debug, Clone, Copy, serde::Deserialize, serde::Serialize, Default, PartialEq, Eq, Display,
)]
pub enum PluginInstanceStatus {
    #[default]
    Unloaded,
    NotFound,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
/// Contains all nescesarry information to try to load in or refer to a plugin.
pub struct PluginDescriptor {
    /// Path to the plugin
    pub path: PathBuf,

    /// The type of the plugin.
    /// This is set by the user abut it should be validated.
    pub plugin_type: PluginType,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct PluginManager {
    /// The saved path to the plugins we want to load in at startup or during runtime.
    pub plugin_loaders: IndexMap<PathBuf, PluginLoadStatus>,

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
            PluginLoadStatus {
                plugin_type,
                status: crate::plugins::PluginHandleStatus::Ok,
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
                        // Create a temporary instance of the plugin to get the "default" parameters of the plugin
                        let plugin_callback = (plugin_entry)(host_callback);

                        // Fetch some information about the plugin
                        let name = unsafe { get_plugin_name(plugin_callback) };
                        let vendor = unsafe { get_vendor_name(plugin_callback) };

                        // Store plugin
                        self.loaded_plugins.insert(
                            path.clone(),
                            PluginHandle {
                                // The pointer to the plugin's handler
                                plugin_entry_fn_ptr: plugin_entry as *mut _,

                                // The plugins type
                                plugin_type: loader.plugin_type,

                                // The raw dll module handle
                                library_handle: module_handle,

                                // When loading up the plugin make sure to snapshot its settings memory so that we know whats a "default" paramater list to the plugin.
                                startup_memory_snapshot: unsafe { save_state(plugin_callback) },

                                // Create a list where all of the newly created instances get inserted into.
                                tracked_instances: Arc::new(Mutex::new(Vec::new())),

                                info: Arc::new(PluginInformation {
                                    name,
                                    vendor,
                                    path: path.clone(),
                                }),
                            },
                        );

                        // Close this instance of the plugin
                        // We dont need this instance of the plugin anymore, we only used it to create the startup snapshot
                        ((unsafe { &*plugin_callback }).dispatcher)(
                            plugin_callback,
                            VstOpcode::Close.as_i32(),
                            0,
                            0,
                            std::ptr::null_mut(),
                            0.0,
                        );
                    } else {
                        loader.status = PluginHandleStatus::PluginEntryNotFound;
                    }
                }
                PluginType::Vst3 => {}
                PluginType::Clap => {}
                PluginType::Lua => {}
            }
        } else {
            // Set the plugins state to not found.
            loader.status = PluginHandleStatus::FileNotFound;
        }
    }
}

pub fn create_plugin_state_writer(
    _plugin_manager: Arc<RwLock<PluginManager>>,
    _fx_map: Arc<DashMap<usize, NodeMap>>,
) {
    // std::thread::spawn(move || {
    //     loop {
    //         // Update stored plugin states when the plugin is modified.
    //         // Do not block waiting for a plugin to write into the queue while the mutex is locked.
    //         if let Ok(param) = PARAMETER_CHANNEL.1.recv() {
    //             // Check which plugin pushed to the parameter queue
    //             if let Some(handle) = plugin_manager
    //                 .read()
    //                 .loaded_plugins
    //                 .get_key2(&param.plugin_pointer)
    //             {
    //                 // Check if the plugin that inserted in the parameter queue is open, and what information was it provided with.
    //                 // This way we will know what node opened the window from the plugin window information.
    //                 if let Some(Some(window_information)) = handle
    //                     .displayed_window_information
    //                     .try_lock_for(Duration::from_secs(2))
    //                     .as_deref()
    //                 {
    //                     // This is the node which was representing the plugin which was modified
    //                     let modified_node =
    //                         fx_map
    //                             .get(&window_information.sample_id)
    //                             .and_then(|nodemap| {
    //                                 nodemap.nodes().get(window_information.node_id).cloned()
    //                             });

    //                     if let Some(node) = modified_node {
    //                         // Modify the locally stored state based on the plugin type
    //                         match node.node_type() {
    //                             crate::ui::fx_map::NodeType::In
    //                             | crate::ui::fx_map::NodeType::Out => (),
    //                             crate::ui::fx_map::NodeType::ExternalPlugin { state, .. } => {
    //                                 // Get the plugin's type
    //                                 match handle.plugin_type {
    //                                     crate::plugins::PluginType::Vst2 => {
    //                                         // Get the locally stored plugin state
    //                                         let plugin_state = &mut *state.write();

    //                                         // Set the parameter's state inside the locally stored parameter buffer.
    //                                         set_parameter_in_state(
    //                                             plugin_state,
    //                                             param.index as usize,
    //                                             param.value,
    //                                         );
    //                                     }
    //                                     crate::plugins::PluginType::Vst3 => todo!(),
    //                                     crate::plugins::PluginType::Clap => todo!(),
    //                                     crate::plugins::PluginType::Lua => todo!(),
    //                                 }
    //                             }
    //                             crate::ui::fx_map::NodeType::InternalCustom(
    //                                 _plugin_node_properties,
    //                             ) => todo!(),
    //                         }
    //                     }
    //                 }
    //             }
    //         }
    //     }
    // });
}
