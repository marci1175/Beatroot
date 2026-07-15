use std::{path::PathBuf, sync::Arc};

use eframe::{App, CreationContext};
use egui::{Color32, RichText, vec2};

use crate::{
    audio::{
        lib::{AudioThreadHandler, HostAudioPlayback, create_playback_thread},
        playback::{HostInformation, MasterPlaybackThread},
    },
    internals::utils::ExactLengthBuffer,
    plugins::PluginManager,
    project_manager::open_project,
    ui::{
        panels::lib::{Panel, PanelStates, create_panels},
        windows::WindowsManager,
    },
};

#[derive(serde::Serialize, serde::Deserialize, Default)]
pub struct AppRoot {
    /// This field indicates which floating windows are enabled (visible) and the state of the settings window.
    /// Note that by state I mean the literal state the settings windows are not the actual settings they are modifiying.
    #[serde(skip)]
    pub window_mngr: WindowsManager,

    /// Every component in the application exists in the Application struct.
    pub application: Application,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Application {
    /// The state of the panels inside, every panel state is accessible from the other one.
    pub panel_states: Arc<PanelStates>,

    /// The list of panels that are present in the application.
    pub panels: Vec<Panel>,

    /// Recently opened project's paths
    pub recently_opened: ExactLengthBuffer<PathBuf>,

    /// If the user has saved a project or opened an existing one this path will point to that file which has been opened.
    pub save_path: Option<PathBuf>,

    #[serde(skip)]
    /// The audio handler is a set of channels and atomic data which ensures audio runs on a different thread than main and that both are syncronized.
    /// This thread is used for playing back individual samples. This is only for simple audio playback.
    pub sample_audio_handler: Arc<AudioThreadHandler>,

    /// The path to the plugins which are loaded at startup, or when the user instructs the application to do so.
    pub plugin_manager: PluginManager,

    #[serde(skip)]
    /// Audio handler for the playlist. This is where most of the computing power is. This handles effects, mixing, etc. to produce the final result when playing back audio.
    pub master_playback_handler: Arc<MasterPlaybackThread>,
}

impl Default for Application {
    fn default() -> Self {
        // Create the host's audio handle
        let host_audio = Arc::new(
            HostAudioPlayback::new().expect("Failed to acquire host audio playback handle."),
        );

        // Create audio playback thread, this thread is only for previewing samples and playing back simple samples.
        // This is not the main playlist playbacker.
        let playback_thread_handler = create_playback_thread(host_audio.clone())
            .expect("Failed to create audio playback thread.");

        // Get config of host
        let host_cfg = host_audio.sink.config();

        // Create master playback handler
        // This runs the more complicated playbacks and handles the playlist's playback.
        let master_playback_handler = MasterPlaybackThread::new(
            HostInformation {
                sample_rate: host_cfg.sample_rate().get(),
                channel_count: host_cfg.channel_count().get(),
            },
            host_audio.sink.mixer().clone(),
        )
        .expect("Failed to create master playback thread.");

        Self {
            // Store the state of the panels separately
            panel_states: Arc::new(PanelStates::default()),

            // Complete list of all of the panels of the application
            panels: create_panels(),

            // Recently opened project paths
            recently_opened: ExactLengthBuffer::new(10),

            // If no paths were logged then this should be None.
            save_path: None,

            plugin_manager: PluginManager::default(),

            // If there was no audio handler added then just handle it with None.
            sample_audio_handler: Arc::new(playback_thread_handler),

            master_playback_handler: Arc::new(master_playback_handler),
        }
    }
}

impl AppRoot {
    pub fn new(cc: &CreationContext) -> Self {
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }
}

impl App for AppRoot {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Create the main options bar
        egui::Panel::top("application_options").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    ui.button("New Project").clicked();

                    ui.separator();

                    if ui.button("Open").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Beatroot Project", &["btrt"])
                            .pick_file()
                    {
                        // Open the actual project
                        open_project(&path);

                        // Save opened path to recently opened projects
                        // The number of recently opened projects are capped inside the type.
                        self.application.recently_opened.store(path);
                    }
                    ui.menu_button("Open Recent", |ui| {
                        ui.allocate_ui(vec2(250., 0.), |ui| {
                            ui.label("Recent Projects");
                            ui.separator();

                            // Display the paths in chronological order
                            for (idx, path) in self
                                .application
                                .recently_opened
                                .clone()
                                .inner()
                                .iter()
                                .enumerate()
                                .rev()
                            {
                                ui.horizontal(|ui| {
                                    if ui
                                        .button(RichText::from(format!(
                                            "{idx}. {}",
                                            path.display()
                                        )))
                                        .clicked()
                                    {
                                        open_project(path);
                                    }

                                    if ui
                                        .button(RichText::from("Remove").color(Color32::RED))
                                        .clicked()
                                    {
                                        self.application.recently_opened.remove(idx);
                                    }
                                });
                            }
                        });
                    });

                    ui.separator();

                    ui.button("Save As").clicked();
                    if ui.button("Save").clicked() {}
                });

                ui.menu_button("View", |_ui| {});

                if ui.button("Plugins").clicked() {
                    self.window_mngr.plugins = !self.window_mngr.plugins;
                }

                if ui.button("Settings").clicked() {
                    self.window_mngr.settings = !self.window_mngr.settings;
                }

                if ui.button("Help").clicked() {
                    self.window_mngr.help = !self.window_mngr.help;
                }
            });
        });

        // Draw detachable panels
        for panel in self.application.panels.iter() {
            // Draw/update panel
            panel.display(
                ui,
                self.application.panel_states.clone(),
                self.application.sample_audio_handler.clone(),
                self.application.master_playback_handler.clone(),
            );

            // If the panel is not detached we can display its toasts in the root ui
            if !panel.detached.load(std::sync::atomic::Ordering::Relaxed) {
                panel.toasts.lock().show(ui);
            }
        }

        // Draw egui windows from window manager
        self.window_mngr.display(ui, &mut self.application);
    }
}
