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
    let receive_path = receive_path.unwrap_or_else(|| PathBuf::from("."));
    
    // Ensure the directory exists
    tokio::fs::create_dir_all(&receive_path).await.context("Failed to create receive directory")?;

    let my_ip = local_ip().context("Failed to get local IP")?;
    let hostname = hostname::get()?.into_string().unwrap_or_else(|_| "Unknown-Device".to_string());
    let short_id: String = uuid::Uuid::new_v4().to_string().chars().take(4).collect();
    let instance_name = format!("{}-{}", hostname, short_id);

    let state = Arc::new(AppState {
        receive_path,
        device_name: instance_name.clone(),
    });

    let app = Router::new()
        .route("/ping", get(handle_ping))
        .route("/upload", post(handle_upload))
        .with_state(state.clone());

    // Try to bind on fixed port; fall back to random if already in use
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", MESHNET_PORT)).await {
        Ok(l) => l,
        Err(_) => {
            eprintln!("Port {} already in use, using a random port.", MESHNET_PORT);
            TcpListener::bind("0.0.0.0:0").await?
        }
    };
    let port = listener.local_addr()?.port();

    println!("╔══════════════════════════════════════════╗");
    println!("║        MeshNet — Receiver Started        ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║  Device : {:<32}║", instance_name);
    println!("║  IP     : {:<32}║", format!("{}:{}", my_ip, port));
    println!("║  Saving : {:<32}║", state.receive_path.display());
    println!("╚══════════════════════════════════════════╝");

    // Register mDNS (best effort — may not work on Android)
    if let Ok(mdns) = ServiceDaemon::new() {
        let properties = vec![("os", std::env::consts::OS)];
        if let Ok(service_info) = ServiceInfo::new(
            SERVICE_TYPE,
            &instance_name,
            &format!("{}.local.", instance_name),
            my_ip.to_string(),
            port,
            &properties[..],
        ) {
            let _ = mdns.register(service_info);
        }
    }

    // Start server
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
    let file_name = match headers.get("X-File-Name") {
        Some(name) => name.to_str().unwrap_or("unknown_file").to_string(),
        None => "unknown_file".to_string(),
    };

    let file_path = state.receive_path.join(&file_name);
    println!("Receiving file: {} -> {:?}", file_name, file_path);

    let mut file = match File::create(&file_path).await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to create file: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create file");
        }
    };

    while let Some(frame) = body.frame().await {
        match frame {
            Ok(frame) => {
                if let Ok(bytes) = frame.into_data() {
                    if let Err(e) = file.write_all(&bytes).await {
                        eprintln!("Failed to write to file: {}", e);
                        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write data");
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading stream: {}", e);
                return (StatusCode::BAD_REQUEST, "Stream error");
            }
        }
    }

    println!("Successfully received {}", file_name);
    (StatusCode::OK, "File received successfully")
}
