use std::rc::Rc;

use egui::{InnerResponse, Ui};

use crate::app::Application;

pub fn display_settings_window(ui: &mut Ui, global_state: Rc<&Application>) -> Option<InnerResponse<Option<()>>> {
    let global_state = global_state.clone();
    egui::Window::new("Settings").id("asdasd".into()).show(ui.ctx(), |ui| {
        ui.label(format!("{:?}", global_state.panels[0].id));
    })
}