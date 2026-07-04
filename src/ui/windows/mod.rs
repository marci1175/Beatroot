use crate::app::Application;
use crate::ui::windows::{
    help::display_help_window, plugins::display_plugins_window, settings::display_settings_window,
};

pub mod help;
pub mod plugins;
pub mod settings;

/// This macro creates a WindowsManager struct which automatically implements a display function which calls the associated function with the window if the window is enabled.
macro_rules! create_window_states {
    ($visibility:vis, $($window_name:ident ($draw_window:path) => { $($state_field:ident : $state_ty:ty),* }),*) => {
        paste::paste! {
            #[derive(Default, Debug)]
            $visibility struct WindowsManager {
                $(
                    $visibility [<$window_name:lower>]: bool,
                    $visibility [<$window_name:lower _state>]: [<$window_name State>],
                )*
            }

            impl WindowsManager {
                /// Displays all enabled windows.
                /// Every function is called with three arguments, that being the ui, global application state and the window's internal state.
                $visibility fn display(&mut self, ui: &mut egui::Ui, global_state: &mut Application) {
                    $(
                        if self.[<$window_name:lower>] {
                            $draw_window(ui, global_state, &mut self.[<$window_name:lower _state>]);
                        }
                    )*
                }
            }

            $(
                #[derive(Default, Debug)]
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
create_window_states! (pub, Settings (display_settings_window) => { current_tab: settings::SettingsType }, Plugins (display_plugins_window) => {  }, Help (display_help_window) => {  });
