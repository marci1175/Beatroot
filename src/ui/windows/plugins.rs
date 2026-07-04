use egui::{InnerResponse, Ui};

use crate::{app::Application, ui::windows::PluginsState};

pub fn display_plugins_window(
    ui: &mut Ui,
    global_state: &Application,
    _window_state: &mut PluginsState,
) -> Option<InnerResponse<Option<()>>> {
    egui::Window::new("Plugins")
        .id("asdasd".into())
        .show(ui.ctx(), |ui| {
            ui.label(format!("{:?}", global_state.panels[0].id));
        })
}
