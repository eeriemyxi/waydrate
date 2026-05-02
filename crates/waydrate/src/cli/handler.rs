use std::fs::create_dir;

use crate::cli::types::DisplayKeys;
use crate::cli::types::{Cli, DisplayCommand, MainCommand, RecordCommand, SetCommand};
use crate::styles;
use crate::utils::{self, get_config_dir, get_db_file};
use anyhow::{Context, Result, anyhow};
use chrono::Local;
use sea_orm::DatabaseConnection;
use waydrate_core::{self as waycore, entity::config};

pub(crate) trait CommandProperties {
    fn needs_db(&self) -> bool;
}

impl CommandProperties for MainCommand {
    fn needs_db(&self) -> bool {
        match self {
            Self::Daily => true,
            Self::Display { command: _, .. } => true,
            Self::Record { command: _ } => true,
            Self::Set { command: _ } => true,
            Self::Status => true,
            Self::Setup => false,
        }
    }
}

pub(crate) struct CommandHandler {
    cli: Cli,
}

impl CommandHandler {
    pub fn new(cli: Cli) -> Result<Self> {
        Ok(Self { cli })
    }

    pub fn validate(&self) -> Result<()> {
        let config_dir = get_config_dir()?;
        if !config_dir.exists() && self.cli.command.needs_db() {
            return Err(anyhow!(
                "The directory {dir:?} doesn't exist.\n{style}help{style:#}: You can run `setup` subcommand to initialize it.",
                dir = &config_dir,
                style = styles::bold_green()
            ));
        }

        let db_file = get_db_file()?;
        if !db_file.exists() && self.cli.command.needs_db() {
            return Err(anyhow!(
                "The database {db_file:?} doesn't exist.\n{style}help{style:#}: You can run `setup` subcommand to initialize it.",
                style = styles::bold_green()
            ));
        }
        Ok(())
    }

    fn db_url(&self) -> Result<String> {
        Ok(self.cli.db_url.clone().unwrap_or(utils::get_db_url()?))
    }

    async fn config(&self, conn: &DatabaseConnection) -> Result<config::Model> {
        waycore::get_config(conn)
            .await?
            .context("config uvailable (bug)")
    }

    async fn get_display_keys(&self) -> Result<DisplayKeys> {
        let conn = utils::get_connection(&self.db_url()?).await?;
        let config = self.config(&conn).await?;
        let ml_today = waycore::get_daily_total(&conn).await?;

        let prec = 10.0;
        let cur_l = (ml_today as f64 / 1000.0 * prec).round() / prec;
        let max_l = (config.daily_goal_ml as f64 / 1000.0 * prec).round() / prec;

        let cur_cup = (ml_today as f64 / config.cup_size as f64).round();
        let max_cup = (config.daily_goal_ml as f64 / config.cup_size as f64).round();

        Ok(DisplayKeys {
            cur_l,
            max_l,
            cur_cup,
            max_cup,
        })
    }

    async fn get_display_content(&self) -> Result<String> {
        let conn = utils::get_connection(&self.db_url()?).await?;
        let config = self.config(&conn).await?;

        let keys = self.get_display_keys().await?;

        Ok(config
            .display_template
            .replace("{cur_l}", &keys.cur_l.to_string())
            .replace("{max_l}", &keys.max_l.to_string())
            .replace("{cur_cup}", &keys.cur_cup.to_string())
            .replace("{max_cup}", &keys.max_cup.to_string()))
    }

    async fn get_display_json_content(&self) -> Result<String> {
        let keys = self.get_display_keys().await?;
        let mut buf = String::new();
        buf.push('{');
        buf.push_str(&format!("\"cur_l\": {},", keys.cur_l));
        buf.push_str(&format!("\"max_l\": {},", keys.max_l));
        buf.push_str(&format!("\"cur_cup\": {},", keys.cur_cup));
        buf.push_str(&format!("\"max_cup\": {}", keys.max_cup));
        buf.push('}');
        Ok(buf)
    }

    pub async fn handle(&self) -> Result<()> {
        match &self.cli.command {
            MainCommand::Daily => {
                let conn = utils::get_connection(&self.db_url()?).await?;
                let records = waycore::get_daily_records(&conn).await?;
                for (rel_id, rec) in records.iter().enumerate() {
                    let mut buf = String::new();
                    let date = rec.date_logged.with_timezone(&Local);
                    buf.push_str(&format!(
                        "┌ {} ({})\n",
                        date.format("%d/%m/%y - %I:%M %p"),
                        chrono_humanize::HumanTime::from(date)
                    ));
                    buf.push_str(&format!(
                        "└ 󰖌 {} ml | id: {} | r-id: {}\n",
                        rec.amount_ml, rec.id, rel_id
                    ));
                    println!("{}", &buf)
                }
            }
            MainCommand::Record { command } => match command {
                RecordCommand::Cup { count } => {
                    let conn = utils::get_connection(&self.db_url()?).await?;
                    let config = self.config(&conn).await?;
                    for _ in 0..*count {
                        waycore::add_record(&conn, config.cup_size).await?;
                    }
                }
                RecordCommand::Remove { real, ids } => {
                    let conn = utils::get_connection(&self.db_url()?).await?;
                    let records = waycore::get_daily_records(&conn).await?;

                    if let Some(id) = ids.first()
                        && id == "last"
                    {
                        waycore::remove_record(
                            &conn,
                            records.last().context("Records were empty.")?.id,
                        )
                        .await?;
                        return Ok(());
                    }

                    let parsed_ids: Vec<i32> = ids
                        .iter()
                        .map(|v| {
                            v.parse::<i32>()
                                .unwrap_or_else(|_| panic!("couldn't convert id {:?}", v))
                        })
                        .collect();
                    for (_, rec) in records.iter().enumerate().filter(|(rid, rec)| {
                        *real && parsed_ids.contains(&rec.id)
                            || !*real && parsed_ids.contains(&(*rid as i32))
                    }) {
                        waycore::remove_record(&conn, rec.id).await?;
                        println!("Removed {:?}", rec)
                    }
                }
            },
            MainCommand::Status => {
                let conn = utils::get_connection(&self.db_url()?).await?;
                let config = self.config(&conn).await?;
                println!("Daily Goal: {} ml", config.daily_goal_ml);
                println!("Cup Size: {} ml", config.cup_size);
                println!("Display Template: {}", config.display_template);
            }
            MainCommand::Display { command, json } => {
                let show_content = async || -> Result<()> {
                    use std::io::{self, Write};

                    if *json {
                        println!("{}", self.get_display_json_content().await?);
                    } else {
                        println!("{}", self.get_display_content().await?);
                    }

                    io::stdout().flush()?;

                    Ok(())
                };
                let Some(command) = command else {
                    show_content().await?;
                    return Ok(());
                };
                match command {
                    DisplayCommand::Watch => {
                        use notify::Watcher;

                        let (tx, rx) = std::sync::mpsc::channel();
                        let db_path = get_db_file()?;

                        let mut watcher =
                            notify::RecommendedWatcher::new(tx, notify::Config::default())?;
                        watcher.watch(&db_path, notify::RecursiveMode::NonRecursive)?;

                        show_content().await?;
                        for _ in rx {
                            show_content().await?;
                        }
                    }
                }
            }
            MainCommand::Setup => {
                let config_dir = get_config_dir()?;
                match create_dir(config_dir) {
                    Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                        println!(
                            "Skipping creating {dir:?} because it already exists",
                            dir = &config_dir
                        )
                    }
                    Err(e) => {
                        println!(
                            "Couldn't create directory: {dir:?} because of: {e}",
                            dir = &config_dir
                        )
                    }
                    Ok(()) => {
                        println!("Created directory: {dir:?}", dir = &config_dir);
                    }
                }

                let _ = utils::get_connection(&self.db_url()?).await?;
                println!("Successfully initialized the database");
            }
            MainCommand::Set { command } => match command {
                SetCommand::Goal { ml } => {
                    let conn = utils::get_connection(&self.db_url()?).await?;
                    waycore::set_goal(&conn, (*ml).try_into()?).await?;
                }
                SetCommand::CupSize { ml } => {
                    let conn = utils::get_connection(&self.db_url()?).await?;
                    waycore::set_cup_size(&conn, (*ml).try_into()?).await?;
                }
                SetCommand::DisplayTemplate { template } => {
                    let conn = utils::get_connection(&self.db_url()?).await?;
                    waycore::set_display_template(&conn, template.to_string()).await?;
                }
            },
        }
        Ok(())
    }
}
