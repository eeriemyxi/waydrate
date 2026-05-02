mod cli;
mod constants;
mod styles;
mod utils;

use crate::cli::handler::CommandHandler;
use crate::cli::types::Cli;
use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.debug {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_test_writer()
            .init();
    }

    let cmd_handler = CommandHandler::new(cli)?;

    cmd_handler.validate()?;
    cmd_handler.handle().await?;

    Ok(())
}
