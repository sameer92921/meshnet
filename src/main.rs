use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod serve;
mod list;
mod send;
mod discovery;

#[derive(Parser)]
#[command(name = "meshnet")]
#[command(about = "A local network file transfer CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the receiving daemon
    Serve {
        /// Optional custom directory to receive files
        #[arg(short, long)]
        receive_path: Option<PathBuf>,
    },
    /// List available devices on the network
    List,
    /// Send a file to a device (interactive mode if no args provided)
    Send {
        /// Path to the file to send
        #[arg(short, long)]
        file: Option<PathBuf>,
        
        /// Device name to send to
        #[arg(short, long)]
        device: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Serve { receive_path } => {
            serve::run(receive_path.clone()).await?;
        }
        Commands::List => {
            list::run().await?;
        }
        Commands::Send { file, device } => {
            send::run(file.clone(), device.clone()).await?;
        }
    }

    Ok(())
}
