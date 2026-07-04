use crate::ui::windows::settings::display_settings_window;
use std::rc::Rc;
use crate::app::Application;

pub mod settings;
pub mod plugins;

/// This macro creates a WindowsManager struct which automatically implements a display function which calls the associated function with the window if the window is enabled.
macro_rules! create_window_states {
    ($visibility:vis, $($window_name:ident ($draw_window:path) => { $($state_field:ident : $state_ty:ty),* }),*) => {
        paste::paste! {
            #[derive(Default, Debug)]
            $visibility struct WindowsManager {
                $(
                    $visibility [<$window_name:lower>]: bool,
                )*
            }

            impl WindowsManager {
                /// Displays all enabled windows.
                /// Every function is called with two arguments, that being the ui and the global application state.
                $visibility fn display(&self, ui: &mut egui::Ui, global_state: Rc<&Application>) {
                    $(
                        if self.[<$window_name:lower>] {
                            $draw_window(ui, global_state.clone());
                        }
                    )*
                }
            }

            $(
                    $visibility struct [<$window_name State>] {
                        $(
                            $visibility $state_field: $state_ty,
                        )*
                    }
            )*
        }
    };
}

// Create windows for different parts of the application
create_window_states! (pub, Settings (display_settings_window) => {  }, Plugins (display_settings_window) => {  }, Help (display_settings_window) => {  });