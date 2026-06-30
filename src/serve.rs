use std::path::PathBuf;
use std::sync::Arc;
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
            eprintln!(
                "  Port {} already in use, picking a random port.",
                MESHNET_PORT
            );
            TcpListener::bind("0.0.0.0:0").await?
        }
    };
    let port = listener.local_addr()?.port();

    println!("  ┌─────────────────────────────────────────┐");
    println!("  │         Receiver is now active          │");
    println!("  ├─────────────────────────────────────────┤");
    println!("  │  Device  {}",  pad_right(&device_name, 29));
    println!("  │  Address {}",  pad_right(&format!("{}:{}", my_ip, port), 29));
    println!("  │  Saving  {}",  pad_right(&receive_path.display().to_string(), 29));
    println!("  └─────────────────────────────────────────┘");
    println!("  Tip: Share your Address with the sender if discovery fails.\n");

    // mDNS registration (best-effort; not all routers/Android support it)
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

    let file_path = state.receive_path.join(&file_name);
    println!("\n  ↓ Incoming: {}", file_name);

    let mut file = match File::create(&file_path).await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("  ✗ Could not create file: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create file");
        }
    };

    while let Some(frame) = body.frame().await {
        match frame {
            Ok(frame) => {
                if let Ok(bytes) = frame.into_data() {
                    if let Err(e) = file.write_all(&bytes).await {
                        eprintln!("  ✗ Write error: {}", e);
                        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write data");
                    }
                }
            }
            Err(e) => {
                eprintln!("  ✗ Stream error: {}", e);
                return (StatusCode::BAD_REQUEST, "Stream error");
            }
        }
    }

    println!("  ✓ Saved to: {}", file_path.display());
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

fn pad_right(s: &str, width: usize) -> String {
    if s.len() >= width {
        format!("{}│", &s[..width])
    } else {
        format!("{}{}│", s, " ".repeat(width - s.len()))
    }
}
