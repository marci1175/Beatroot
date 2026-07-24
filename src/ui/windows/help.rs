use egui::{InnerResponse, RichText, Ui};
use crate::{app::Application, ui::windows::HelpState};

pub fn display_help_window(
    ui: &mut Ui,
    _global_state: &Application,
    _window_state: &mut HelpState,
) -> Option<InnerResponse<Option<()>>> {
    egui::Window::new("Help").show(ui.ctx(), |ui| {
        ui.label("Build information");
        ui.label(format!("Version: {} {}", env!("CARGO_PKG_VERSION"), {
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        }));
        ui.separator();
        ui.hyperlink_to("API documentation", "https://www.google.com");
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::from("Checking for updates....").weak());
            ui.spinner();
        });
    })
}
