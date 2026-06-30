use std::path::PathBuf;
use anyhow::Result;
use inquire::{Select, Text};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use reqwest::Body;
use futures_util::TryStreamExt;
use std::sync::Arc;
use crate::list::{FoundDevice, scan_all};
use crate::serve::{MESHNET_PORT, expand_tilde};

pub async fn run(file: Option<PathBuf>, device: Option<String>, ip: Option<String>) -> Result<()> {
    let file_path = match file {
        Some(p) => expand_tilde(p),
        None => {
            let cwd = std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let raw = Text::new(&format!("File to send (current dir: {}):", cwd))
                .with_help_message("You can use ~ for home directory, e.g. ~/Desktop/video.mp4")
                .prompt()?;
            expand_tilde(PathBuf::from(raw.trim()))
        }
    };

    if !file_path.exists() {
        anyhow::bail!("File not found: {}", file_path.display());
    }
    if !file_path.is_file() {
        anyhow::bail!("Path is a directory, not a file: {}", file_path.display());
    }

    let target = if let Some(ip_str) = ip {
        parse_ip_device(&ip_str)?
    } else if let Some(name) = device {
        println!("\n  Searching for '{}'...", name);
        scan_all()
            .await
            .into_iter()
            .find(|d| d.name.to_lowercase().contains(&name.to_lowercase()))
            .ok_or_else(|| anyhow::anyhow!("Device '{}' not found.", name))?
    } else {
        println!("\n  Scanning for devices...");
        let mut devices = scan_all().await;

        devices.push(FoundDevice {
            name: "✏️  Enter IP manually".to_string(),
            ip: String::new(),
            port: 0,
            os: String::new(),
        });

        if devices.len() == 1 {
            println!("  No devices found via auto-discovery.");
        }

        let sel = Select::new("Select destination:", devices)
            .with_help_message("↑↓ to navigate, Enter to select. Choose 'Enter IP manually' if your device isn't listed.")
            .prompt()?;

        if sel.name.contains("Enter IP manually") {
            let raw = Text::new("Enter IP and port (e.g. 192.168.1.4:7878):")
                .with_placeholder("192.168.1.X:7878")
                .prompt()?;
            parse_ip_device(&raw)?
        } else {
            sel
        }
    };

    transfer_file(file_path, target).await
}

fn parse_ip_device(raw: &str) -> Result<FoundDevice> {
    let raw = raw.trim();
    let (ip, port) = if let Some(pos) = raw.rfind(':') {
        let ip = raw[..pos].to_string();
        let port = raw[pos + 1..].parse().unwrap_or(MESHNET_PORT);
        (ip, port)
    } else {
        (raw.to_string(), MESHNET_PORT)
    };
    Ok(FoundDevice {
        name: ip.clone(),
        ip,
        port,
        os: String::new(),
    })
}

async fn transfer_file(file_path: PathBuf, device: FoundDevice) -> Result<()> {
    let file_name = file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let file = File::open(&file_path).await?;
    let file_size = file.metadata().await?.len();

    println!("\n  Sending '{}' ({}) to {} ...", file_name, human_size(file_size), device.name);

    let pb = Arc::new(ProgressBar::new(file_size));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}  {bytes_per_sec}  ETA {eta}")?
            .progress_chars("█▉▊▋▌▍▎▏ "),
    );

    let pb_clone = pb.clone();
    let stream = ReaderStream::new(file).inspect_ok(move |chunk| {
        pb_clone.inc(chunk.len() as u64);
    });

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()?;

    let res = client
        .post(format!("http://{}:{}/upload", device.ip, device.port))
        .header("X-File-Name", &file_name)
        .body(Body::wrap_stream(stream))
        .send()
        .await?;

    pb.finish_and_clear();

    if res.status().is_success() {
        println!("  ✓ '{}' delivered successfully!\n", file_name);
    } else {
        anyhow::bail!("Transfer failed — server returned {}", res.status());
    }

    Ok(())
}

fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
