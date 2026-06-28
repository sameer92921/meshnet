use std::path::PathBuf;
use mdns_sd::{ServiceDaemon, ServiceEvent};
use anyhow::{Context, Result};
use crate::discovery::SERVICE_TYPE;
use std::time::Duration;
use inquire::{Select, Text};
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use reqwest::Body;
use futures_util::TryStreamExt;
use std::sync::Arc;

struct DiscoveredDevice {
    name: String,
    ip: String,
    port: u16,
}

impl std::fmt::Display for DiscoveredDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}:{})", self.name, self.ip, self.port)
    }
}

pub async fn run(file: Option<PathBuf>, device: Option<String>) -> Result<()> {
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

    let target_device = match device {
        Some(dev_name) => {
            println!("Looking for device '{}'...", dev_name);
            find_device_by_name(&dev_name).await?
        }
        None => {
            println!("Scanning for devices...");
            let devices = scan_for_devices().await?;
            if devices.is_empty() {
                anyhow::bail!("No devices found on the network.");
            }
            Select::new("Select a device to send to:", devices).prompt()?
        }
    };

    send_file(file_path, target_device).await?;

    Ok(())
}

async fn scan_for_devices() -> Result<Vec<DiscoveredDevice>> {
    let mdns = ServiceDaemon::new().context("Failed to create mDNS daemon")?;
    let receiver = mdns.browse(SERVICE_TYPE).context("Failed to browse mDNS")?;

    let timeout = tokio::time::sleep(Duration::from_secs(3));
    tokio::pin!(timeout);

    let mut devices = Vec::new();

    loop {
        tokio::select! {
            _ = &mut timeout => {
                break;
            }
            event = receiver.recv_async() => {
                if let Ok(event) = event {
                    if let ServiceEvent::ServiceResolved(info) = event {
                        if let Some(ip) = info.get_addresses().iter().next() {
                            devices.push(DiscoveredDevice {
                                name: info.get_fullname().to_string(),
                                ip: ip.to_string(),
                                port: info.get_port(),
                            });
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }

    Ok(devices)
}

async fn find_device_by_name(name: &str) -> Result<DiscoveredDevice> {
    let mdns = ServiceDaemon::new().context("Failed to create mDNS daemon")?;
    let receiver = mdns.browse(SERVICE_TYPE).context("Failed to browse mDNS")?;

    let timeout = tokio::time::sleep(Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            _ = &mut timeout => {
                anyhow::bail!("Device not found within timeout.");
            }
            event = receiver.recv_async() => {
                if let Ok(event) = event {
                    if let ServiceEvent::ServiceResolved(info) = event {
                        if info.get_fullname().contains(name) {
                            if let Some(ip) = info.get_addresses().iter().next() {
                                return Ok(DiscoveredDevice {
                                    name: info.get_fullname().to_string(),
                                    ip: ip.to_string(),
                                    port: info.get_port(),
                                });
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }
    
    anyhow::bail!("Device not found.")
}

async fn send_file(file_path: PathBuf, device: DiscoveredDevice) -> Result<()> {
    let file = File::open(&file_path).await?;
    let file_size = file.metadata().await?.len();
    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy().to_string();

    let pb = ProgressBar::new(file_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
        .progress_chars("#>-"));

    // Wrap the file stream with a progress bar update
    let reader_stream = ReaderStream::new(file);
    let pb = Arc::new(pb);
    let pb_clone = pb.clone();
    
    let async_stream = reader_stream.inspect_ok(move |chunk| {
        pb_clone.inc(chunk.len() as u64);
    });

    let body = Body::wrap_stream(async_stream);

    let client = Client::new();
    let url = format!("http://{}:{}/upload", device.ip, device.port);

    println!("Sending {} to {}...", file_name, device.name);

    let res = client.post(&url)
        .header("X-File-Name", file_name)
        .body(body)
        .send()
        .await?;

    pb.finish_with_message("Transfer complete!");

    if res.status().is_success() {
        println!("File sent successfully!");
    } else {
        anyhow::bail!("Failed to send file. Server responded with: {}", res.status());
    }

    Ok(())
}
