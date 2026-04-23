use once_cell::sync::OnceCell;
use sea_orm::DatabaseConnection;
use std::path::PathBuf;
use waydrate_core as waycore;

use crate::constants::PROGRAM_NAME;
use crate::styles;

use anyhow::{Context, Result};
use platform_dirs::AppDirs;

static CONN: tokio::sync::OnceCell<DatabaseConnection> = tokio::sync::OnceCell::const_new();

pub(crate) fn get_app_dirs() -> Result<&'static AppDirs> {
    static APP_DIRS: OnceCell<AppDirs> = OnceCell::new();

    APP_DIRS.get_or_try_init(|| {
        AppDirs::new(Some(PROGRAM_NAME), true)
            .with_context(|| format!("Failed to find the app directories for {PROGRAM_NAME}"))
    })
}

pub(crate) fn get_config_dir() -> Result<&'static PathBuf> {
    let app_dirs = get_app_dirs()?;
    Ok(&app_dirs.config_dir)
}

pub(crate) fn get_db_file() -> Result<PathBuf> {
    let app_dirs = get_app_dirs()?;
    Ok(app_dirs.config_dir.join("waydrate.db"))
}

pub(crate) async fn get_connection(db_url: &str) -> Result<DatabaseConnection> {
    Ok(CONN.get_or_try_init(async ||
                         waycore::establish_connection(db_url)
                            .await
        .with_context(|| {
            format!(
                "Couldn't open DB connection: {db_url}\n{style}help:{style:#} You probably just need to run `setup` command to create the config directory",
                style = styles::bold_green()
            )
        })).await?.clone())
}

pub(crate) fn get_db_url() -> Result<String> {
    Ok(format!(
        "sqlite://{file}?mode=rwc",
        file = get_db_file()?.display()
    ))
}
