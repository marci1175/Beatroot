use std::{path::PathBuf, rc::Rc, sync::Arc};

use eframe::{App, CreationContext};
use egui::{Color32, RichText, vec2};

use crate::{
    IS_DEBUG, internals::utils::ExactLengthBuffer, project_manager::open_project, ui::{panels::lib::{Panel, PanelStates, create_panels}, windows::WindowsManager},
};

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

    /// This field indicates which floating windows are enabled (visible).
    #[serde(skip)]
    pub window_mngr: WindowsManager,
}

impl Default for Application {
    fn default() -> Self {
        Self {
            // Store the state of the panels separately
            panel_states: Arc::new(PanelStates::default()),

            // Complete list of all of the panels of the application
            panels: create_panels(),

            // Recently opened project paths
            recently_opened: ExactLengthBuffer::new(10),

            // If no paths were logged then this should be None.
            save_path: None,

            // A struct indicating which windows are enabled
            window_mngr: WindowsManager::default(),
        }
    }
}

impl Application {
    pub fn new(cc: &CreationContext) -> Self {
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }
}

impl App for Application {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn update(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {}

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Create the main options bar
        egui::Panel::top("application_options").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Project").clicked() {}

                    ui.separator();

                    if ui.button("Open").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Beatroot Project", &["btrt"])
                            .pick_file()
                        {
                            // Open the actual project
                            open_project(&path);

                            // Save opened path to recently opened projects
                            // The number of recently opened projects are capped inside the type.
                            self.recently_opened.store(path);
                        }
                    }
                    ui.menu_button("Open Recent", |ui| {
                        ui.allocate_ui(vec2(250., 0.), |ui| {
                            ui.label("Recent Projects");
                            ui.separator();

                            // Display the paths in chronological order
                            for (idx, path) in self.recently_opened.clone().inner().iter().enumerate().rev() {
                                ui.horizontal(|ui| {
                                    if ui
                                        .button(RichText::from(format!("{idx}. {}", path.display())))
                                        .clicked()
                                    {
                                        open_project(path);
                                    }

                                    if ui
                                        .button(RichText::from("Remove").color(Color32::RED))
                                        .clicked()
                                    {
                                        self.recently_opened.remove(idx);
                                    }
                                });
                            }
                        });
                    });

                    ui.separator();

                    if ui.button("Save As").clicked() {}
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
        for panel in self.panels.iter() {
            // Draw/update panel
            panel.display(ui, self.panel_states.clone());

            // If the panel is not detached we can display its toasts in the root ui
            if !panel.detached.load(std::sync::atomic::Ordering::Relaxed) {
                panel.toasts.lock().show(ui);
            }
        }

        // Draw egui windows from window manager
        self.window_mngr.display(ui, Rc::new(&*self));
    }
}
