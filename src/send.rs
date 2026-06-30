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
use crate::serve::MESHNET_PORT;

pub async fn run(file: Option<PathBuf>, device: Option<String>, ip: Option<String>) -> Result<()> {
    let file_path = match file {
        Some(path) => path,
        None => {
            let path_str = Text::new("Enter the path to the file you want to send:").prompt()?;
            PathBuf::from(path_str)
        }
    };

    if !file_path.exists() || !file_path.is_file() {
        anyhow::bail!("File does not exist or is not a file: {:?}", file_path);
    }

    let target_device = if let Some(ip_addr) = ip {
        // Direct IP mode
        let parts: Vec<&str> = ip_addr.splitn(2, ':').collect();
        let (target_ip, target_port) = if parts.len() == 2 {
            (parts[0].to_string(), parts[1].parse().unwrap_or(MESHNET_PORT))
        } else {
            (ip_addr.clone(), MESHNET_PORT)
        };
        FoundDevice {
            name: "Direct IP Device".to_string(),
            ip: target_ip,
            port: target_port,
            os: "unknown".to_string(),
        }
    } else if let Some(dev_name) = device {
        // Search by name in a scan
        println!("Looking for device '{}'...", dev_name);
        let devices = scan_all().await;
        devices.into_iter()
            .find(|d| d.name.contains(&dev_name))
            .ok_or_else(|| anyhow::anyhow!("Device '{}' not found on the network.", dev_name))?
    } else {
        // Full interactive mode
        println!("Scanning for devices... (this may take a few seconds)");
        let mut devices = scan_all().await;

        // Always add the manual IP option at the top
        devices.insert(0, FoundDevice {
            name: "📝 Enter IP Manually".to_string(),
            ip: "".to_string(),
            port: 0,
            os: "".to_string(),
        });

        if devices.len() == 1 {
            println!("No devices found via auto-discovery.");
        }

        let selection = Select::new("Select a device to send to:", devices).prompt()?;

        if selection.name.contains("Enter IP Manually") {
            let ip_addr = Text::new("Enter the device IP and port (e.g. 192.168.1.5:7878):").prompt()?;
            let parts: Vec<&str> = ip_addr.splitn(2, ':').collect();
            let (target_ip, target_port) = if parts.len() == 2 {
                (parts[0].to_string(), parts[1].parse().unwrap_or(MESHNET_PORT))
            } else {
                (ip_addr.clone(), MESHNET_PORT)
            };
            FoundDevice {
                name: "Direct IP Device".to_string(),
                ip: target_ip,
                port: target_port,
                os: "unknown".to_string(),
            }
        } else {
            selection
        }
    };

    send_file(file_path, target_device).await?;

    Ok(())
}

async fn send_file(file_path: PathBuf, device: FoundDevice) -> Result<()> {
    let file = File::open(&file_path).await?;
    let file_size = file.metadata().await?.len();
    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();

    let pb = ProgressBar::new(file_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
        .progress_chars("#>-"));

    let reader_stream = ReaderStream::new(file);
    let pb = Arc::new(pb);
    let pb_clone = pb.clone();

    let async_stream = reader_stream.inspect_ok(move |chunk| {
        pb_clone.inc(chunk.len() as u64);
    });

    let body = Body::wrap_stream(async_stream);

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;
    let url = format!("http://{}:{}/upload", device.ip, device.port);

    println!("Sending '{}' to {} ({})...", file_name, device.name, device.ip);

    let res = client.post(&url)
        .header("X-File-Name", &file_name)
        .body(body)
        .send()
        .await?;

    pb.finish_with_message("done");

    if res.status().is_success() {
        println!("✓ '{}' sent successfully!", file_name);
    } else {
        anyhow::bail!("Failed to send file. Server responded with: {}", res.status());
    }

    Ok(())
}
