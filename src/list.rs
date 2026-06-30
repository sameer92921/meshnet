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
use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug, Clone)]
pub struct FoundDevice {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub os: String,
}

impl std::fmt::Display for FoundDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = clean_name(&self.name);
        if self.os.is_empty() {
            write!(f, "{}  ({}:{})", label, self.ip, self.port)
        } else {
            write!(f, "{}  [{}]  {}:{}", label, self.os, self.ip, self.port)
        }
    }
}

fn clean_name(raw: &str) -> String {
    raw.split('.').next().unwrap_or(raw).to_string()
}

#[derive(Deserialize)]
struct PingResponse {
    device_name: String,
    os: String,
}

pub async fn run() -> anyhow::Result<()> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("  {spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.set_message("Scanning for MeshNet devices (mDNS + subnet)...");
    spinner.enable_steady_tick(Duration::from_millis(80));

    let devices = scan_all().await;

    spinner.finish_and_clear();

    if devices.is_empty() {
        println!("  No devices found.\n");
        println!("  Make sure the other device is running:  meshnet serve");
        println!("  Or use a direct IP:                     meshnet send --ip <IP>:7878\n");
    } else {
        println!("  Found {} device(s):\n", devices.len());
        for d in &devices {
            let os_tag = if d.os.is_empty() { "?" } else { &d.os };
            println!("    ●  {}  [{}]  {}:{}", clean_name(&d.name), os_tag, d.ip, d.port);
        }
        println!();
    }

    Ok(())
}

pub async fn scan_all() -> Vec<FoundDevice> {
    let mut devices = Vec::new();

    if let Ok(found) = scan_mdns().await {
        devices.extend(found);
    }

    if let Ok(my_ip) = local_ip() {
        if let Some(prefix) = subnet_prefix(&my_ip) {
            let subnet_found = scan_subnet(&prefix, MESHNET_PORT).await;
            for d in subnet_found {
                if !devices.iter().any(|e: &FoundDevice| e.ip == d.ip) {
                    devices.push(d);
                }
            }
        }
    }

    devices
}

async fn scan_mdns() -> anyhow::Result<Vec<FoundDevice>> {
    let mdns = ServiceDaemon::new().context("mDNS unavailable")?;
    let rx = mdns.browse(SERVICE_TYPE).context("mDNS browse failed")?;

    let deadline = tokio::time::sleep(Duration::from_secs(2));
    tokio::pin!(deadline);

    let mut devices = Vec::new();
    loop {
        tokio::select! {
            _ = &mut deadline => break,
            ev = rx.recv_async() => {
                match ev {
                    Ok(ServiceEvent::ServiceResolved(info)) => {
                        if let Some(ip) = info.get_addresses().iter().next() {
                            devices.push(FoundDevice {
                                name: info.get_fullname().to_string(),
                                ip: ip.to_string(),
                                port: info.get_port(),
                                os: String::new(),
                            });
                        }
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
        }
    }
    Ok(devices)
}

pub async fn scan_subnet(prefix: &str, port: u16) -> Vec<FoundDevice> {
    let handles: Vec<_> = (1u8..=254)
        .map(|i| {
            let ip = format!("{}.{}", prefix, i);
            tokio::spawn(async move { probe(&ip, port).await })
        })
        .collect();

    let mut found = Vec::new();
    for h in handles {
        if let Ok(Some(d)) = h.await {
            found.push(d);
        }
    }
    found
}

async fn probe(ip: &str, port: u16) -> Option<FoundDevice> {
    let addr = format!("{}:{}", ip, port);
    timeout(Duration::from_millis(200), TcpStream::connect(&addr))
        .await
        .ok()?
        .ok()?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
        .ok()?;

    let resp = client
        .get(format!("http://{}:{}/ping", ip, port))
        .send()
        .await
        .ok()?;

    let ping: PingResponse = resp.json().await.ok()?;

    Some(FoundDevice {
        name: ping.device_name,
        ip: ip.to_string(),
        port,
        os: ping.os,
    })
}

fn subnet_prefix(ip: &IpAddr) -> Option<String> {
    if let IpAddr::V4(v4) = ip {
        let o = v4.octets();
        Some(format!("{}.{}.{}", o[0], o[1], o[2]))
    } else {
        None
    }
}
