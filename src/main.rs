mod cli;
mod coin_selection;
mod config;
mod logging;
mod persistence;
mod protocol;
mod rpc;
mod session;
mod tui;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Tui(args) => {
            let cfg = config::Config::load_with_overrides(&args)?;
            tui::run(cfg).await?;
        }
        _ => {
            logging::init(cli.verbose)?;
            run_cli(cli).await?;
        }
    }

    Ok(())
}

async fn run_cli(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Init(args) => {
            let cfg = config::Config::initialize(args)?;
            config::Config::save(&cfg)?;
            tracing::info!("Configuration initialized at {}", cfg.data_dir.display());
            println!("pj-receive initialized successfully.");
            println!("Data directory: {}", cfg.data_dir.display());
            println!(
                "Network: {} | RPC: {}:{}",
                cfg.network, cfg.rpc_host, cfg.rpc_port
            );
        }
        Command::Receive(args) => {
            let cfg = config::Config::load_with_overrides(&args.global)?;
            protocol::run_receive_session(&cfg, &args).await?;
        }
        Command::Status(args) => {
            let cfg = config::Config::load_with_overrides(&args)?;
            session::show_status(&cfg)?;
        }
        Command::History(args) => {
            let cfg = config::Config::load_with_overrides(&args)?;
            session::show_history(&cfg)?;
        }
        Command::Config(args) => {
            let cfg = config::Config::load_with_overrides(&args.global)?;
            if args.show {
                println!("{}", serde_json::to_string_pretty(&cfg)?);
            }
        }
        Command::Tui(_) => unreachable!(),
    }

    Ok(())
}
