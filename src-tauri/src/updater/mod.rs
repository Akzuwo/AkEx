use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

/// Checks the signed GitHub release feed without blocking application startup.
/// Update failures are logged and never prevent the file browser from opening.
pub fn spawn_auto_update(app: AppHandle) {
    if cfg!(debug_assertions) {
        log::debug!(target: "updater", "Automatic updates are disabled in development builds");
        return;
    }

    tauri::async_runtime::spawn(async move {
        if let Err(error) = check_and_install(app).await {
            log::warn!(target: "updater", "Automatic update check failed: {error}");
        }
    });
}

async fn check_and_install(app: AppHandle) -> tauri_plugin_updater::Result<()> {
    let Some(update) = app
        .updater_builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?
        .check()
        .await?
    else {
        log::debug!(target: "updater", "Akex is up to date");
        return Ok(());
    };

    log::info!(
        target: "updater",
        "Installing signed Akex update {} (current {})",
        update.version,
        update.current_version
    );
    let mut downloaded = 0_u64;
    let mut last_logged_percent = 0_u64;
    update
        .download_and_install(
            |chunk_length, content_length| {
                downloaded = downloaded.saturating_add(chunk_length as u64);
                if let Some(total) = content_length.filter(|total| *total > 0) {
                    let percent = downloaded.saturating_mul(100) / total;
                    if percent >= last_logged_percent.saturating_add(10) {
                        last_logged_percent = percent;
                        log::info!(target: "updater", "Update download: {percent}%");
                    }
                }
            },
            || log::info!(target: "updater", "Update downloaded; starting installation"),
        )
        .await?;

    log::info!(target: "updater", "Update installed; restarting Akex");
    app.restart();
}
