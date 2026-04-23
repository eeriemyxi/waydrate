mod constants;
mod styles;
mod utils;

use std::fs::create_dir;

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use sea_orm::DatabaseConnection;
use utils::{get_config_dir, get_db_file};
use waydrate_core::{self as waycore, entity::config};

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    db_url: Option<String>,
    #[arg(short, long)]
    debug: bool,
    #[command(subcommand)]
    command: MainCommand,
}

trait CommandProperties {
    fn needs_db(&self) -> bool;
}

#[derive(Subcommand)]
enum MainCommand {
    /// Daily intake log
    Daily,
    /// Setup Waydrate
    Setup,
    /// Configure things
    Set {
        #[command(subcommand)]
        command: SetCommand,
    },
    /// Record intakes
    Record {
        #[command(subcommand)]
        command: RecordCommand,
    },
    /// See Waydrate's status
    Status,
    /// Print the templated hydration status (-j for JSON output)
    Display {
        #[command(subcommand)]
        command: Option<DisplayCommand>,
    },
}

#[derive(Subcommand)]
enum DisplayCommand {
    /// Continuously watch for changes in the DB
    /// and reprint the display command accordingly
    Watch,
}

#[derive(Subcommand)]
enum RecordCommand {
    Cup {
        #[arg(default_value_t = 1)]
        count: u8,
    },
    Remove {
        #[arg(short, long)]
        real: bool,
        #[arg(required = true)]
        ids: Vec<String>,
    },
}

#[derive(Subcommand)]
enum SetCommand {
    /// Set daily intake needs
    Goal { ml: u32 },
    /// Set cup size
    CupSize { ml: u32 },
    /// Set the display template. Do `--help` on this for more info.
    ///
    /// Set the display template.
    ///
    /// Available Keys:
    /// 1. `cur_l`
    /// -  The current intake today
    /// 2. `max_l`
    /// -  The max needed intake today
    /// 3. `cur_cup`
    /// -  How many cups you've had today
    /// 4. `max_cup`
    /// -  How many cups you should have today
    #[command(verbatim_doc_comment)]
    DisplayTemplate { template: String },
}

impl CommandProperties for MainCommand {
    fn needs_db(&self) -> bool {
        match self {
            Self::Daily => true,
            Self::Display { command: _ } => true,
            Self::Record { command: _ } => true,
            Self::Set { command: _ } => true,
            Self::Status => true,
            Self::Setup => false,
        }
    }
}

struct CommandHandler {
    cli: Cli,
}

impl CommandHandler {
    async fn new(cli: Cli) -> Result<Self> {
        Ok(Self { cli })
    }

    fn validate(&self) -> Result<()> {
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

    async fn get_display_content(&self) -> Result<String> {
        let conn = utils::get_connection(&self.db_url()?).await?;
        let config = self.config(&conn).await?;
        let ml_today = waycore::get_daily_total(&conn).await?;

        let cur_l = format!("{:.1}", (ml_today as f64) / 1000.0).to_string();
        let max_l = format!("{:.1}", (config.daily_goal_ml as f64) / 1000.0);

        let cur_cup = (ml_today as f64 / config.cup_size as f64)
            .round()
            .to_string();
        let max_cup = (config.daily_goal_ml as f64 / config.cup_size as f64)
            .round()
            .to_string();

        Ok(config
            .display_template
            .replace("{cur_l}", &cur_l)
            .replace("{max_l}", &max_l)
            .replace("{cur_cup}", &cur_cup)
            .replace("{max_cup}", &max_cup))
    }

    async fn handle(&self) -> Result<()> {
        match &self.cli.command {
            MainCommand::Daily => {
                let conn = utils::get_connection(&self.db_url()?).await?;
                let records = waycore::get_daily_records(&conn).await?;
                for (rel_id, rec) in records.iter().enumerate() {
                    let mut buf = String::new();
                    buf.push_str(&format!(
                        "┌ {}\n",
                        rec.date_logged.format("%d/%m/%y - %I:%M %p")
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
            MainCommand::Display { command } => {
                let Some(command) = command else {
                    println!("{}", self.get_display_content().await?);
                    return Ok(());
                };
                match command {
                    DisplayCommand::Watch => {
                        use notify::Watcher;
                        use std::io::{self, Write};

                        let (tx, rx) = std::sync::mpsc::channel();
                        let db_path = get_db_file()?;
                        let mut watcher =
                            notify::RecommendedWatcher::new(tx, notify::Config::default())?;
                        watcher.watch(&db_path, notify::RecursiveMode::NonRecursive)?;

                        println!("{}", self.get_display_content().await?);
                        io::stdout().flush()?;

                        for _ in rx {
                            println!("{}", self.get_display_content().await?);
                            io::stdout().flush()?;
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.debug {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .init();
    }

    let cmd_handler = CommandHandler::new(cli).await?;

    cmd_handler.validate()?;
    cmd_handler.handle().await?;

    Ok(())
}
