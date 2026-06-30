use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::TcpListener;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use axum::{
    routing::{get, post},
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    body::Body,
    Json,
};
use http_body_util::BodyExt;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use local_ip_address::local_ip;
use anyhow::Context;
use serde::Serialize;
use indicatif::{ProgressBar, ProgressStyle};
use crate::discovery::SERVICE_TYPE;

pub const MESHNET_PORT: u16 = 7878;

struct AppState {
    receive_path: PathBuf,
    device_name: String,
}

#[derive(Serialize)]
struct PingResponse {
    device_name: String,
    os: String,
}

pub async fn run(receive_path: Option<PathBuf>) -> anyhow::Result<()> {
    let receive_path = receive_path
        .map(|p| expand_tilde(p))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    tokio::fs::create_dir_all(&receive_path)
        .await
        .context("Failed to create receive directory")?;

    let my_ip = local_ip().context("Failed to get local IP")?;
    let hostname = hostname::get()?
        .into_string()
        .unwrap_or_else(|_| "Unknown-Device".to_string());
    let short_id: String = uuid::Uuid::new_v4().to_string().chars().take(4).collect();
    let device_name = format!("{}-{}", hostname, short_id);

    let state = Arc::new(AppState {
        receive_path: receive_path.clone(),
        device_name: device_name.clone(),
    });

    let app = Router::new()
        .route("/ping", get(handle_ping))
        .route("/upload", post(handle_upload))
        .with_state(state.clone());

    let listener = match TcpListener::bind(format!("0.0.0.0:{}", MESHNET_PORT)).await {
        Ok(l) => l,
        Err(_) => {
            eprintln!("  ⚠  Port {} in use, picking a random port.", MESHNET_PORT);
            TcpListener::bind("0.0.0.0:0").await?
        }
    };
    let port = listener.local_addr()?.port();

    println!("  ┌─────────────────────────────────────────┐");
    println!("  │      ✦  Receiver is now active  ✦       │");
    println!("  ├─────────────────────────────────────────┤");
    println!("  │  Device   {}", pad_right(&device_name, 28));
    println!("  │  Address  {}", pad_right(&format!("{}:{}", my_ip, port), 28));
    println!("  │  Saving   {}", pad_right(&format!("{}", receive_path.display()), 28));
    println!("  └─────────────────────────────────────────┘");
    println!("  Share your Address with the sender if discovery fails.\n");

    if let Ok(mdns) = ServiceDaemon::new() {
        let properties = vec![("os", std::env::consts::OS)];
        if let Ok(svc) = ServiceInfo::new(
            SERVICE_TYPE,
            &device_name,
            &format!("{}.local.", device_name),
            my_ip.to_string(),
            port,
            &properties[..],
        ) {
            let _ = mdns.register(svc);
        }
    }

    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_ping(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(PingResponse {
        device_name: state.device_name.clone(),
        os: std::env::consts::OS.to_string(),
    })
}

async fn handle_upload(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut body: Body,
) -> impl IntoResponse {
    let file_name = headers
        .get("X-File-Name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("received_file")
        .to_string();

    let total_size: Option<u64> = headers
        .get("Content-Length")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    let file_path = state.receive_path.join(&file_name);

    let size_str = total_size.map(|s| format!(" ({})", human_size(s))).unwrap_or_default();
    println!("\n  ↓  Receiving: {}{}", file_name, size_str);

    let pb = match total_size {
        Some(size) => {
            let pb = ProgressBar::new(size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("  ↓  [{wide_bar:.green/dim}] {bytes}/{total_bytes}  {bytes_per_sec}  ETA {eta}")
                    .unwrap_or_else(|_| ProgressStyle::default_bar())
                    .progress_chars("█▉▊▋▌▍▎▏ "),
            );
            Some(pb)
        }
        None => None,
    };

    let received = AtomicU64::new(0);

    let mut file = match File::create(&file_path).await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("  ✗ Could not create file: {}", e);
            if let Some(pb) = pb { pb.abandon(); }
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create file");
        }
    };

    while let Some(frame) = body.frame().await {
        match frame {
            Ok(frame) => {
                if let Ok(bytes) = frame.into_data() {
                    let len = bytes.len() as u64;
                    if let Err(e) = file.write_all(&bytes).await {
                        eprintln!("  ✗ Write error: {}", e);
                        if let Some(pb) = &pb { pb.abandon(); }
                        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write data");
                    }
                    received.fetch_add(len, Ordering::Relaxed);
                    if let Some(pb) = &pb {
                        pb.inc(len);
                    }
                }
            }
            Err(e) => {
                eprintln!("  ✗ Stream error: {}", e);
                if let Some(pb) = &pb { pb.abandon(); }
                return (StatusCode::BAD_REQUEST, "Stream error");
            }
        }
    }

    let total = received.load(Ordering::Relaxed);
    if let Some(pb) = pb {
        pb.finish_and_clear();
    }
    println!("  ✓  Saved: {} ({})", file_path.display(), human_size(total));
    (StatusCode::OK, "OK")
}

pub fn expand_tilde(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy();
    if s.starts_with("~/") || s == "~" {
        if let Some(home) = dirs_next::home_dir() {
            return home.join(s.trim_start_matches("~/").trim_start_matches('~'));
        }
    }
    path
}

pub fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn pad_right(s: &str, width: usize) -> String {
    if s.len() >= width {
        format!("{}│", &s[..width])
    } else {
        format!("{}{}│", s, " ".repeat(width - s.len()))
    }
}
