use chrono::{DateTime, Duration, Local, Months, Utc};
use once_cell::sync::OnceCell;
use sea_orm::DatabaseConnection;
use std::path::PathBuf;
use waydrate_core as waycore;

use crate::constants::PROGRAM_NAME;
use crate::styles;

use anyhow::{Context, Result, anyhow};
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

pub(crate) fn parse_period(input: &str) -> Result<(i64, char)> {
    let input = input.chars();

    let mut offset = String::new();
    let mut period = 'd';

    for c in input.clone() {
        if c.is_numeric() {
            offset.push(c);
        } else if matches!(c, 'd' | 'w' | 'm' | 'y') {
            period = c;
        } else {
            return Err(anyhow!("Unexpected char {:?} in {:?}", &c, &input.as_str()));
        }
    }

    let offset = offset.parse::<i64>()?;

    Ok((offset, period))
}

pub(crate) fn period_to_datetime(
    offset: i64,
    period: char,
    should_offset_end: bool,
) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
    let today_local = Local::now().date_naive();

    let mut day_start_local = today_local
        .and_hms_opt(0, 0, 0)
        .context("Invalid time parameters provided")?
        .and_local_timezone(Local)
        .latest()
        .context("Invalid time parameters provided")?;

    let mut day_end_local = today_local
        .and_hms_opt(23, 59, 59)
        .context("Invalid time parameters provided")?
        .and_local_timezone(Local)
        .latest()
        .context("Invalid time parameters provided")?;

    match period {
        'd' => {
            day_start_local -= Duration::days(offset);
            if should_offset_end {
                day_end_local -= Duration::days(offset);
            }
        }
        'w' => {
            day_start_local -= Duration::weeks(offset);
            if should_offset_end {
                day_end_local -= Duration::weeks(offset);
            }
        }
        period @ ('m' | 'y') => {
            let offset = u32::try_from(offset)? * (if period == 'y' { 12 } else { 1 });
            day_start_local = day_start_local
                .checked_sub_months(Months::new(offset))
                .context("Couldn't convert date")?;
            if should_offset_end {
                day_end_local = day_end_local
                    .checked_sub_months(Months::new(offset))
                    .context("Couldn't convert date")?;
            }
        }
        _ => unreachable!(),
    }

    let start_utc = day_start_local.with_timezone(&Utc);
    let end_utc = day_end_local.with_timezone(&Utc);

    Ok((start_utc, end_utc))
}
