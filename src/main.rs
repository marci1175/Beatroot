use beatroot::{
    APP_NAME,
    app::AppRoot,
    audio::lib::{HostAudioPlayback, create_playback_thread},
};
use eframe::NativeOptions;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create the host's audio handle
    let host_audio = Arc::new(HostAudioPlayback::new()?);

    // Create audio playback thread, this thread is only for previewing samples and playing back simple samples.
    // This is not the main playlist playbacker.
    let palyback_thread_handler = create_playback_thread(host_audio)?;

    let native_options = NativeOptions {
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(|cc| Ok(Box::new(AppRoot::new(cc, palyback_thread_handler)))),
    )?;

    Ok(())
}
