use std::sync::Arc;

use egui::{Align2, Color32, InnerResponse, Panel, RichText, ScrollArea, Sense, Ui};
use strum::{Display, VariantArray};

use crate::{
    VALID_TIME_SIG_DENOMINATORS,
    app::Application,
    audio::host::{HOST_STATE, HostInformation},
    ui::windows::SettingsState,
};

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
    Project,
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
                .show(ui, |ui| {
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
                    ScrollArea::both().auto_shrink([false, false]).show(
                        ui,
                        |ui| match window_state.current_tab {
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
                            SettingsType::Mixer => {}
                            SettingsType::Performance => {}
                            SettingsType::Project => {
                                ui.label(RichText::from("Audio Settings").strong());

                                ui.horizontal(|ui| {
                                    // Get the current host's state
                                    let current_host_state = HOST_STATE.load();

                                    // Current sample rate, compare the modifiable sample rate to the original value
                                    let original_sample_rate = current_host_state.sample_rate;

                                    // Create a mutable copy of the original sample rate so that we can modify this one.
                                    let mut current_sample_rate = original_sample_rate;

                                    egui::ComboBox::from_label("Sampling Rate")
                                        .selected_text(format!("{:?}", current_sample_rate))
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut current_sample_rate,
                                                32000,
                                                "32000hz",
                                            );
                                            ui.selectable_value(
                                                &mut current_sample_rate,
                                                44100,
                                                "44100hz",
                                            );
                                            ui.selectable_value(
                                                &mut current_sample_rate,
                                                48000,
                                                "48000hz",
                                            );
                                            ui.selectable_value(
                                                &mut current_sample_rate,
                                                88200,
                                                "88200hz",
                                            );
                                            ui.selectable_value(
                                                &mut current_sample_rate,
                                                96000,
                                                "96000hz",
                                            );
                                            ui.selectable_value(
                                                &mut current_sample_rate,
                                                176400,
                                                "176400hz",
                                            );
                                            ui.selectable_value(
                                                &mut current_sample_rate,
                                                192000,
                                                "192000hz",
                                            );
                                        });

                                    // Handle selection change
                                    if current_sample_rate != original_sample_rate {
                                        // Modify the current host state
                                        HOST_STATE.store(Arc::new(HostInformation::new(
                                            current_sample_rate,
                                            current_host_state.channel_count,
                                        )));
                                    }
                                });

                                ui.label(RichText::from("Timing"));

                                let time_sig_numerator = global_state
                                    .panel_states
                                    .playlist_panel
                                    .read()
                                    .time_signature_numerator
                                    .clone();
                                let time_signature_denominator = global_state
                                    .panel_states
                                    .playlist_panel
                                    .read()
                                    .time_signature_denominator
                                    .clone();

                                ui.add(
                                    egui::Slider::new(&mut *time_sig_numerator.lock(), 1..=99)
                                        .prefix("Time Signature Numerator"),
                                );

                                let time_signature_denominator =
                                    &mut *time_signature_denominator.lock();

                                egui::ComboBox::from_label("Time Signature Denominator")
                                    .selected_text(time_signature_denominator.to_string())
                                    .show_ui(ui, |ui| {
                                        for valid_denom in VALID_TIME_SIG_DENOMINATORS {
                                            ui.selectable_value(
                                                time_signature_denominator,
                                                *time_signature_denominator,
                                                valid_denom.to_string(),
                                            );
                                        }
                                    });
                            }
                        },
                    );
                });
        })
}
