use egui::{Align2, Color32, InnerResponse, Panel, RichText, ScrollArea, Sense, Ui};
use strum::{Display, VariantArray};

use crate::{app::Application, ui::windows::PluginsState};

#[derive(Display, Debug, Default, Clone, Copy, strum::VariantArray, PartialEq)]
pub enum PluginTabType {
    Imported,
    #[default]
    Loaded,
}

pub fn display_plugins_window(
    ui: &mut Ui,
    _global_state: &Application,
    window_state: &mut PluginsState,
) -> Option<InnerResponse<Option<()>>> {
    let screen_size = ui.ctx().viewport_rect().size();

    egui::Window::new("Plugins")
        .fixed_size(screen_size / 2.)
        .collapsible(false)
        .movable(false)
        .anchor(Align2::CENTER_CENTER, [0., 0.])
        .show(ui.ctx(), |ui| {
            Panel::left("plugin_tab_selector")
                .resizable(false)
                .show_inside(ui, |ui| {
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
                    ScrollArea::both().auto_shrink([false, false]).show(
                        ui,
                        |_ui| match window_state.current_tab {
                            PluginTabType::Imported => {}
                            PluginTabType::Loaded => {}
                        },
                    );
                });
        })
}
