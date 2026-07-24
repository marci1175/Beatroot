use crate::{app::Application, internals::endpoint::check_for_update, ui::windows::HelpState};
use egui::{Color32, InnerResponse, RichText, Ui};

pub fn display_help_window(
    ui: &mut Ui,
    global_state: &Application,
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

        ui.label(RichText::from("Updates").size(20.).strong());

        // Clone the status to avoid a deadlock
        let status = global_state
            .update_available
            .lock()
            .as_ref()
            .map(|r| match r {
                Ok(b) => Ok(*b),
                Err(e) => Err(e.to_string()),
            });

        // Inform user if an update is available
        match status {
            Some(fetch_res) => {
                match fetch_res {
                    Ok(result) => {
                        if result {
                            // Redirect user to releases
                            ui.hyperlink_to(
                                RichText::from("Update available!").color(Color32::GREEN),
                                "https://github.com/marci1175/Beatroot/releases",
                            );
                        } else {
                            ui.label("No updates available.");
                        }
                    }
                    Err(result) => {
                        ui.label(RichText::from(result.to_string()).color(Color32::RED));
                    }
                }
            }
            None => {
                ui.horizontal(|ui| {
                    ui.label(RichText::from("Checking for updates....").weak());
                    ui.spinner();

                    if ui.button("Retry").clicked() {
                        check_for_update(global_state.update_available.clone());
                    }
                });
            }
        }
    })
}
