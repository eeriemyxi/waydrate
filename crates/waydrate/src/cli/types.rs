use clap::{Parser, Subcommand};

#[derive(Parser)]
pub(crate) struct Cli {
    #[arg(long)]
    pub db_url: Option<String>,
    #[arg(short, long)]
    pub debug: bool,
    #[command(subcommand)]
    pub command: MainCommand,
}
#[derive(Subcommand)]
pub(crate) enum MainCommand {
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
pub(crate) enum DisplayCommand {
    /// Continuously watch for changes in the DB
    /// and reprint the display command accordingly
    Watch,
}

#[derive(Subcommand)]
pub(crate) enum RecordCommand {
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
pub(crate) enum SetCommand {
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
