use clap::{Parser, Subcommand};
use std::path::PathBuf;
use inquire::Select;

mod serve;
mod list;
mod send;
mod discovery;

#[derive(Parser)]
#[command(name = "meshnet")]
#[command(version, about = "Fast local-network file transfer — works on macOS, Linux & Android (Termux)")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start only the file-receiver daemon (saves files to PATH, default: current dir)
    Serve {
        #[arg(value_name = "PATH", help = "Directory to save received files")]
        receive_path: Option<PathBuf>,
    },
    /// Scan the local network and list all active MeshNet devices
    List,
    /// Send a file to another device
    Send {
        #[arg(short, long, value_name = "FILE", help = "Path to the file to send")]
        file: Option<PathBuf>,

        #[arg(short, long, value_name = "NAME", help = "Device name (partial match)")]
        device: Option<String>,

        #[arg(short, long, value_name = "IP:PORT", help = "Direct IP address (bypasses discovery)")]
        ip: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Serve { receive_path }) => {
            serve::run(receive_path).await?;
        }
        Some(Commands::List) => {
            list::run().await?;
        }
        Some(Commands::Send { file, device, ip }) => {
            send::run(file, device, ip).await?;
        }
        None => {
            println!("\n  ███╗   ███╗███████╗███████╗██╗  ██╗███╗   ██╗███████╗████████╗");
            println!("  ████╗ ████║██╔════╝██╔════╝██║  ██║████╗  ██║██╔════╝╚══██╔══╝");
            println!("  ██╔████╔██║█████╗  ███████╗███████║██╔██╗ ██║█████╗     ██║   ");
            println!("  ██║╚██╔╝██║██╔══╝  ╚════██║██╔══██║██║╚██╗██║██╔══╝     ██║   ");
            println!("  ██║ ╚═╝ ██║███████╗███████║██║  ██║██║ ╚████║███████╗   ██║   ");
            println!("  ╚═╝     ╚═╝╚══════╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═══╝╚══════╝   ╚═╝   ");
            println!("  Fast local file transfer  •  github.com/sameer92921/meshnet\n");

            let _serve_task = tokio::spawn(async {
                if let Err(e) = serve::run(None).await {
                    eprintln!("Receiver error: {}", e);
                }
            });

            tokio::time::sleep(std::time::Duration::from_millis(600)).await;

            loop {
                let options = vec![
                    "📤  Send a file",
                    "🔍  Scan for devices",
                    "❌  Exit",
                ];
                match Select::new("What would you like to do?", options).prompt() {
                    Ok("📤  Send a file") => {
                        if let Err(e) = send::run(None, None, None).await {
                            eprintln!("\n  ✗ {}\n", e);
                        }
                    }
                    Ok("🔍  Scan for devices") => {
                        if let Err(e) = list::run().await {
                            eprintln!("\n  ✗ {}\n", e);
                        }
                    }
                    _ => {
                        println!("\n  Goodbye!\n");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}
