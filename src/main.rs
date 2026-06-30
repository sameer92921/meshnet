use clap::{Parser, Subcommand};
use std::path::PathBuf;
use inquire::{Select, Text};

mod serve;
mod list;
mod send;
mod discovery;

#[derive(Parser)]
#[command(
    name = "meshnet",
    version,
    about = "Fast local-network file transfer — macOS, Linux & Android (Termux)",
    long_about = "\
MeshNet lets you send files between devices on the same Wi-Fi network.
No cloud, no accounts, no configuration — just run it.

INTERACTIVE MODE (recommended):
  Simply run `meshnet` with no arguments to start a two-way node that
  can both send and receive files simultaneously.

EXAMPLES:
  meshnet                              Start interactive two-way mode
  meshnet -r ~/Downloads               Interactive mode, save files to ~/Downloads
  meshnet serve ~/Downloads            Receive files into ~/Downloads
  meshnet list                         Show all MeshNet nodes on the LAN
  meshnet send                         Interactive file-send wizard
  meshnet send -f photo.jpg -i 192.168.1.4:7878   Direct send by IP",
    after_help = "For more info, visit: https://github.com/sameer92921/meshnet"
)]
struct Cli {
    /// Directory to save received files (used in interactive mode)
    #[arg(short = 'r', long = "receive-path", value_name = "PATH", global = true)]
    receive_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the file-receiver daemon
    #[command(
        long_about = "Start a daemon that listens for incoming file transfers.\n\
                       Files are saved to PATH (defaults to the current directory).\n\n\
                       EXAMPLES:\n  \
                         meshnet serve\n  \
                         meshnet serve ~/Downloads"
    )]
    Serve {
        #[arg(value_name = "PATH", help = "Directory to save received files (default: current dir)")]
        receive_path: Option<PathBuf>,
    },

    /// Scan the local network for active MeshNet devices
    #[command(
        long_about = "Scans your local network using both mDNS and a fast subnet\n\
                       TCP probe to find all running MeshNet instances.\n\
                       Works even when mDNS is blocked (e.g. Android/Termux)."
    )]
    List,

    /// Send a file to another device
    #[command(
        long_about = "Send a file to another MeshNet device on your network.\n\
                       Run without flags for an interactive wizard, or pass\n\
                       all options for scripted/one-shot transfers.\n\n\
                       EXAMPLES:\n  \
                         meshnet send\n  \
                         meshnet send -f photo.jpg\n  \
                         meshnet send -f video.mp4 -i 192.168.1.4:7878\n  \
                         meshnet send -f doc.pdf -d pixel"
    )]
    Send {
        #[arg(short, long, value_name = "FILE", help = "Path to the file to send")]
        file: Option<PathBuf>,

        #[arg(short, long, value_name = "NAME", help = "Destination device name (partial match, case-insensitive)")]
        device: Option<String>,

        #[arg(short, long, value_name = "IP:PORT", help = "Direct IP:PORT — bypasses network discovery")]
        ip: Option<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Serve { receive_path }) => {
            let path = receive_path.or(cli.receive_path);
            serve::run(path).await?;
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

            let receive_path = if let Some(p) = cli.receive_path {
                Some(p)
            } else {
                let cwd = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| ".".to_string());

                let raw = Text::new("Save received files to:")
                    .with_default(&cwd)
                    .with_help_message("Press Enter for current dir, or type a path (~ supported)")
                    .prompt()?;

                let trimmed = raw.trim();
                if trimmed == cwd || trimmed.is_empty() {
                    None
                } else {
                    Some(PathBuf::from(trimmed))
                }
            };

            let _serve_task = tokio::spawn(async move {
                if let Err(e) = serve::run(receive_path).await {
                    eprintln!("  Receiver error: {}", e);
                }
            });

            tokio::time::sleep(std::time::Duration::from_millis(600)).await;

            loop {
                println!();
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
