use mdns_sd::{ServiceDaemon, ServiceEvent};
use anyhow::Context;
use crate::discovery::SERVICE_TYPE;
use std::time::Duration;

pub async fn run() -> anyhow::Result<()> {
    println!("Scanning for meshnet devices on the local network...");

    let mdns = ServiceDaemon::new().context("Failed to create mDNS daemon")?;
    let receiver = mdns.browse(SERVICE_TYPE).context("Failed to browse mDNS")?;

    // Wait for a few seconds to gather results
    let timeout = tokio::time::sleep(Duration::from_secs(3));
    tokio::pin!(timeout);

    let mut found = false;

    loop {
        tokio::select! {
            _ = &mut timeout => {
                break;
            }
            event = receiver.recv_async() => {
                if let Ok(event) = event {
                    if let ServiceEvent::ServiceResolved(info) = event {
                        found = true;
                        let addresses: Vec<String> = info.get_addresses().iter().map(|addr| addr.to_string()).collect();
                        println!("- {} at {}:{}", info.get_fullname(), addresses.join(", "), info.get_port());
                    }
                } else {
                    break;
                }
            }
        }
    }

    if !found {
        println!("No devices found on the local network.");
    }

    Ok(())
}
