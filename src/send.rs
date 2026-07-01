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
use crate::serve::{MESHNET_PORT, expand_tilde, human_size};
use crate::completer::FilePathCompleter;

const READ_BUF_SIZE: usize = 256 * 1024; // 256 KB chunks for higher throughput

pub async fn run(file: Option<PathBuf>, device: Option<String>, ip: Option<String>) -> Result<()> {
    let file_path = match file {
        Some(p) => expand_tilde(p),
        None => {
            let cwd = std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let raw = Text::new(&format!("File to send (cwd: {}):", cwd))
                .with_help_message("Tab to autocomplete • ~ for home directory")
                .with_autocomplete(FilePathCompleter)
                .prompt()?;
            expand_tilde(PathBuf::from(raw.trim()))
        }
    };

    if !file_path.exists() {
        anyhow::bail!("File not found: {}", file_path.display());
    }
    if !file_path.is_file() {
        anyhow::bail!("Not a file: {}", file_path.display());
    }

    let target = if let Some(ip_str) = ip {
        parse_ip_device(&ip_str)?
    } else if let Some(name) = device {
        println!("\n  Searching for '{}'...", name);
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(ProgressStyle::default_spinner().template("  {spinner:.cyan} {msg}").unwrap());
        spinner.set_message("Scanning network...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));
        let devices = scan_all().await;
        spinner.finish_and_clear();
        devices
            .into_iter()
            .find(|d| d.name.to_lowercase().contains(&name.to_lowercase()))
            .ok_or_else(|| anyhow::anyhow!("Device '{}' not found.", name))?
    } else {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(ProgressStyle::default_spinner().template("  {spinner:.cyan} {msg}").unwrap());
        spinner.set_message("Scanning for devices...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(80));
        let mut devices = scan_all().await;
        spinner.finish_and_clear();

        devices.push(FoundDevice {
            name: "✏️  Enter IP manually".to_string(),
            ip: String::new(),
            port: 0,
            os: String::new(),
        });

        if devices.len() == 1 {
            println!("  No devices found via auto-discovery.");
        } else {
            println!("  Found {} device(s).", devices.len() - 1);
        }

        let sel = Select::new("Select destination:", devices)
            .with_help_message("↑↓ navigate • Enter select • Type to filter")
            .prompt()?;

        if sel.name.contains("Enter IP manually") {
            let raw = Text::new("Enter IP:PORT (e.g. 192.168.1.4:7878):")
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
        (raw[..pos].to_string(), raw[pos + 1..].parse().unwrap_or(MESHNET_PORT))
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

    println!(
        "\n  ↑  Sending '{}' ({}) → {}:{}",
        file_name,
        human_size(file_size),
        device.ip,
        device.port
    );

    let pb = Arc::new(ProgressBar::new(file_size));
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  ↑  [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}  {bytes_per_sec}  ETA {eta}")?
            .progress_chars("█▉▊▋▌▍▎▏ "),
    );

    let pb_clone = pb.clone();
    let stream = ReaderStream::with_capacity(file, READ_BUF_SIZE).inspect_ok(move |chunk| {
        pb_clone.inc(chunk.len() as u64);
    });

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(3600))
        .build()?;

    let start = std::time::Instant::now();

    let res = client
        .post(format!("http://{}:{}/upload", device.ip, device.port))
        .header("X-File-Name", &file_name)
        .header("Content-Length", file_size)
        .body(Body::wrap_stream(stream))
        .send()
        .await?;

    let elapsed = start.elapsed();
    pb.finish_and_clear();

    if res.status().is_success() {
        let speed = if elapsed.as_secs_f64() > 0.0 {
            human_size((file_size as f64 / elapsed.as_secs_f64()) as u64)
        } else {
            "∞".to_string()
        };
        println!(
            "  ✓  '{}' delivered!  {} in {:.1}s ({}/s)\n",
            file_name,
            human_size(file_size),
            elapsed.as_secs_f64(),
            speed
        );
    } else {
        anyhow::bail!("Transfer failed — server returned {}", res.status());
    }

    Ok(())
}
