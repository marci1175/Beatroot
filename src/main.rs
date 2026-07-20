use std::panic;

use beatroot::{APP_NAME, app::AppRoot, internals::mem::str_to_pcwstr};
use eframe::{NativeOptions, wgpu};
use windows::{
    Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
    core::w,
};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    panic::set_hook(Box::new(|info| {
        let payload = info.payload_as_str().unwrap_or_default();

        let time = chrono::Utc::now();
        let current_time = time.timestamp();
        let file_name = format!("error_{}.log", current_time);
        let path = format!(
            "{}\\Beatroot\\{file_name}",
            std::env::var("APPDATA").unwrap()
        );
        let panic_message =
            format!("Panic occured: {payload}.\nThis error has been logged to `{path}`.");

        // Try writing full report to disk
        std::fs::write(
            path,
            format!(
                "Message: {panic_message}\nTime: {current_time}\nLocation: {}\nVersion: {}\nBuild Timestamp: {}\nGit Hash: {}",
                info.location()
                    .map(|loc| loc.to_string())
                    .unwrap_or("INVALID".to_string()),
                env!("CARGO_PKG_VERSION"),
                env!("BUILD_TIMESTAMP"),
                env!("GIT_HASH"),
            ),
        )
        .unwrap_or_default();

        // Display error to user
        unsafe {
            MessageBoxW(
                None,
                str_to_pcwstr(&panic_message).0,
                w!("Panic!"),
                MB_ICONERROR | MB_OK,
            );
        }
    }));

    let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle_from_env();
    instance_descriptor.backends = wgpu::Backends::PRIMARY;

    let mut wgpu_setup_create_new = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    wgpu_setup_create_new.instance_descriptor = instance_descriptor;

    let native_options = NativeOptions {
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(wgpu_setup_create_new),
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(|cc| Ok(Box::new(AppRoot::new(cc)))),
    )?;

    Ok(())
}
