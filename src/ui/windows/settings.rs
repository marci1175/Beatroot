use egui::{
    Align2, Color32, InnerResponse, Panel, RichText, ScrollArea, Sense, Ui, UiBuilder, vec2,
};
use strum::{Display, VariantArray};

use crate::{app::Application, ui::windows::SettingsState};

#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    serde::Serialize,
    serde::Deserialize,
    Display,
    PartialEq,
    strum::VariantArray,
)]
pub enum SettingsType {
    #[default]
    General,
    Plugins,
    Playlist,
    Mixer,
    Performance,
}

pub fn display_settings_window(
    ui: &mut Ui,
    global_state: &mut Application,
    window_state: &mut SettingsState,
) -> Option<InnerResponse<Option<()>>> {
    let screen_size = ui.ctx().viewport_rect().size();

    egui::Window::new("Settings")
        .fixed_size(screen_size / 2.)
        .collapsible(false)
        .movable(false)
        .anchor(Align2::CENTER_CENTER, [0., 0.])
        .show(ui.ctx(), |ui| {
            // Tab selector on the side
            Panel::left("settings_tab_selector")
                .resizable(false)
                .show_inside(ui, |ui| {
                    // Display all of the types of settings that are available and highlight the current one.
                    ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for tab in SettingsType::VARIANTS {
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
                    // Create a scrollable area for the specific tab
                    ScrollArea::both()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            match window_state.current_tab {
                                SettingsType::General => {}
                                SettingsType::Plugins => {}
                                SettingsType::Playlist => {
                                    let mut playlist_guard =
                                        global_state.panel_states.playlist_panel.write();

                                    ui.label(RichText::from("Appearance").strong());
                                    ui.horizontal(|ui| {
                                        ui.label("Waveform color");
                                        ui.color_edit_button_srgba(
                                            &mut playlist_guard.playlist_preferences.waveform_color,
                                        );
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Default track label color");
                                        ui.color_edit_button_srgba(
                                            &mut playlist_guard
                                                .playlist_preferences
                                                .default_track_label_color,
                                        );
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Default track text color");
                                        ui.color_edit_button_srgba(
                                            &mut playlist_guard
                                                .playlist_preferences
                                                .default_track_label_text_color,
                                        );
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label("Cursor color");
                                        ui.color_edit_button_srgba(
                                            &mut playlist_guard.playlist_preferences.cursor_color,
                                        );
                                    });
                                }
                                SettingsType::Mixer => {
                                    ui.label(RichText::from("Appearance").strong());
                                    // ui.color_edit_button_rgb(global_state.preferences);
                                }
                                SettingsType::Performance => {}
                            }
                        });
                });
        })
}
