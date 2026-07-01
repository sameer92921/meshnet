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
    // Resolve the file argument into one or more paths (supports globs like *.pdf)
    let paths = match file {
        Some(p) => resolve_paths(&p.to_string_lossy())?,
        None => {
            let cwd = std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let raw = Text::new(&format!("File(s) to send (cwd: {}):", cwd))
                .with_help_message("Tab to autocomplete • ~ for home • wildcards: *.pdf, ~/Photos/*")
                .with_autocomplete(FilePathCompleter)
                .prompt()?;
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                anyhow::bail!("No file specified");
            }
            resolve_paths(trimmed)?
        }
    };

    if paths.is_empty() {
        anyhow::bail!("No files matched the pattern.");
    }

    // Pick the target device once, then send all files to it
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

    if paths.len() > 1 {
        println!("\n  Sending {} files...", paths.len());
    }

    let mut failed = 0u32;
    for (i, path) in paths.iter().enumerate() {
        if paths.len() > 1 {
            println!("  [{}/{}]", i + 1, paths.len());
        }
        if let Err(e) = transfer_file(path.clone(), target.clone()).await {
            eprintln!("  ✗ {}: {}", path.display(), e);
            failed += 1;
        }
    }

    if failed > 0 {
        anyhow::bail!("{} of {} file(s) failed to send.", failed, paths.len());
    }

    Ok(())
}

/// Expand tilde and glob patterns into a list of real file paths.
/// If the input has no wildcards, validates that the single path exists and is a file.
fn resolve_paths(input: &str) -> Result<Vec<PathBuf>> {
    let expanded = expand_tilde(PathBuf::from(input));
    let s = expanded.to_string_lossy();

    if s.contains('*') || s.contains('?') {
        // Glob mode
        let matched: Vec<PathBuf> = glob::glob(&s)
            .map_err(|e| anyhow::anyhow!("Invalid pattern '{}': {}", input, e))?
            .filter_map(|entry| entry.ok())
            .filter(|p| p.is_file())
            .collect();

        if matched.is_empty() {
            anyhow::bail!("No files matched pattern: {}", input);
        }

        println!("  Matched {} file(s).", matched.len());
        Ok(matched)
    } else {
        // Single file mode — keep clear error messages
        if !expanded.exists() {
            anyhow::bail!("File not found: {}", expanded.display());
        }
        if !expanded.is_file() {
            anyhow::bail!("Not a file: {}", expanded.display());
        }
        Ok(vec![expanded])
    }
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
