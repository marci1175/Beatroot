use std::sync::Arc;

use parking_lot::Mutex;

const VERSION_API: &str = "https://marci1175.github.io/Beatroot/version/status.json";

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LatestVersion {
    /// Latest version number - fetched from the latest commit's Cargo.toml
    pub version: String,

    /// Latest git commit id
    pub commit: String,
}

/// Checks for updates by requesting the latest version information from the static API.
pub fn check_for_update(update_clone: Arc<Mutex<Option<anyhow::Result<bool>>>>) {
    // Check the version by fetching latest info from API.
    tokio::spawn(async move {
        // Create a fn which modifies the update status if successful and returns an error if failed.
        let result = async || -> anyhow::Result<()> {
            // Create a get request to the version's API endpoint
            let req = reqwest::get(VERSION_API).await?;

            // Get the text of the request
            let str = req.text().await?;

            // Serialize the response of the GET request
            let latest = serde_json::from_str::<LatestVersion>(&str)?;

            // Compare with current version number, if they mismatch it means that there is an update available
            let update_handle = &mut *update_clone.lock();

            if latest.version != env!("CARGO_PKG_VERSION") {
                // Indicate that there is an update available.
                *update_handle = Some(Ok(true))
            } else {
                *update_handle = Some(Ok(false))
            }

            Ok(())
        };

        // Change the upddate status to Err if the update function failed to fetch the version number
        if let Err(err) = (result)().await {
            *update_clone.lock() = Some(Err(err));
        }
    });
}
