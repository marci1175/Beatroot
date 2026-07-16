use std::path::PathBuf;

use egui::{Align2, Color32, InnerResponse, Panel, RichText, ScrollArea, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use egui_toast::ToastStyle;
use strum::{Display, VariantArray};

use crate::{
    app::Application,
    ui::{panels::lib::display_error_as_toast, windows::PluginsState},
};

#[derive(Display, Debug, Default, Clone, Copy, strum::VariantArray, PartialEq)]
pub enum PluginTabType {
    Import,
    #[default]
    Imported,
    Loaded,
}

pub fn display_plugins_window(
    ui: &mut Ui,
    global_state: &mut Application,
    window_state: &mut PluginsState,
) -> Option<InnerResponse<Option<()>>> {
    let screen_size = ui.ctx().viewport_rect().size();
    let plugin_loaders = global_state.plugin_manager.read().plugin_loaders.clone();

    egui::Window::new("Plugins")
        .fixed_size(screen_size / 2.)
        .collapsible(false)
        .movable(false)
        .anchor(Align2::CENTER_CENTER, [0., 0.])
        .show(ui.ctx(), |ui| {
            Panel::left("plugin_tab_selector")
                .resizable(false)
                .show(ui, |ui| {
                    // Display all of the types of settings that are available and highlight the current one.
                    ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for tab in PluginTabType::VARIANTS {
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::from(tab.to_string()).color(Color32::WHITE),
                                        )
                                        .fill(
                                            // If the button is selected
                                            if &window_state.current_tab == tab {
                                                Color32::GRAY
                                            }
                                            // If its not selected just leave the bg as is
                                            else {
                                                Color32::TRANSPARENT
                                            },
                                        ),
                                    )
                                    .interact(Sense::click())
                                    .clicked()
                                {
                                    window_state.current_tab = *tab;
                                }
                            }
                        });
                });

            egui::Frame::NONE
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    match window_state.current_tab {
                        PluginTabType::Import => {
                            ui.label("Import Plugin");

                            ui.horizontal(|ui| {
                                // Create the import button
                                if ui.button("Import").clicked() {
                                    // Create file dialog for supported extensions
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter(
                                            "Plugin",
                                            &[match window_state.plugin_type {
                                                crate::plugins::PluginType::Vst2 => "dll",
                                                crate::plugins::PluginType::Vst3 => "vst3",
                                                crate::plugins::PluginType::Clap => "clap",
                                                crate::plugins::PluginType::Lua => "lua",
                                            }],
                                        )
                                        .pick_file()
                                    {
                                        global_state
                                            .plugin_manager
                                            .write()
                                            .store_plugin(path, window_state.plugin_type);
                                    }
                                }

                                egui::ComboBox::from_label("Plugin Type")
                                    .selected_text(window_state.plugin_type.to_string())
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(
                                            &mut window_state.plugin_type,
                                            crate::plugins::PluginType::Vst2,
                                            "Vst2",
                                        );
                                        ui.selectable_value(
                                            &mut window_state.plugin_type,
                                            crate::plugins::PluginType::Vst3,
                                            "Vst3",
                                        );
                                        ui.selectable_value(
                                            &mut window_state.plugin_type,
                                            crate::plugins::PluginType::Clap,
                                            "Clap",
                                        );
                                        ui.selectable_value(
                                            &mut window_state.plugin_type,
                                            crate::plugins::PluginType::Lua,
                                            "Lua",
                                        );
                                    });
                            });
                        }
                        PluginTabType::Imported => {
                            egui::ScrollArea::horizontal()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    // Display all the imported plugins in a grid
                                    TableBuilder::new(ui)
                                        .striped(true)
                                        .column(Column::auto().resizable(true))
                                        .column(Column::auto().resizable(true))
                                        .column(Column::remainder())
                                        .column(Column::remainder())
                                        .header(24.0, |mut header| {
                                            header.col(|ui| {
                                                ui.label("Name");
                                            });
                                            header.col(|ui| {
                                                ui.label("Type");
                                            });
                                            header.col(|ui| {
                                                ui.label("Path");
                                            });
                                            header.col(|ui| {
                                                ui.label("Status");
                                            });
                                        })
                                        .body(|body| {
                                            body.rows(20., plugin_loaders.len(), |mut row| {
                                                let (path, plugin) =
                                                    plugin_loaders.get_index(row.index()).unwrap();

                                                row.col(|ui| {
                                                    ui.label(
                                                        path.file_name()
                                                            .unwrap_or_default()
                                                            .to_string_lossy(),
                                                    );
                                                });
                                                row.col(|ui| {
                                                    ui.label(plugin.plugin_type.to_string());
                                                });
                                                row.col(|ui| {
                                                    ui.label(path.to_string_lossy());
                                                });
                                                row.col(|ui| {
                                                    ui.label(plugin.status.to_string());
                                                });
                                            });
                                        });
                                });
                        }
                        PluginTabType::Loaded => {
                            egui::ScrollArea::horizontal()
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let plugin_manager = &mut global_state.plugin_manager.write();

                                    // If a plugin was removed this is a Some.
                                    let mut removed_plugin: Option<PathBuf> = None;

                                    plugin_manager.loaded_plugins.retain(|path, handle| {
                                        ui.horizontal(|ui| {
                                            // Show the name of the plugin
                                            ui.label(
                                                path.file_name()
                                                    .unwrap_or_default()
                                                    .to_string_lossy(),
                                            );

                                            // Display remove button
                                            if ui.button("Unload and Remove").clicked() {
                                                // Do not reload this plugin at startup
                                                removed_plugin = Some(path.clone());

                                                // Try to free the library and display if any error occured
                                                display_error_as_toast(
                                                    handle.destroy(),
                                                    ToastStyle::default(),
                                                    global_state.toasts.clone(),
                                                );

                                                // Remove this entry from the loaded plugin's list
                                                false
                                            } else {
                                                true
                                            }
                                        })
                                        .inner
                                    });

                                    // If a plugin has been stored as removed remove it from the application too so that its not reloaded at startup.
                                    if let Some(path) = removed_plugin {
                                        plugin_manager.plugin_loaders.swap_remove(&path);
                                    }
                                });
                        }
                    }
                });
        })
}
