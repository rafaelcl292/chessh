use std::net::SocketAddr;
use std::sync::Arc;

use russh::keys::PrivateKey;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use chessh::server::SessionManager;
use chessh::ssh::{SshServer, SshServerConfig};
use chessh::ui::init_sprites;

const DEFAULT_PORT: u16 = 2222;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    FmtSubscriber::builder().with_max_level(Level::INFO).init();

    info!("CheSSH - SSH Chess Server");
    info!("Loading sprite assets...");
    init_sprites();
    info!("Sprites loaded successfully");

    info!("Generating host key...");

    let host_key = PrivateKey::random(&mut rand::rng(), russh::keys::Algorithm::Ed25519)
        .expect("Failed to generate host key");

    let address: SocketAddr = format!("0.0.0.0:{}", DEFAULT_PORT).parse()?;
    let config = SshServerConfig::new(address, host_key);

    let session_manager = Arc::new(RwLock::new(SessionManager::new()));

    let server = SshServer::new(session_manager);

    info!(
        "Connect with: ssh -p {} -o StrictHostKeyChecking=no localhost",
        DEFAULT_PORT
    );

    tokio::select! {
        result = server.run(config) => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = shutdown_signal() => {
            info!("Shutdown signal received, stopping server...");
        }
    }

    info!("Server stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
