use std::net::SocketAddr;
use std::sync::Arc;

use tokio::signal;
use tokio::sync::RwLock;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use chessh::server::SessionManager;
use chessh::ssh::{load_or_create_host_key, SshServer, SshServerConfig};
use chessh::storage::GameHistory;
use chessh::ui::init_sprites;

const DEFAULT_PORT: u16 = 2222;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    FmtSubscriber::builder().with_max_level(Level::INFO).init();

    info!("CheSSH - SSH Chess Server");
    info!("Loading sprite assets...");
    init_sprites();
    info!("Sprites loaded successfully");

    let host_key_path = std::env::var_os("CHESSH_HOST_KEY")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "host_key".into());
    info!("Loading SSH host key from {}", host_key_path.display());
    let host_key = load_or_create_host_key(&host_key_path)?;

    let address: SocketAddr = std::env::var("CHESSH_BIND_ADDR")
        .unwrap_or_else(|_| format!("0.0.0.0:{DEFAULT_PORT}"))
        .parse()?;
    let config = SshServerConfig::new(address, host_key);

    let history_path = std::env::var_os("CHESSH_HISTORY_PATH")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "game_history.jsonl".into());
    let history = GameHistory::open(&history_path)?;
    info!(
        "Loaded {} games from {}",
        history.get_records().len(),
        history_path.display()
    );
    let session_manager = Arc::new(RwLock::new(SessionManager::with_history(history)));

    let server = SshServer::new(session_manager.clone());

    info!("Listening on {address}");
    info!("Connect with: ssh -p {} localhost", address.port());

    let result = tokio::select! {
        result = server.run(config) => result,
        _ = shutdown_signal() => {
            info!("Shutdown signal received, stopping server...");
            Ok(())
        }
    };
    session_manager.write().await.shutdown()?;
    result?;

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
