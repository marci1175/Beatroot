/*
ui.label("Build information");
                    ui.label(format!("Build: {}{}", env!("CARGO_PKG_VERSION"), {
                        if IS_DEBUG { "debug" } else { "release" }
                    }));
                    ui.separator();
                    ui.hyperlink_to("API documentation", "https://www.google.com")
                     */

use egui::{InnerResponse, Ui};

use crate::{app::Application, ui::windows::HelpState};

pub fn display_help_window(
    ui: &mut Ui,
    global_state: &Application,
    _window_state: &mut HelpState,
) -> Option<InnerResponse<Option<()>>> {
    egui::Window::new("Help").show(ui.ctx(), |ui| {
        ui.label(format!("{:?}", global_state.panels[0].id));
    })
}
