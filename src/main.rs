use beatroot::{APP_NAME, app::AppRoot, audio::lib::create_playback_thread};
use eframe::NativeOptions;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create audio playback thread
    let thread_handler = create_playback_thread()?;

    let native_options = NativeOptions {
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(|cc| Ok(Box::new(AppRoot::new(cc, thread_handler)))),
    )?;

    Ok(())
}
