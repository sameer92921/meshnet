use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use axum::{
    routing::post,
    Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    body::Body,
};
use http_body_util::BodyExt;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use local_ip_address::local_ip;
use anyhow::Context;
use crate::discovery::SERVICE_TYPE;

struct AppState {
    receive_path: PathBuf,
}

pub async fn run(receive_path: Option<PathBuf>) -> anyhow::Result<()> {
    let receive_path = receive_path.unwrap_or_else(|| PathBuf::from("."));
    
    // Ensure the directory exists
    tokio::fs::create_dir_all(&receive_path).await.context("Failed to create receive directory")?;

    let state = Arc::new(AppState {
        receive_path,
    });

    let app = Router::new()
        .route("/upload", post(handle_upload))
        .with_state(state.clone());

    // Bind to any available port
    let listener = TcpListener::bind("0.0.0.0:0").await?;
    let port = listener.local_addr()?.port();

    let my_ip = local_ip().context("Failed to get local IP")?;
    let hostname = hostname::get()?.into_string().unwrap_or_else(|_| "Unknown-Device".to_string());
    let instance_name = format!("{}-{}", hostname, uuid::Uuid::new_v4().to_string().chars().take(4).collect::<String>());

    println!("Starting meshnet daemon...");
    println!("Receive path: {:?}", state.receive_path);
    println!("Listening on: {}:{}", my_ip, port);
    println!("Device name: {}", instance_name);

    // Register mDNS
    let mdns = ServiceDaemon::new().context("Failed to create mDNS daemon")?;
    
    // Properties can hold additional info, like device type
    let properties = vec![("os", std::env::consts::OS)];
    let service_info = ServiceInfo::new(
        SERVICE_TYPE,
        &instance_name,
        &format!("{}.local.", instance_name),
        my_ip.to_string(),
        port,
        &properties[..],
    ).context("Failed to create service info")?;

    mdns.register(service_info).context("Failed to register mDNS service")?;

    // Start server
    axum::serve(listener, app).await?;

    Ok(())
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
