use egui::{Align2, InnerResponse, RichText, Ui};

use crate::{app::Application, ui::windows::PluginsState};

pub fn display_plugins_window(
    ui: &mut Ui,
    _global_state: &Application,
    _window_state: &mut PluginsState,
) -> Option<InnerResponse<Option<()>>> {
    let screen_size = ui.ctx().viewport_rect().size();

    egui::Window::new("Plugins")
        .fixed_size(screen_size / 2.)
        .collapsible(false)
        .movable(false)
        .anchor(Align2::CENTER_CENTER, [0., 0.])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::from("Available Plugins").strong());
                if ui.button(RichText::from("Refresh").weak()).clicked() {}
            });

            // Separate title from items
            ui.separator();

            // Display available items in the plugins folder
            // Plugins are loaded lazily - theyre loaded at startup or when the user requests a refresh of the list.
            for _plugin in 0..3 {}
        })
}
