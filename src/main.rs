use beatroot::{
    APP_NAME,
    app::AppRoot,
    audio::{
        lib::{HostAudioPlayback, create_playback_thread},
        playback::{HostInformation, MasterPlaybackThread},
    },
};
use eframe::NativeOptions;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let native_options = NativeOptions {
        ..Default::default()
    };

    eframe::run_native(
        APP_NAME,
        native_options,
        Box::new(|cc| Ok(Box::new(AppRoot::new(cc)))),
    )?;

    Ok(())
}
