use mdns_sd::{ServiceDaemon, ServiceEvent};
use anyhow::Context;
use crate::discovery::SERVICE_TYPE;
use crate::serve::MESHNET_PORT;
use local_ip_address::local_ip;
use std::time::Duration;
use std::net::IpAddr;
use tokio::net::TcpStream;
use tokio::time::timeout;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct FoundDevice {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub os: String,
}

impl std::fmt::Display for FoundDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}) — {}:{}", self.name, self.os, self.ip, self.port)
    }
}

#[derive(Deserialize)]
struct PingResponse {
    device_name: String,
    os: String,
}

pub async fn run() -> anyhow::Result<()> {
    println!("Scanning for meshnet devices on the local network...");

    let devices = scan_all().await;

    if devices.is_empty() {
        println!("No devices found on the local network.");
        println!("Tip: Make sure the other device is running `meshnet serve` or just `meshnet`.");
    } else {
        println!("Found {} device(s):", devices.len());
        for d in &devices {
            println!("  ● {} — {}:{}", d.name, d.ip, d.port);
        }
    }

    Ok(())
}

/// Tries mDNS first, then falls back to subnet TCP scan.
pub async fn scan_all() -> Vec<FoundDevice> {
    let mut devices = Vec::new();

    // Step 1: mDNS
    if let Ok(found) = scan_mdns().await {
        devices.extend(found);
    }

    // Step 2: Subnet TCP scan (always runs so Android devices are found too)
    if let Ok(my_ip) = local_ip() {
        let subnet = get_subnet_prefix(&my_ip);
        if let Some(prefix) = subnet {
            let found = scan_subnet(&prefix, MESHNET_PORT).await;
            for d in found {
                // Avoid duplicates from mDNS
                if !devices.iter().any(|existing: &FoundDevice| existing.ip == d.ip) {
                    devices.push(d);
                }
            }
        }
    }

    devices
}

async fn scan_mdns() -> anyhow::Result<Vec<FoundDevice>> {
    let mdns = ServiceDaemon::new().context("Failed to create mDNS daemon")?;
    let receiver = mdns.browse(SERVICE_TYPE).context("Failed to browse mDNS")?;

    let timeout_dur = tokio::time::sleep(Duration::from_secs(2));
    tokio::pin!(timeout_dur);

    let mut devices = Vec::new();

    loop {
        tokio::select! {
            _ = &mut timeout_dur => break,
            event = receiver.recv_async() => {
                if let Ok(event) = event {
                    if let ServiceEvent::ServiceResolved(info) = event {
                        if let Some(ip) = info.get_addresses().iter().next() {
                            devices.push(FoundDevice {
                                name: info.get_fullname().to_string(),
                                ip: ip.to_string(),
                                port: info.get_port(),
                                os: "unknown".to_string(),
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

/// Probes all 254 IPs in a /24 subnet concurrently for the meshnet port.
pub async fn scan_subnet(prefix: &str, port: u16) -> Vec<FoundDevice> {
    let mut handles = Vec::new();

    for i in 1u8..=254 {
        let ip = format!("{}.{}", prefix, i);
        let port = port;
        handles.push(tokio::spawn(async move {
            probe_device(ip, port).await
        }));
    }

    let mut found = Vec::new();
    for handle in handles {
        if let Ok(Some(device)) = handle.await {
            found.push(device);
        }
    }
    found
}

async fn probe_device(ip: String, port: u16) -> Option<FoundDevice> {
    let addr = format!("{}:{}", ip, port);
    // Quick TCP connect check (200ms timeout)
    if timeout(Duration::from_millis(200), TcpStream::connect(&addr)).await.is_err() {
        return None;
    }

    // If TCP connects, try the /ping endpoint to confirm it's a meshnet node
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .ok()?;

    let url = format!("http://{}:{}/ping", ip, port);
    let resp = client.get(&url).send().await.ok()?;
    let ping: PingResponse = resp.json().await.ok()?;

    Some(FoundDevice {
        name: ping.device_name,
        ip,
        port,
        os: ping.os,
    })
}

fn get_subnet_prefix(ip: &IpAddr) -> Option<String> {
    if let IpAddr::V4(v4) = ip {
        let octets = v4.octets();
        Some(format!("{}.{}.{}", octets[0], octets[1], octets[2]))
    } else {
        None
    }
}
