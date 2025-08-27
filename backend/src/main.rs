use anyhow::Result;
use tracing::{info, warn};
use voice_chat_backend::{config::Config, server::Server};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("voice_chat_backend=debug,tower_http=debug")
        .init();

    info!("Starting Voice Chat Backend Server");

    // Check for loopback mode argument
    let args: Vec<String> = std::env::args().collect();
    let config = if args.len() > 1 && args[1] == "--loopback" {
        info!("🔄 Starting in LOOPBACK mode for local testing");
        Config::load_from("config_loopback")?
    } else {
        Config::load()?
    };
    
    info!("Configuration loaded: {:?}", config);
    if config.audio.loopback_mode {
        warn!("🔄 LOOPBACK MODE ENABLED - Audio will be echoed back to sender");
    }

    // Create and start server
    let server = Server::new(config).await?;
    
    // Handle graceful shutdown
    tokio::select! {
        result = server.run() => {
            if let Err(e) = result {
                warn!("Server error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down gracefully");
        }
    }

    info!("Server shutdown complete");
    Ok(())
}
