use clap::{Parser, Subcommand};
use std::path::PathBuf;
use inquire::Select;

mod serve;
mod list;
mod send;
mod discovery;

#[derive(Parser)]
#[command(name = "meshnet")]
#[command(about = "A local network file transfer CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the receiving daemon (Blocks)
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
        
        /// Device name to send to via mDNS
        #[arg(short, long)]
        device: Option<String>,

        /// Direct IP and Port to send to (e.g. 192.168.1.5:8080)
        #[arg(short, long)]
        ip: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Serve { receive_path }) => {
            serve::run(receive_path.clone()).await?;
        }
        Some(Commands::List) => {
            list::run().await?;
        }
        Some(Commands::Send { file, device, ip }) => {
            send::run(file.clone(), device.clone(), ip.clone()).await?;
        }
        None => {
            // Interactive Two-Way Mode
            println!("Starting MeshNet Two-Way Interactive Mode...");
            
            // Start the server in the background
            let _serve_task = tokio::spawn(async {
                if let Err(e) = serve::run(None).await {
                    eprintln!("Receiver daemon error: {}", e);
                }
            });

            // Small delay to let the server print its startup message before prompting
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            loop {
                let options = vec!["Send a file", "List devices", "Exit"];
                let ans = Select::new("\nWhat would you like to do?", options).prompt();

                match ans {
                    Ok("Send a file") => {
                        if let Err(e) = send::run(None, None, None).await {
                            eprintln!("Error sending file: {}", e);
                        }
                    }
                    Ok("List devices") => {
                        if let Err(e) = list::run().await {
                            eprintln!("Error listing devices: {}", e);
                        }
                    }
                    Ok("Exit") | Err(_) => {
                        println!("Exiting MeshNet...");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
